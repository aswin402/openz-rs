use crate::agent::AgentLoop;
use crate::config::schema::AgentDefaults;
use std::io::{self, Write};

pub struct CliChannel {
    agent_loop: AgentLoop,
    defaults: AgentDefaults,
}

impl CliChannel {
    pub fn new(agent_loop: AgentLoop, defaults: AgentDefaults) -> Self {
        CliChannel {
            agent_loop,
            defaults,
        }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let mut input = String::new();
        let session_key = "cli:direct";
        
        println!("Welcome to {}! Type your message and press Enter (or type /restart to clear, /help for list of commands).", self.defaults.bot_name);
        
        loop {
            print!("{} {} > ", self.defaults.bot_icon, self.defaults.bot_name);
            io::stdout().flush()?;
            
            input.clear();
            io::stdin().read_line(&mut input)?;
            let trimmed = input.trim();
            
            if trimmed.is_empty() {
                continue;
            }

            if trimmed == "/exit" || trimmed == "exit" || trimmed == "quit" {
                println!("Goodbye!");
                break;
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
