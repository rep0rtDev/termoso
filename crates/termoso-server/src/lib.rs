//! Termoso server library (the binary is a thin wrapper, tests build the router directly).

#![forbid(unsafe_code)]
// Ad-hoc row tuples for `sqlx::query_as` are clearer at the call site than
// dozens of one-off named types.
#![allow(clippy::type_complexity)]

pub mod audit;
pub mod cache;
pub mod codes;
pub mod config;
pub mod error;
pub mod events;
pub mod extract;
pub mod live;
pub mod mail;
pub mod metrics;
pub mod openapi;
pub mod ratelimit;
pub mod routes;
pub mod session;
pub mod sso;
pub mod state;
pub mod storage;
pub mod users;
pub mod util;
pub mod ws;

pub use state::{AppState, Inner, VERSION};

use axum::Router;

/// Build the full application router for the given state.
pub fn app(state: AppState) -> Router {
    routes::router(state)
}
