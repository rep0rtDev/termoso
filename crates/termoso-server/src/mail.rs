//! Outgoing email (SMTP). Every message is built from an [`Email`]: a plain
//! text part for text-only clients and an HTML part laid out the way mail
//! clients still render reliably — nested tables, inline styles, no external
//! CSS, fonts or scripts. The only remote asset is the logo, served from the
//! public site (no tracking pixels).

use anyhow::Context;
use lettre::message::{Mailbox, MultiPart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::config::{Config, SmtpConfig, SmtpSecurity};

#[derive(Clone)]
pub struct Mailer {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
    server_name: String,
    /// Public site the footer links to and the logo is loaded from.
    site_url: String,
}

/// A transactional email: a title, paragraphs and at most one of each
/// highlight (code box, button, secondary link, detail rows, log block).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Email {
    pub subject: String,
    /// Shown by inbox previews next to the subject; hidden in the body.
    pub preheader: String,
    pub title: String,
    pub intro: Vec<String>,
    /// One-time code, rendered large and monospaced.
    pub code: Option<String>,
    pub button: Option<Link>,
    /// Plain link after the body text (e.g. the "go ahead" counterpart of a
    /// cancel button, or where to change a subscription).
    pub secondary: Option<Link>,
    /// Label / value rows (digest counts, device details).
    pub rows: Vec<(String, String)>,
    /// Small label above the log block.
    pub log_title: Option<String>,
    /// Monospaced block (digest timeline).
    pub log: Vec<String>,
    pub outro: Vec<String>,
    /// Small print at the bottom of the card ("if this was not you…").
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub label: String,
    pub url: String,
}

impl Link {
    pub fn new(label: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            url: url.into(),
        }
    }
}

impl Email {
    pub fn new(subject: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            subject: subject.into(),
            title: title.into(),
            ..Self::default()
        }
    }

    pub fn preheader(mut self, s: impl Into<String>) -> Self {
        self.preheader = s.into();
        self
    }

    pub fn paragraph(mut self, s: impl Into<String>) -> Self {
        self.intro.push(s.into());
        self
    }

    pub fn code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn button(mut self, label: impl Into<String>, url: impl Into<String>) -> Self {
        self.button = Some(Link::new(label, url));
        self
    }

    pub fn secondary(mut self, label: impl Into<String>, url: impl Into<String>) -> Self {
        self.secondary = Some(Link::new(label, url));
        self
    }

    pub fn row(mut self, label: impl Into<String>, value: impl Into<String>) -> Self {
        self.rows.push((label.into(), value.into()));
        self
    }

    pub fn log(
        mut self,
        title: impl Into<String>,
        lines: impl IntoIterator<Item = String>,
    ) -> Self {
        self.log_title = Some(title.into());
        self.log.extend(lines);
        self
    }

    pub fn outro(mut self, s: impl Into<String>) -> Self {
        self.outro.push(s.into());
        self
    }

    pub fn note(mut self, s: impl Into<String>) -> Self {
        self.note = Some(s.into());
        self
    }

    /// Plain-text alternative: the same content, no markup.
    pub fn text(&self, server_name: &str, site_url: &str) -> String {
        let mut out = String::new();
        out.push_str(&self.title);
        out.push_str("\n\n");
        for p in &self.intro {
            out.push_str(p);
            out.push_str("\n\n");
        }
        if let Some(code) = &self.code {
            out.push_str(&format!("    {code}\n\n"));
        }
        if let Some(b) = &self.button {
            out.push_str(&format!("{}:\n    {}\n\n", b.label, b.url));
        }
        if !self.rows.is_empty() {
            for (k, v) in &self.rows {
                out.push_str(&format!("  {k}: {v}\n"));
            }
            out.push('\n');
        }
        if !self.log.is_empty() {
            if let Some(t) = &self.log_title {
                out.push_str(t);
                out.push('\n');
            }
            for l in &self.log {
                out.push_str(&format!("  {l}\n"));
            }
            out.push('\n');
        }
        for p in &self.outro {
            out.push_str(p);
            out.push_str("\n\n");
        }
        if let Some(l) = &self.secondary {
            out.push_str(&format!("{}:\n    {}\n\n", l.label, l.url));
        }
        if let Some(n) = &self.note {
            out.push_str(n);
            out.push_str("\n\n");
        }
        out.push_str(&format!("— {server_name}\n{site_url}\n"));
        out
    }

    /// HTML alternative. 600px card on a light background, system font stack,
    /// everything inline — Gmail, Outlook (desktop and web), Apple Mail and
    /// Proton all render this the same way. Colours mirror the app: ink
    /// `#1A1E2B`, muted `#6B7280`, brand green `#2BB884`, background `#F3F4F7`.
    pub fn html(&self, server_name: &str, site_url: &str) -> String {
        const FONT: &str = "font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;";
        const MONO: &str =
            "font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,'Liberation Mono',monospace;";
        let e = esc;
        let mut body = String::new();
        let p = |s: &str| {
            format!(
                "<p style=\"margin:0 0 16px;{FONT}font-size:16px;line-height:24px;color:#1A1E2B\">{}</p>",
                e(s)
            )
        };
        body.push_str(&format!(
            "<h1 style=\"margin:0 0 20px;{FONT}font-size:24px;line-height:32px;font-weight:700;color:#1A1E2B\">{}</h1>",
            e(&self.title)
        ));
        for s in &self.intro {
            body.push_str(&p(s));
        }
        if let Some(code) = &self.code {
            body.push_str(&format!(
                "<table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" width=\"100%\" style=\"margin:8px 0 24px\"><tr>\
                 <td align=\"center\" bgcolor=\"#F3F4F7\" style=\"background:#F3F4F7;border-radius:8px;padding:20px 16px\">\
                 <span style=\"{MONO}font-size:32px;line-height:40px;letter-spacing:6px;font-weight:700;color:#1A1E2B\">{}</span>\
                 </td></tr></table>",
                e(code)
            ));
        }
        if let Some(b) = &self.button {
            body.push_str(&format!(
                "<table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" style=\"margin:8px 0 20px\"><tr>\
                 <td align=\"center\" bgcolor=\"#2BB884\" style=\"background:#2BB884;border-radius:8px\">\
                 <a href=\"{url}\" style=\"display:inline-block;padding:13px 24px;{FONT}font-size:16px;line-height:20px;font-weight:600;color:#FFFFFF;text-decoration:none;border-radius:8px\">{label}</a>\
                 </td></tr></table>\
                 <p style=\"margin:0 0 20px;{FONT}font-size:13px;line-height:20px;color:#6B7280\">If the button does not work, open this link:<br>\
                 <a href=\"{url}\" style=\"color:#2BB884;text-decoration:underline;word-break:break-all\">{url}</a></p>",
                url = e(&b.url),
                label = e(&b.label)
            ));
        }
        if !self.rows.is_empty() {
            body.push_str("<table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" width=\"100%\" style=\"margin:4px 0 20px;border-collapse:collapse\">");
            for (k, v) in &self.rows {
                body.push_str(&format!(
                    "<tr><td style=\"padding:8px 0;border-bottom:1px solid #E5E7EB;{FONT}font-size:15px;line-height:22px;color:#6B7280\">{}</td>\
                     <td align=\"right\" style=\"padding:8px 0;border-bottom:1px solid #E5E7EB;{FONT}font-size:15px;line-height:22px;color:#1A1E2B;font-weight:600\">{}</td></tr>",
                    e(k),
                    e(v)
                ));
            }
            body.push_str("</table>");
        }
        if !self.log.is_empty() {
            if let Some(t) = &self.log_title {
                body.push_str(&format!(
                    "<p style=\"margin:0 0 8px;{FONT}font-size:13px;line-height:18px;font-weight:600;letter-spacing:0.04em;text-transform:uppercase;color:#6B7280\">{}</p>",
                    e(t)
                ));
            }
            body.push_str(&format!(
                "<table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" width=\"100%\" style=\"margin:4px 0 20px\"><tr>\
                 <td bgcolor=\"#12151F\" style=\"background:#12151F;border-radius:8px;padding:14px 16px\">\
                 <pre style=\"margin:0;{MONO}font-size:13px;line-height:20px;color:#D9DEE8;white-space:pre-wrap;word-break:break-word\">{}</pre>\
                 </td></tr></table>",
                self.log.iter().map(|l| e(l)).collect::<Vec<_>>().join("\n")
            ));
        }
        for s in &self.outro {
            body.push_str(&p(s));
        }
        if let Some(l) = &self.secondary {
            body.push_str(&format!(
                "<p style=\"margin:0 0 20px;{FONT}font-size:15px;line-height:22px;color:#1A1E2B\">{}: \
                 <a href=\"{url}\" style=\"color:#2BB884;text-decoration:underline;word-break:break-all\">{url}</a></p>",
                e(&l.label),
                url = e(&l.url)
            ));
        }
        if let Some(n) = &self.note {
            body.push_str(&format!(
                "<p style=\"margin:4px 0 0;padding-top:16px;border-top:1px solid #E5E7EB;{FONT}font-size:13px;line-height:20px;color:#6B7280\">{}</p>",
                e(n)
            ));
        }

        let site = site_url.trim_end_matches('/');
        let host = site
            .trim_start_matches("https://")
            .trim_start_matches("http://");
        format!(
            "<!doctype html>\
             <html lang=\"en\" xmlns=\"http://www.w3.org/1999/xhtml\">\
             <head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
             <meta name=\"color-scheme\" content=\"light\"><meta name=\"supported-color-schemes\" content=\"light\">\
             <title>{subject}</title></head>\
             <body style=\"margin:0;padding:0;background:#F3F4F7;-webkit-text-size-adjust:100%\">\
             <div style=\"display:none;max-height:0;overflow:hidden;opacity:0;color:transparent\">{preheader}</div>\
             <table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" width=\"100%\" bgcolor=\"#F3F4F7\" style=\"background:#F3F4F7\"><tr><td align=\"center\" style=\"padding:32px 16px\">\
             <table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" width=\"600\" style=\"width:100%;max-width:600px\">\
             <tr><td align=\"center\" style=\"padding:0 0 24px\">\
             <a href=\"{site}\" style=\"text-decoration:none\"><img src=\"{site}/email-logo.png\" width=\"180\" height=\"44\" alt=\"{name}\" style=\"display:block;border:0;width:180px;height:44px\"></a>\
             </td></tr>\
             <tr><td bgcolor=\"#FFFFFF\" style=\"background:#FFFFFF;border-radius:12px;padding:36px 40px\">{body}</td></tr>\
             <tr><td align=\"center\" style=\"padding:24px 16px 0;{FONT}font-size:12px;line-height:18px;color:#8E93A8\">\
             This email was sent by {name}. <a href=\"{site}\" style=\"color:#8E93A8;text-decoration:underline\">{host}</a>\
             </td></tr>\
             </table></td></tr></table></body></html>",
            subject = e(&self.subject),
            preheader = e(&self.preheader),
            name = e(server_name),
            host = e(host),
        )
    }
}

impl Mailer {
    pub fn new(cfg: &SmtpConfig, app: &Config) -> anyhow::Result<Self> {
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
        let site_url = app
            .landing_url()
            .unwrap_or(app.web_url())
            .trim_end_matches('/')
            .to_string();
        Ok(Self {
            transport: builder.build(),
            from,
            server_name: app.server_name.clone(),
            site_url,
        })
    }

    pub async fn send(&self, to: &str, email: &Email) -> anyhow::Result<()> {
        let to: Mailbox = to.parse().context("invalid recipient")?;
        let msg = Message::builder()
            .from(self.from.clone())
            .to(to)
            .subject(&email.subject)
            .multipart(MultiPart::alternative_plain_html(
                email.text(&self.server_name, &self.site_url),
                email.html(&self.server_name, &self.site_url),
            ))?;
        self.transport.send(msg).await.context("smtp send")?;
        Ok(())
    }

    /// One-time code for `purpose` ("sign-in verification", "email change", …).
    pub fn code_email(&self, purpose: &str, code: &str, ttl_minutes: u64) -> Email {
        let name = &self.server_name;
        Email::new(
            format!("{name}: {purpose} — code {code}"),
            format!("Your {purpose} code"),
        )
        .preheader(format!("{code} is your {name} code for {purpose}."))
        .paragraph(format!(
            "Enter this code in {name} to continue with {purpose}."
        ))
        .code(code)
        .outro(format!(
            "The code is valid for {ttl_minutes} minutes and works once."
        ))
        .note("If you did not request this, you can ignore this email — nothing changes without the code.")
    }
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `TERMOSO_MAIL_DUMP=/dir cargo test -p termoso-server mail::` writes the
    /// sample emails there for eyeballing in a browser / mail client.
    #[test]
    fn dump_samples() {
        let Ok(dir) = std::env::var("TERMOSO_MAIL_DUMP") else {
            return;
        };
        let name = "Termoso Cloud";
        let site = "https://termoso.dev";
        let code = Email::new("Termoso Cloud: sign-in verification — code 482913", "Your sign-in verification code")
            .preheader("482913 is your Termoso Cloud code for sign-in verification.")
            .paragraph("Enter this code in Termoso Cloud to continue with sign-in verification.")
            .code("482913")
            .outro("The code is valid for 10 minutes and works once.")
            .note("If you did not request this, you can ignore this email — nothing changes without the code.");
        let invite = Email::new("Invitation to Ops", "Join \"Ops\"")
            .paragraph("Alice invited you to join the team \"Ops\" on Termoso Cloud. Accept to get access to the team's shared vaults and hosts.")
            .button("Accept invitation", "https://app.termoso.dev/invite/3f9a…")
            .outro("The invitation expires in 7 days.")
            .note("Not expecting this? You can ignore the email — nothing is shared with you until you accept, and Alice is not notified.");
        let digest = Email::new(
            "Ops: activity for Monday, 14 September 2026",
            "Activity in \"Ops\" — Monday, 14 September 2026",
        )
        .paragraph("3 events by 1 person.")
        .row("Invitations", "1")
        .row("Vaults & access", "1")
        .row("Shared data", "1")
        .log(
            "Timeline (UTC):",
            [
                "09:12  Alice invited bob@example.com (can edit)".to_string(),
                "09:40  Alice gave bob@example.com access (can edit) (in \"Staging\")".to_string(),
                "10:05  Alice added 3 hosts (in \"Staging\")".to_string(),
            ],
        )
        .outro("You receive this because you turned on the activity digest for \"Ops\".")
        .secondary(
            "Change or stop it in the team settings",
            "https://app.termoso.dev/team/0000",
        );
        for (file, e) in [("code", code), ("invite", invite), ("digest", digest)] {
            std::fs::write(format!("{dir}/{file}.html"), e.html(name, site)).unwrap();
            std::fs::write(format!("{dir}/{file}.txt"), e.text(name, site)).unwrap();
        }
    }

    #[test]
    fn html_is_self_contained_and_escaped() {
        let e = Email::new("Subj", "Hello <you>")
            .paragraph("A & B")
            .code("123456")
            .button("Open", "https://app.example.com/x?a=1&b=2")
            .row("Hosts", "3")
            .log("Timeline (UTC)", ["10:00 alice added web-01".to_string()])
            .note("Not you? Ignore.");
        let html = e.html("Termoso Cloud", "https://example.com/");
        assert!(html.contains("Hello &lt;you&gt;"));
        assert!(html.contains("A &amp; B"));
        assert!(html.contains("https://app.example.com/x?a=1&amp;b=2"));
        assert!(html.contains("src=\"https://example.com/email-logo.png\""));
        assert!(html.contains("letter-spacing:6px"), "code box");
        assert!(!html.contains("<script"));
        assert!(!html.contains("<link"));
        assert!(!html.contains("@import"));

        let text = e.text("Termoso Cloud", "https://example.com");
        assert!(text.starts_with(
            "Hello <you>\n\nA & B\n\n    123456\n\nOpen:\n    https://app.example.com/x?a=1&b=2\n"
        ));
        assert!(text.contains("  Hosts: 3\n"));
        assert!(text.contains("Timeline (UTC)\n  10:00 alice added web-01\n"));
        assert!(text.ends_with("— Termoso Cloud\nhttps://example.com\n"));
    }
}
