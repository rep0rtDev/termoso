//! Outgoing email (SMTP). Plain-text, no tracking pixels, no external assets.

use anyhow::Context;
use lettre::message::{Mailbox, MultiPart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::config::{SmtpConfig, SmtpSecurity};

#[derive(Clone)]
pub struct Mailer {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
    server_name: String,
}

impl Mailer {
    pub fn new(cfg: &SmtpConfig, server_name: &str) -> anyhow::Result<Self> {
        let mut builder = match cfg.security {
            SmtpSecurity::Tls => AsyncSmtpTransport::<Tokio1Executor>::relay(&cfg.host)?,
            SmtpSecurity::Starttls => {
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.host)?
            }
            SmtpSecurity::None => {
                AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&cfg.host)
            }
        }
        .port(cfg.port)
        .timeout(Some(std::time::Duration::from_secs(20)));
        if let (Some(u), Some(p)) = (&cfg.username, &cfg.password) {
            builder = builder.credentials(Credentials::new(u.clone(), p.clone()));
        }
        let from: Mailbox = cfg
            .from
            .parse()
            .context("TERMOSO_SMTP__FROM must be a mailbox")?;
        Ok(Self {
            transport: builder.build(),
            from,
            server_name: server_name.to_string(),
        })
    }

    pub async fn send(&self, to: &str, subject: &str, text: &str) -> anyhow::Result<()> {
        let to: Mailbox = to.parse().context("invalid recipient")?;
        let body = format!("{text}\n\n— {}\n", self.server_name);
        let html = format!(
            "<!doctype html><html><body style=\"font-family:system-ui,sans-serif;color:#1A1E2B\">\
             <pre style=\"font:inherit;white-space:pre-wrap\">{}</pre>\
             <p style=\"color:#8E93A8\">— {}</p></body></html>",
            html_escape(text),
            html_escape(&self.server_name)
        );
        let msg = Message::builder()
            .from(self.from.clone())
            .to(to)
            .subject(subject)
            .multipart(MultiPart::alternative_plain_html(body, html))?;
        self.transport.send(msg).await.context("smtp send")?;
        Ok(())
    }

    pub fn code_email(&self, purpose: &str, code: &str, ttl_minutes: u64) -> (String, String) {
        let subject = format!("{}: {purpose} — code {code}", self.server_name);
        let text = format!(
            "Your {} code for {purpose}:\n\n    {code}\n\nIt is valid for {ttl_minutes} minutes. \
             If you did not request this, you can ignore this email.",
            self.server_name
        );
        (subject, text)
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
