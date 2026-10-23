//! One-time codes for keyboard-interactive logins: an identity can carry a
//! Totp secret, and [`TotpResponder`] types the current code into the prompt
//! that asks for it (PAM `google-authenticator`, Duo, FreeIPA, routers),
//! the password into hidden prompts, and hands anything else to the UI.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use totp_rs::{Algorithm, Builder, Secret, Totp};
use zeroize::Zeroizing;

use super::auth::{InteractivePrompt, InteractiveQuestion};
use crate::error::{CoreError, Result};

/// Parse an `otpauth://totp/...` URI or a bare base32 secret.
pub fn parse(raw: &str) -> Result<Totp> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(CoreError::Invalid("Totp secret is empty".into()));
    }
    if raw.starts_with("otpauth://") {
        return Totp::from_url_unchecked(raw)
            .map_err(|e| CoreError::Invalid(format!("Totp URI: {e}")));
    }
    let compact: String = raw
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect::<String>()
        .to_ascii_uppercase();
    let secret = Secret::try_from_base32(&compact)
        .map_err(|e| CoreError::Invalid(format!("TOTP secret is not base32: {e}")))?;
    if secret.as_ref().is_empty() {
        return Err(CoreError::Invalid("TOTP secret is empty".into()));
    }
    Ok(Builder::new()
        .with_algorithm(Algorithm::SHA1)
        .with_digits(6)
        .with_skew(1)
        .with_step_duration(30)
        .with_secret(secret)
        .build_noncompliant())
}

/// Validate a secret before storing it; returns the trimmed input (the URI
/// keeps issuer / digits / period, a bare secret stays as typed).
pub fn normalize_secret(raw: &str) -> Result<String> {
    parse(raw)?;
    Ok(raw.trim().to_string())
}

/// Does this prompt ask for a one-time code?
pub fn asks_for_code(prompt: &str) -> bool {
    let p = prompt.to_ascii_lowercase();
    const HINTS: &[&str] = &[
        "verification code",
        "one-time",
        "one time",
        "otp",
        "totp",
        "token",
        "authenticator",
        "2fa",
        "two-factor",
        "two factor",
        "second factor",
        "mfa",
        "passcode",
        "duo",
        "код", // ru: "введите код"
    ];
    if HINTS.iter().any(|h| p.contains(h)) {
        return true;
    }
    if p.contains("password") || p.contains("passphrase") {
        // "Password or code:"-style prompts stay with the password.
        return false;
    }
    p.trim_end_matches([':', ' ']).ends_with("code")
}

/// Answers code prompts with the current Totp, hidden prompts with the
/// identity password when there is one, and asks `fallback` for the rest.
pub struct TotpResponder {
    totp: Arc<Totp>,
    password: Option<Zeroizing<String>>,
    fallback: Option<Arc<dyn InteractivePrompt>>,
}

impl TotpResponder {
    /// `fallback` answers prompts that are neither a code nor (with a
    /// password) hidden; `None` cancels those rounds.
    pub fn new(
        totp: Arc<Totp>,
        password: Option<Zeroizing<String>>,
        fallback: Option<Arc<dyn InteractivePrompt>>,
    ) -> Self {
        Self {
            totp,
            password,
            fallback,
        }
    }

    /// Parse a stored secret into the shared generator.
    pub fn parse(secret: &str) -> Result<Arc<Totp>> {
        parse(secret).map(Arc::new)
    }

    fn code(&self) -> String {
        self.totp.generate_current().to_string()
    }
}

impl InteractivePrompt for TotpResponder {
    fn respond(
        &self,
        name: String,
        instructions: String,
        prompts: Vec<InteractiveQuestion>,
    ) -> Pin<Box<dyn Future<Output = Option<Vec<String>>> + Send + '_>> {
        Box::pin(async move {
            let mut answers: Vec<Option<String>> = Vec::with_capacity(prompts.len());
            let mut open = Vec::new();
            for (i, q) in prompts.iter().enumerate() {
                if asks_for_code(&q.prompt) {
                    answers.push(Some(self.code()));
                } else if !q.echo
                    && let Some(pw) = &self.password
                {
                    answers.push(Some(pw.to_string()));
                } else {
                    answers.push(None);
                    open.push(i);
                }
            }
            if !open.is_empty() {
                let fallback = self.fallback.as_ref()?;
                let asked = open.iter().map(|&i| prompts[i].clone()).collect();
                let typed = fallback.respond(name, instructions, asked).await?;
                if typed.len() != open.len() {
                    return None;
                }
                for (i, v) in open.into_iter().zip(typed) {
                    answers[i] = Some(v);
                }
            }
            answers.into_iter().collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "JBSWY3DPEHPK3PXP";

    struct Fixed(Vec<String>);
    impl InteractivePrompt for Fixed {
        fn respond(
            &self,
            _n: String,
            _i: String,
            prompts: Vec<InteractiveQuestion>,
        ) -> Pin<Box<dyn Future<Output = Option<Vec<String>>> + Send + '_>> {
            assert_eq!(prompts.len(), self.0.len());
            Box::pin(async move { Some(self.0.clone()) })
        }
    }

    fn q(prompt: &str, echo: bool) -> InteractiveQuestion {
        InteractiveQuestion {
            prompt: prompt.into(),
            echo,
        }
    }

    #[test]
    fn parses_base32_and_uri() {
        assert!(parse(SECRET).is_ok());
        assert!(parse("jbsw y3dp ehpk 3pxp").is_ok());
        assert!(
            parse("otpauth://totp/Termoso:root@host?secret=JBSWY3DPEHPK3PXP&issuer=Termoso")
                .is_ok()
        );
        assert!(parse("").is_err());
        assert!(parse("not base32!!").is_err());
        assert_eq!(normalize_secret("  JBSWY3DPEHPK3PXP ").unwrap(), SECRET);
    }

    #[test]
    fn recognises_code_prompts() {
        for p in [
            "Verification code: ",
            "One-time password (OATH) for root:",
            "Enter your Totp token",
            "Duo two-factor login for admin",
            "Authenticator code",
            "Введите код:",
        ] {
            assert!(asks_for_code(p), "{p}");
        }
        for p in [
            "Password: ",
            "Password or code:",
            "Username:",
            "Passphrase for key:",
        ] {
            assert!(!asks_for_code(p), "{p}");
        }
    }

    #[tokio::test]
    async fn fills_code_password_and_delegates_rest() {
        let totp = Arc::new(parse(SECRET).unwrap());
        let expected = totp.generate_current().to_string();
        let r = TotpResponder::new(
            totp,
            Some(Zeroizing::new("pw".into())),
            Some(Arc::new(Fixed(vec!["alice".into()]))),
        );
        let out = r
            .respond(
                String::new(),
                String::new(),
                vec![
                    q("Password: ", false),
                    q("Verification code: ", false),
                    q("Username: ", true),
                ],
            )
            .await
            .unwrap();
        assert_eq!(out, vec!["pw".to_string(), expected, "alice".to_string()]);
    }

    #[tokio::test]
    async fn no_fallback_means_cancel() {
        let r = TotpResponder::new(Arc::new(parse(SECRET).unwrap()), None, None);
        assert!(
            r.respond(String::new(), String::new(), vec![q("Password: ", false)])
                .await
                .is_none()
        );
        assert!(
            r.respond(String::new(), String::new(), vec![q("Code: ", false)])
                .await
                .is_some()
        );
    }
}
