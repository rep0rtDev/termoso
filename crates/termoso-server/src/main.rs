use std::net::SocketAddr;

use anyhow::Context;
use termoso_server::config::{Config, LogFormat};
use termoso_server::{Inner, metrics};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Config::load()?;
    if std::env::args().any(|a| a == "--healthcheck") {
        return healthcheck(&cfg).await;
    }
    init_tracing(&cfg);

    let metrics_handle = if cfg.metrics.enabled {
        Some(metrics::install()?)
    } else {
        None
    };

    let state = Inner::build(cfg.clone()).await?;
    let app = termoso_server::app(state.clone());

    if let Some(handle) = metrics_handle {
        let bind = cfg.metrics.bind;
        tokio::spawn(async move {
            if let Err(e) = metrics::serve(bind, handle).await {
                tracing::error!(error = %e, "metrics listener failed");
            }
        });
    }
    tokio::spawn(termoso_server::ws::run_fanout(state.clone()));

    let listener = tokio::net::TcpListener::bind(cfg.bind)
        .await
        .with_context(|| format!("binding {}", cfg.bind))?;
    tracing::info!(
        version = termoso_server::VERSION,
        bind = %cfg.bind,
        public_url = %cfg.public_url,
        s3 = state.storage.is_some(),
        smtp = state.mailer.is_some(),
        webauthn = state.webauthn.is_some(),
        sso = !state.sso.is_empty(),
        "termoso-server started"
    );
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

/// Container health probe (no curl in the runtime image): `GET /readyz` on the
/// configured bind port, exit 0 iff 200.
async fn healthcheck(cfg: &Config) -> anyhow::Result<()> {
    let host = if cfg.bind.ip().is_unspecified() {
        "127.0.0.1".to_string()
    } else {
        cfg.bind.ip().to_string()
    };
    let url = format!("http://{host}:{}/readyz", cfg.bind.port());
    let status = reqwest::Client::new()
        .get(&url)
        .timeout(std::time::Duration::from_secs(4))
        .send()
        .await
        .with_context(|| format!("GET {url}"))?
        .status();
    anyhow::ensure!(status.is_success(), "{url} returned {status}");
    Ok(())
}

fn init_tracing(cfg: &Config) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&cfg.log));
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    match cfg.log_format {
        LogFormat::Json => builder.json().init(),
        LogFormat::Text => builder.init(),
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutting down");
}
