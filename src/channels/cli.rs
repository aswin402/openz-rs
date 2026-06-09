use crate::agent::AgentLoop;
use crate::config::schema::AgentDefaults;
use crate::agent::style::*;
use std::io::{self, Write};

pub struct CliChannel {
    agent_loop: AgentLoop,
    defaults: AgentDefaults,
}

fn handle_clipboard_paste() -> anyhow::Result<std::path::PathBuf> {
    use arboard::Clipboard;
    let mut clipboard = Clipboard::new()?;
    let image = clipboard.get_image()?;
    
    let path = crate::config::resolve_path("~/.openz/clipboard_image.png");
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

fn read_line_raw(prompt: &str) -> anyhow::Result<String> {
    use crossterm::event::{self, Event, KeyCode, KeyModifiers, KeyEventKind};
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
    use std::io::stdout;

    enable_raw_mode()?;
    let mut input = String::new();
    print!("{}", prompt);
    stdout().flush()?;

    loop {
        if let Some(inbox_msg) = crate::agent::activity::pop_inbox_message("cli:direct") {
            disable_raw_mode()?;
            println!("\r\n{}[INFO] 🔌 [Remote Control] Received prompt: {}{}", AURA_BLUE, inbox_msg.message, COLOR_RESET);
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
                    match handle_clipboard_paste() {
                        Ok(img_path) => {
                            print!("\r\n{}[INFO] ◇ Image pasted from clipboard: {}{}\r\n", AURA_GREEN, img_path.display(), COLOR_RESET);
                            // Append markdown link to input
                            input.push_str(&format!(" ![](file://{})", img_path.to_string_lossy()));
                            print!("{}", prompt);
                            print!("{}", input);
                            stdout().flush()?;
                        }
                        Err(e) => {
                            print!("\r\n{}[ERROR] ◇ No image found in clipboard: {}{}\r\n", AURA_ROSE, e, COLOR_RESET);
                            print!("{}", prompt);
                            print!("{}", input);
                            stdout().flush()?;
                        }
                    }
                    continue;
                }

                match key_event.code {
                    KeyCode::Char(c) => {
                        input.push(c);
                        print!("{}", c);
                        stdout().flush()?;
                    }
                    KeyCode::Backspace => {
                        if !input.is_empty() {
                            input.pop();
                            print!("\u{8} \u{8}");
                            stdout().flush()?;
                        }
                    }
                    KeyCode::Enter => {
                        disable_raw_mode()?;
                        println!();
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(input)
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
        let orange = "\x1b[38;2;255;165;0m";
        let slate = "\x1b[38;2;107;122;153m";
        
        println!("{}     ██████╗ ██████╗ ███████╗███╗   ██╗{}███████╗", white, orange);
        println!("{}    ██╔═══██╗██╔══██╗██╔════╝████╗  ██║{}╚══███╔╝", white, orange);
        println!("{}    ██║   ██║██████╔╝█████╗  ██╔██╗ ██║{}  ███╔╝", white, orange);
        println!("{}    ██║   ██║██╔═══╝ ██╔══╝  ██║╚██╗██║{} ███╔╝", white, orange);
        println!("{}    ╚██████╔╝██║     ███████╗██║ ╚████║{}███████╗", white, orange);
        println!("{}     ╚═════╝ ╚═╝     ╚══════╝╚═╝  ╚═══╝{}╚══════╝\r", white, orange);
        
        println!("{}openz v{}{}", COLOR_BOLD, env!("CARGO_PKG_VERSION"), COLOR_RESET);
        println!("{}Provider: {} | Model: {}{}", slate, self.defaults.provider, self.defaults.model, COLOR_RESET);
        
        if let Ok(current_dir) = std::env::current_dir() {
            println!("{}Directory: {}{}", slate, current_dir.display(), COLOR_RESET);
        }
        
        println!("{}────────────────────────────────────────────────────────────{}", slate, COLOR_RESET);
        
        let prompt = format!("{}{}{} {} > {}", COLOR_BOLD, AURA_PURPLE, self.defaults.bot_icon, self.defaults.bot_name, COLOR_RESET);
        
        loop {
            let input = match read_line_raw(&prompt) {
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
                match handle_clipboard_paste() {
                    Ok(img_path) => {
                        println!("{}[INFO] ◇ Image captured from clipboard and saved to: {}{}", AURA_GREEN, img_path.display(), COLOR_RESET);
                        print!("Enter query/instructions for this image: ");
                        io::stdout().flush()?;
                        let mut query = String::new();
                        io::stdin().read_line(&mut query)?;
                        let combined_query = format!("{} ![](file://{})", query.trim(), img_path.to_string_lossy());
                        
                        match self.agent_loop.run(&combined_query, session_key).await {
                            Ok(res) => {
                                println!("\n{}\n", res.content);
                            }
                            Err(e) => {
                                eprintln!("Error: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("{}[ERROR] ◇ Failed to retrieve image from clipboard: {}{}", AURA_ROSE, e, COLOR_RESET);
                    }
                }
                continue;
            }
 
            match self.agent_loop.run(trimmed, session_key).await {
                Ok(res) => {
                    println!("\n{}\n", res.content);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                }
            }
        }
        
        Ok(())
    }
}
