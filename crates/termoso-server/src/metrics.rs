//! Opt-in operator metrics (Prometheus text format) on a *separate* listener.
//!
//! Disabled by default. Nothing is ever sent anywhere; this only exposes
//! counters to whoever can reach `TERMOSO_METRICS__BIND` on the operator's
//! own network.

use std::net::SocketAddr;

use axum::Router;
use axum::routing::get;
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

pub fn install() -> anyhow::Result<PrometheusHandle> {
    Ok(PrometheusBuilder::new().install_recorder()?)
}

pub async fn serve(bind: SocketAddr, handle: PrometheusHandle) -> anyhow::Result<()> {
    let app = Router::new().route(
        "/metrics",
        get(move || {
            let h = handle.clone();
            async move { h.render() }
        }),
    );
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!(%bind, "metrics listener started (opt-in)");
    axum::serve(listener, app).await?;
    Ok(())
}
