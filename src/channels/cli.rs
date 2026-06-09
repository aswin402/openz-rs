use crate::agent::AgentLoop;
use crate::config::schema::AgentDefaults;
use crate::agent::style::*;
use std::io::{self, Write};

pub struct CliChannel {
    agent_loop: AgentLoop,
    defaults: AgentDefaults,
}

fn handle_clipboard_paste(index: usize) -> anyhow::Result<std::path::PathBuf> {
    use arboard::Clipboard;
    let mut clipboard = Clipboard::new()?;
    let image = clipboard.get_image()?;
    
    let path = crate::config::resolve_path(&format!("~/.openz/clipboard_image_{}.png", index));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    
    image::save_buffer(
        &path,
        &image.bytes,
        image.width as u32,
        image.height as u32,
        image::ColorType::Rgba8,
    )?;
    
    Ok(path)
}

fn char_display_width(c: char) -> usize {
    let cp = c as u32;
    if (cp >= 0x1F000 && cp <= 0x1FBF9) || c == '⬢' || c == '🗑' || c == '📊' {
        2
    } else {
        1
    }
}

fn string_display_width(s: &str) -> usize {
    s.chars().map(char_display_width).sum()
}

fn print_colored_markdown(content: &str) {
    use crate::agent::style::*;
    let light_blue = "\x1b[38;2;135;206;250m";
    
    for line in content.lines() {
        if line.trim_start().starts_with("#") {
            println!("{}{}{}", HEADING_BLUE, line, COLOR_RESET);
        } else {
            let mut formatted = line.to_string();
            
            if let Ok(re_bold) = regex::Regex::new(r"\*\*(.*?)\*\*") {
                formatted = re_bold.replace_all(&formatted, &format!("{}{}$1{}", RED_ORANGE, COLOR_BOLD, COLOR_RESET)).to_string();
            }
            if let Ok(re_code) = regex::Regex::new(r"`(.*?)`") {
                formatted = re_code.replace_all(&formatted, &format!("{}$1{}", light_blue, COLOR_RESET)).to_string();
            }
            if let Ok(re_italic) = regex::Regex::new(r"\*(.*?)\*") {
                formatted = re_italic.replace_all(&formatted, &format!("{}$1{}", light_blue, COLOR_RESET)).to_string();
            }
            
            println!("{}", formatted);
        }
    }
}

fn render_box(
    model: &str,
    provider: &str,
    session_manager: &crate::session::SessionManager,
    session_key: &str,
    input: &str,
    width: usize,
) -> anyhow::Result<()> {
    use crate::agent::style::*;
    use std::io::{stdout, Write};
    
    // 1. Calculate token usage
    let session = session_manager.get_or_create(session_key);
    let total_chars: usize = session.messages.iter().map(|m| m.content.len()).sum();
    let approx_tokens = total_chars / 4;
    
    let model_lower = model.to_lowercase();
    let limit_tokens = if model_lower.contains("claude-3-5") {
        200_000
    } else if model_lower.contains("gpt-4") {
        128_000
    } else if model_lower.contains("gemini") {
        1_048_576
    } else if model_lower.contains("minimax") {
        204_800
    } else if model_lower.contains("deepseek") {
        64_000
    } else {
        128_000
    };
    
    let limit_str = if limit_tokens >= 1_000_000 {
        format!("{}M", limit_tokens / 1_000_000)
    } else {
        format!("{}K", limit_tokens / 1000)
    };
    
    let approx_tokens_str = if approx_tokens >= 1000 {
        format!("{:.1}K", approx_tokens as f64 / 1000.0)
    } else {
        format!("{}", approx_tokens)
    };
    
    let provider_lower = provider.to_lowercase();
    let display_provider = match provider_lower.as_str() {
        "openai" => "OpenAI",
        "anthropic" => "Anthropic",
        "google" => "Google",
        "deepseek" => "DeepSeek",
        other => other,
    };

    let max_model_len = if width < 60 {
        10
    } else if width < 80 {
        16
    } else {
        30
    };

    let display_model = if model.len() > max_model_len {
        format!("{}…", &model[..max_model_len - 1])
    } else {
        model.to_string()
    };

    let visible_status_len = display_provider.chars().count()
        + display_model.chars().count()
        + approx_tokens_str.chars().count()
        + limit_str.chars().count()
        + 11; // spacing, vertical pipes, slash, brackets

    let fill_chars = width.saturating_sub(visible_status_len);
    let line_fill: String = std::iter::repeat('─').take(fill_chars).collect();

    let status_content = format!(
        " {}{}{} | {}{}{} | {}{}{}/{}{}",
        RED_ORANGE, display_provider, LIGHT_WHITE,
        RED_ORANGE, display_model, LIGHT_WHITE,
        RED_ORANGE, approx_tokens_str, LIGHT_WHITE,
        RED_ORANGE, limit_str
    );

    let status_line = format!(
        "{}{}{}[{}]{}{}",
        LIGHT_WHITE, line_fill,
        LIGHT_WHITE, status_content, LIGHT_WHITE,
        COLOR_RESET
    );
    
    // Print input line prefix and input
    let input_width = string_display_width(input);
    let max_input_width = width.saturating_sub(3);
    
    let (display_input, display_width) = if input_width > max_input_width {
        let mut start_idx = 0;
        let mut current_width = input_width;
        for (i, c) in input.char_indices() {
            if current_width <= max_input_width {
                start_idx = i;
                break;
            }
            current_width -= char_display_width(c);
        }
        (&input[start_idx..], current_width)
    } else {
        (input, input_width)
    };
    
    print!("\r\x1b[2K{}> {}{}", LIGHT_WHITE, COLOR_RESET, display_input);
    
    // Print status line below
    print!("\r\n\x1b[2K{}", status_line);
    
    // Move cursor back up to the input line and place it at the end of the text
    let cursor_col = 3 + display_width;
    print!("\x1b[1A\x1b[{}G", cursor_col);
    
    stdout().flush()?;
    Ok(())
}

fn read_line_raw(
    model: &str,
    provider: &str,
    session_manager: &crate::session::SessionManager,
    session_key: &str,
) -> anyhow::Result<String> {
    use crossterm::event::{self, Event, KeyCode, KeyModifiers, KeyEventKind};
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
    use std::io::stdout;

    enable_raw_mode()?;
    let mut input = String::new();
    let mut pasted_images = Vec::new();
    let (width, _) = crossterm::terminal::size().unwrap_or((80, 24));
    let mut width_usize = width as usize;

    render_box(model, provider, session_manager, session_key, &input, width_usize)?;

    loop {
        if let Some(inbox_msg) = crate::agent::activity::pop_inbox_message("cli:direct") {
            disable_raw_mode()?;
            println!("\r\n{}🔌 [Remote Control] Received prompt: {}{}", AURA_BLUE, inbox_msg.message, COLOR_RESET);
            return Ok(inbox_msg.message);
        }

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key_event) = event::read()? {
                if key_event.kind == KeyEventKind::Release {
                    continue;
                }

                let ctrl = key_event.modifiers.contains(KeyModifiers::CONTROL);
                let alt = key_event.modifiers.contains(KeyModifiers::ALT);

                // Ctrl+C or Ctrl+D to exit
                if ctrl && (key_event.code == KeyCode::Char('c') || key_event.code == KeyCode::Char('d')) {
                    disable_raw_mode()?;
                    println!("\r\nGoodbye!");
                    std::process::exit(0);
                }

                // Ctrl+V or Alt+V to paste image
                let is_paste_image = (ctrl && key_event.code == KeyCode::Char('v')) || (alt && key_event.code == KeyCode::Char('v'));
                if is_paste_image {
                    let next_index = pasted_images.len();
                    match handle_clipboard_paste(next_index) {
                        Ok(img_path) => {
                            pasted_images.push(img_path);
                            let placeholder = if next_index == 0 {
                                "[image]".to_string()
                            } else {
                                format!("[image{}]", next_index)
                            };
                            let space = if input.is_empty() { "" } else { " " };
                            input.push_str(&format!("{space}{placeholder}"));
                            if let Ok((w, _)) = crossterm::terminal::size() {
                                width_usize = w as usize;
                            }
                            render_box(model, provider, session_manager, session_key, &input, width_usize)?;
                        }
                        Err(e) => {
                            print!("\r\n\x1b[2K{}✕ Error: No image found in clipboard: {}{}\r\n", ERROR_RED, e, COLOR_RESET);
                            if let Ok((w, _)) = crossterm::terminal::size() {
                                width_usize = w as usize;
                            }
                            render_box(model, provider, session_manager, session_key, &input, width_usize)?;
                        }
                    }
                    continue;
                }

                if let Ok((w, _)) = crossterm::terminal::size() {
                    width_usize = w as usize;
                }

                match key_event.code {
                    KeyCode::Char(c) => {
                        input.push(c);
                        render_box(model, provider, session_manager, session_key, &input, width_usize)?;
                    }
                    KeyCode::Backspace => {
                        if !input.is_empty() {
                            input.pop();
                            render_box(model, provider, session_manager, session_key, &input, width_usize)?;
                        }
                    }
                    KeyCode::Enter => {
                        disable_raw_mode()?;
                        print!("\x1b[1B\r\n");
                        stdout().flush()?;
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    let mut final_input = input.clone();
    for (i, path) in pasted_images.iter().enumerate() {
        let placeholder = if i == 0 {
            "[image]".to_string()
        } else {
            format!("[image{}]", i)
        };
        let replacement = format!("![](file://{})", path.to_string_lossy());
        final_input = final_input.replace(&placeholder, &replacement);
    }

    Ok(final_input)
}

impl CliChannel {
    pub fn new(agent_loop: AgentLoop, defaults: AgentDefaults) -> Self {
        CliChannel {
            agent_loop,
            defaults,
        }
    }
}

#[async_trait::async_trait]
impl super::Channel for CliChannel {
    fn name(&self) -> &'static str {
        "cli"
    }

    async fn start(&self) -> anyhow::Result<()> {
        let session_key = "cli:direct";
        
        let white = "\x1b[38;2;240;240;240m";
        let slate = "\x1b[38;2;107;122;153m";
        
        println!("{}     ██████╗ ██████╗ ███████╗███╗   ██╗{}███████╗", white, RED_ORANGE);
        println!("{}    ██╔═══██╗██╔══██╗██╔════╝████╗  ██║{}╚══███╔╝", white, RED_ORANGE);
        println!("{}    ██║   ██║██████╔╝█████╗  ██╔██╗ ██║{}  ███╔╝", white, RED_ORANGE);
        println!("{}    ██║   ██║██╔═══╝ ██╔══╝  ██║╚██╗██║{} ███╔╝", white, RED_ORANGE);
        println!("{}    ╚██████╔╝██║     ███████╗██║ ╚████║{}███████╗", white, RED_ORANGE);
        println!("{}     ╚═════╝ ╚═╝     ╚══════╝╚═╝  ╚═══╝{}╚══════╝\r", white, RED_ORANGE);
        
        println!("{}openz v{}{}", COLOR_BOLD, env!("CARGO_PKG_VERSION"), COLOR_RESET);
        println!("{}{}{}", slate, format!("{} | {}", self.defaults.provider, self.defaults.model), COLOR_RESET);
        
        if let Ok(current_dir) = std::env::current_dir() {
            let path_str = if let Some(home) = dirs::home_dir() {
                if current_dir == home {
                    "~".to_string()
                } else if let Ok(stripped) = current_dir.strip_prefix(&home) {
                    format!("~/{}", stripped.display())
                } else {
                    current_dir.display().to_string()
                }
            } else {
                current_dir.display().to_string()
            };
            println!("{}{}{}", slate, path_str, COLOR_RESET);
        }
        
        println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
        
        loop {
            let input = match read_line_raw(
                &self.defaults.model,
                &self.defaults.provider,
                &self.agent_loop.session_manager,
                session_key,
            ) {
                Ok(inp) => inp,
                Err(e) => {
                    eprintln!("Error reading input: {}", e);
                    continue;
                }
            };
            let trimmed = input.trim();
            
            if trimmed.is_empty() {
                continue;
            }
 
            if trimmed == "/exit" || trimmed == "exit" || trimmed == "quit" {
                println!("Goodbye!");
                break;
            }
 
            if trimmed == "/paste" || trimmed == "/clip" {
                match handle_clipboard_paste(0) {
                    Ok(img_path) => {
                        println!("{}✓ Image captured from clipboard and saved to: {}{}", EMERALD_GREEN, img_path.display(), COLOR_RESET);
                        print!("Enter query/instructions for this image: ");
                        io::stdout().flush()?;
                        let mut query = String::new();
                        io::stdin().read_line(&mut query)?;
                        let combined_query = format!("{} ![](file://{})", query.trim(), img_path.to_string_lossy());
                        
                        match self.agent_loop.run(&combined_query, session_key).await {
                            Ok(res) => {
                                println!();
                                print_colored_markdown(&res.content);
                                println!();
                                println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
                            }
                            Err(e) => {
                                eprintln!("{}✕ Error: {}{}", ERROR_RED, e, COLOR_RESET);
                                println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("{}✕ Error: Failed to retrieve image from clipboard: {}{}", ERROR_RED, e, COLOR_RESET);
                    }
                }
                continue;
            }
 
            match self.agent_loop.run(trimmed, session_key).await {
                Ok(res) => {
                    println!();
                    print_colored_markdown(&res.content);
                    println!();
                    println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
                }
                Err(e) => {
                    eprintln!("{}✕ Error: {}{}", ERROR_RED, e, COLOR_RESET);
                    println!("{}────────────────────────────────────────────────────────────{}", LIGHT_WHITE, COLOR_RESET);
                }
            }
        }
        
        Ok(())
    }
}
