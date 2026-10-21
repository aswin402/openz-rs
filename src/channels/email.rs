use crate::agent::AgentLoop;
use anyhow::{anyhow, Result};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use std::sync::Arc;

pub struct EmailChannel {
    agent_loop: Arc<AgentLoop>,
}

impl EmailChannel {
    pub fn new(agent_loop: AgentLoop) -> Self {
        Self::with_shared_loop(Arc::new(agent_loop))
    }

    pub fn with_shared_loop(agent_loop: Arc<AgentLoop>) -> Self {
        EmailChannel { agent_loop }
    }
}

fn extract_text_plain(part: &mailparse::ParsedMail) -> Option<String> {
    match part.ctype.mimetype == "text/plain" {
        true => part.get_body().ok(),
        false => part.subparts.iter().find_map(extract_text_plain),
    }
}

fn clean_email_sender(from_header: &str) -> String {
    match from_header.contains('<') {
        true => from_header
            .split('<')
            .nth(1)
            .and_then(|s| s.split('>').next())
            .unwrap_or(from_header)
            .trim()
            .to_string(),
        false => from_header.trim().to_string(),
    }
}

fn fetch_unseen_emails(
    imap_server: &str,
    imap_port: u16,
    username: &str,
    password: &str,
) -> Result<Vec<(String, String, String)>> {
    let client = imap::ClientBuilder::new(imap_server, imap_port).connect()?;
    let mut session = client
        .login(username, password)
        .map_err(|e| anyhow!("{:?}", e))?;
    session.select("INBOX")?;

    let uids = session.uid_search("UNSEEN")?;
    let mut emails = Vec::new();

    for uid in uids.iter() {
        let fetch_results = session.uid_fetch(uid.to_string(), "RFC822")?;
        let parsed = fetch_results
            .iter()
            .next()
            .and_then(|msg| msg.body())
            .and_then(|body_bytes| mailparse::parse_mail(body_bytes).ok());

        match parsed {
            Some(parsed) => {
                let from_header = parsed
                    .headers
                    .iter()
                    .find(|h| h.get_key().eq_ignore_ascii_case("from"))
                    .map(|h| h.get_value())
                    .unwrap_or_default();

                let clean_from = clean_email_sender(&from_header);

                let subject = parsed
                    .headers
                    .iter()
                    .find(|h| h.get_key().eq_ignore_ascii_case("subject"))
                    .map(|h| h.get_value())
                    .unwrap_or_default()
                    .trim()
                    .to_string();

                let body = extract_text_plain(&parsed).unwrap_or_default();

                emails.push((clean_from, subject, body));
            }
            None => {}
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
    let mut builder = AsyncSmtpTransport::<Tokio1Executor>::relay(smtp_server)?.port(smtp_port);

    match password.is_empty() {
        true => {}
        false => {
            let creds = Credentials::new(username.to_string(), password.to_string());
            builder = builder.credentials(creds);
        }
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

async fn send_reply_and_log(
    smtp_server: &str,
    smtp_port: u16,
    username: &str,
    password: &str,
    to: &str,
    subject: &str,
    body: &str,
    context: &str,
) {
    match send_reply_email(smtp_server, smtp_port, username, password, to, subject, body).await {
        Ok(()) => {}
        Err(e) => {
            tracing::error!("Failed to send {} email to {}: {:?}", context, to, e);
        }
    }
}

fn spawn_single_email_task(
    agent_clone: Arc<AgentLoop>,
    sem_clone: Arc<tokio::sync::Semaphore>,
    smtp_server: String,
    smtp_port: u16,
    user_clone: String,
    pass_clone: String,
    from: String,
    subject: String,
    body: String,
) {
    tokio::spawn(async move {
        let _permit = sem_clone.acquire_owned().await;
        let reply_subject = match subject.to_lowercase().starts_with("re:") {
            true => subject.clone(),
            false => format!("Re: {}", subject),
        };

        let session_key = format!("email_{}", from);
        match crate::channels::intercept_channel_text_command(
            &agent_clone.session_manager,
            &session_key,
            &body,
            "Stop requested. Active OpenZ turn interrupted.",
        )
        .await
        {
            Some(response_text) => {
                send_reply_and_log(
                    &smtp_server,
                    smtp_port,
                    &user_clone,
                    &pass_clone,
                    &from,
                    &reply_subject,
                    &response_text,
                    "command reply",
                )
                .await;
                return;
            }
            None => {}
        }

        match agent_clone.run(&body, &session_key).await {
            Ok(res) => {
                send_reply_and_log(
                    &smtp_server,
                    smtp_port,
                    &user_clone,
                    &pass_clone,
                    &from,
                    &reply_subject,
                    &res.content,
                    "reply",
                )
                .await;
            }
            Err(e) => {
                tracing::error!("Agent failed for email from {}: {:?}", from, e);
            }
        }
    });
}

#[async_trait::async_trait]
impl super::Channel for EmailChannel {
    fn name(&self) -> &'static str {
        "email"
    }

    async fn start(&self) -> anyhow::Result<()> {
        let silent = std::env::var("OPENZ_SILENT").is_ok();
        let email_config = match (&self.agent_loop.config.channels.email, silent) {
            (Some(cfg), _) => cfg.clone(),
            (None, false) => {
                println!("⚠️ Email configuration not found. Email channel deactivated.");
                return Ok(());
            }
            (None, true) => return Ok(()),
        };

        match (email_config.enabled, silent) {
            (false, false) => {
                println!("⚠️ Email channel is disabled in configuration.");
                return Ok(());
            }
            (false, true) => return Ok(()),
            (true, _) => {}
        }

        let lock_id = format!(
            "{}@{}:{}",
            email_config.username, email_config.imap_server, email_config.imap_port
        );
        let poll_lock = match crate::channels::telegram::lock::acquire_channel_singleton_lock(
            "email", &lock_id,
        )? {
            Some(lock) => lock,
            None => {
                match silent {
                    false => {
                        println!(
                            "⚠️ Email polling is already active in another OpenZ process. Skipping duplicate poller."
                        );
                    }
                    true => {}
                }
                tracing::warn!("Email polling already active; skipping duplicate poller");
                return Ok(());
            }
        };

        match silent {
            false => println!(
                "🤖 Email Channel listening started (polling every {}s)...",
                email_config.poll_interval_secs
            ),
            true => {}
        }

        let agent = self.agent_loop.clone();
        let concurrency_limit = crate::channels::channel_concurrency_semaphore();

        let mut shutdown_rx = match crate::shutdown::receiver() {
            Some(rx) => rx,
            None => {
                let (_, rx) = tokio::sync::watch::channel(false);
                rx
            }
        };

        tokio::spawn(async move {
            let _poll_lock = poll_lock;
            let poll_interval = std::time::Duration::from_secs(email_config.poll_interval_secs);
            loop {
                match *shutdown_rx.borrow() {
                    true => break,
                    false => {}
                }

                let imap_server = email_config.imap_server.clone();
                let imap_port = email_config.imap_port;
                let username = email_config.username.clone();
                let password = email_config.password.clone();

                let fetched = tokio::task::spawn_blocking(move || {
                    fetch_unseen_emails(&imap_server, imap_port, &username, &password)
                })
                .await;

                match fetched {
                    Ok(Ok(emails)) => {
                        for (from, subject, body) in emails {
                            spawn_single_email_task(
                                agent.clone(),
                                concurrency_limit.clone(),
                                email_config.smtp_server.clone(),
                                email_config.smtp_port,
                                email_config.username.clone(),
                                email_config.password.clone(),
                                from,
                                subject,
                                body,
                            );
                        }
                    }
                    Ok(Err(e)) => {
                        tracing::error!("Error fetching emails: {:?}", e);
                    }
                    Err(e) => {
                        tracing::error!("Join error during email fetch: {:?}", e);
                    }
                }

                tokio::select! {
                    biased;
                    _ = shutdown_rx.changed() => {
                        break;
                    }
                    _ = tokio::time::sleep(poll_interval) => {}
                }
            }
        });

        Ok(())
    }
}

#[cfg(test)]
#[path = "email_tests.rs"]
mod tests;
