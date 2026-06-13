use crate::agent::AgentLoop;
use std::sync::Arc;
use anyhow::{anyhow, Result};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, Tokio1Executor, AsyncSmtpTransport, AsyncTransport};

pub struct EmailChannel {
    agent_loop: Arc<AgentLoop>,
}

impl EmailChannel {
    pub fn new(agent_loop: AgentLoop) -> Self {
        EmailChannel {
            agent_loop: Arc::new(agent_loop),
        }
    }
}

fn extract_text_plain(part: &mailparse::ParsedMail) -> Option<String> {
    if part.ctype.mimetype == "text/plain" {
        return part.get_body().ok();
    }
    for subpart in &part.subparts {
        if let Some(body) = extract_text_plain(subpart) {
            return Some(body);
        }
    }
    None
}

fn fetch_unseen_emails(
    imap_server: &str,
    imap_port: u16,
    username: &str,
    password: &str,
) -> Result<Vec<(String, String, String)>> {
    let client = imap::ClientBuilder::new(imap_server, imap_port).connect()?;
    let mut session = client.login(username, password).map_err(|e| anyhow!("{:?}", e))?;
    session.select("INBOX")?;

    let uids = session.uid_search("UNSEEN")?;
    let mut emails = Vec::new();

    for uid in uids.iter() {
        let fetch_results = session.uid_fetch(uid.to_string(), "RFC822")?;
        if let Some(msg) = fetch_results.iter().next() {
            if let Some(body_bytes) = msg.body() {
                if let Ok(parsed) = mailparse::parse_mail(body_bytes) {
                    let from_header = parsed.headers.iter()
                        .find(|h| h.get_key().to_ascii_lowercase() == "from")
                        .map(|h| h.get_value())
                        .unwrap_or_default();

                    let clean_from = if from_header.contains('<') {
                        from_header.split('<')
                            .nth(1)
                            .and_then(|s| s.split('>').next())
                            .unwrap_or(&from_header)
                            .trim()
                            .to_string()
                    } else {
                        from_header.trim().to_string()
                    };

                    let subject = parsed.headers.iter()
                        .find(|h| h.get_key().to_ascii_lowercase() == "subject")
                        .map(|h| h.get_value())
                        .unwrap_or_default()
                        .trim()
                        .to_string();

                    let body = extract_text_plain(&parsed).unwrap_or_default();

                    emails.push((clean_from, subject, body));
                }
            }
        }
    }

    session.logout()?;
    Ok(emails)
}

async fn send_reply_email(
    smtp_server: &str,
    smtp_port: u16,
    username: &str,
    password: &str,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<()> {
    let mut builder = AsyncSmtpTransport::<Tokio1Executor>::relay(smtp_server)?
        .port(smtp_port);

    if !password.is_empty() {
        let creds = Credentials::new(username.to_string(), password.to_string());
        builder = builder.credentials(creds);
    }

    let transport = builder.build();

    let email = Message::builder()
        .from(username.parse()?)
        .to(to.parse()?)
        .subject(subject)
        .body(body.to_string())?;

    transport.send(email).await?;
    Ok(())
}

#[async_trait::async_trait]
impl super::Channel for EmailChannel {
    fn name(&self) -> &'static str {
        "email"
    }

    async fn start(&self) -> anyhow::Result<()> {
        let silent = std::env::var("OPENZ_SILENT").is_ok();
        let email_config = match &self.agent_loop.config.channels.email {
            Some(cfg) => cfg.clone(),
            None => {
                if !silent {
                    println!("⚠️ Email configuration not found. Email channel deactivated.");
                }
                return Ok(());
            }
        };

        if !email_config.enabled {
            if !silent {
                println!("⚠️ Email channel is disabled in configuration.");
            }
            return Ok(());
        }

        if !silent {
            println!("🤖 Email Channel listening started (polling every {}s)...", email_config.poll_interval_secs);
        }

        let agent = self.agent_loop.clone();

        tokio::spawn(async move {
            let poll_interval = std::time::Duration::from_secs(email_config.poll_interval_secs);
            loop {
                let imap_server = email_config.imap_server.clone();
                let imap_port = email_config.imap_port;
                let username = email_config.username.clone();
                let password = email_config.password.clone();

                let fetched = tokio::task::spawn_blocking(move || {
                    fetch_unseen_emails(&imap_server, imap_port, &username, &password)
                }).await;

                match fetched {
                    Ok(Ok(emails)) => {
                        for (from, subject, body) in emails {
                            let agent_clone = agent.clone();
                            let smtp_server = email_config.smtp_server.clone();
                            let smtp_port = email_config.smtp_port;
                            let user_clone = email_config.username.clone();
                            let pass_clone = email_config.password.clone();

                            tokio::spawn(async move {
                                let reply_subject = if subject.to_lowercase().starts_with("re:") {
                                    subject.clone()
                                } else {
                                    format!("Re: {}", subject)
                                };

                                let session_key = format!("email_{}", from);
                                match agent_clone.run(&body, &session_key).await {
                                    Ok(res) => {
                                        let reply_res = send_reply_email(
                                            &smtp_server,
                                            smtp_port,
                                            &user_clone,
                                            &pass_clone,
                                            &from,
                                            &reply_subject,
                                            &res.content,
                                        ).await;

                                        if let Err(e) = reply_res {
                                            eprintln!("Failed to send reply email to {}: {:?}", from, e);
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Agent failed for email from {}: {:?}", from, e);
                                    }
                                }
                            });
                        }
                    }
                    Ok(Err(e)) => {
                        eprintln!("Error fetching emails: {:?}", e);
                    }
                    Err(e) => {
                        eprintln!("Join error during email fetch: {:?}", e);
                    }
                }

                tokio::time::sleep(poll_interval).await;
            }
        });

        Ok(())
    }
}
