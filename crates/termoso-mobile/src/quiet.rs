//! Quiet snippet run: the expanded script goes to each host over a fresh
//! `exec` channel — no terminal, no session — and stdout / stderr / exit code
//! come back per host. The counterpart of the desktop «Run quietly…».

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use termoso_core::model::Host;
use termoso_core::store::Store;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::connect::{
    ConnectStage, ConnectUi, Connector, PromptAnswer, PromptRequest, SecretCache, connect_resolved,
};
use crate::dto::parse_id;
use crate::error::{MobileError, Result};
use crate::settings::MobileSettings;
use crate::snippets;

/// Upper bound per host: connect + command.
const TIMEOUT: Duration = Duration::from_secs(120);
/// Output kept per stream per host; the rest is dropped with a marker.
const OUTPUT_CAP: usize = 64 * 1024;
/// Hosts worked on at the same time.
const PARALLEL: usize = 4;

/// One host of a quiet run.
#[derive(Debug, Clone, uniffi::Record)]
pub struct QuietHostResult {
    pub host_id: String,
    pub label: String,
    /// `user@host:port` the command ran on (empty when connecting failed).
    pub target: String,
    pub exit_code: Option<u32>,
    pub stdout: String,
    pub stderr: String,
    /// Connection or transport error; the command never ran.
    pub error: Option<String>,
    pub ms: u64,
}

/// Callbacks arrive on tokio threads; post to the main thread, do not block.
#[uniffi::export(with_foreign)]
pub trait QuietRunListener: Send + Sync {
    /// One host finished (ok or not).
    fn on_result(&self, result: QuietHostResult);
    /// Every host finished (or the run was cancelled).
    fn on_done(&self);
    /// Answer with [`QuietRun::answer`] using the same `prompt_id`.
    fn on_prompt(&self, prompt_id: u64, request: PromptRequest);
}

struct QuietUi {
    listener: Arc<dyn QuietRunListener>,
}

impl ConnectUi for QuietUi {
    fn phase(&self, _stage: ConnectStage, _hop: Option<String>) {}

    fn prompt(&self, prompt_id: u64, request: PromptRequest) {
        self.listener.on_prompt(prompt_id, request);
    }
}

pub(crate) struct QuietLaunch {
    pub store: Arc<Store>,
    pub secrets: Arc<SecretCache>,
    pub snippet_id: Uuid,
    pub host_ids: Vec<Uuid>,
    pub vars: HashMap<String, String>,
    pub settings: MobileSettings,
    pub listener: Arc<dyn QuietRunListener>,
}

/// A quiet run in flight. Results stream through the listener and stay
/// readable from [`QuietRun::results`] afterwards.
#[derive(uniffi::Object)]
pub struct QuietRun {
    command: String,
    conn: Arc<Connector>,
    results: Arc<Mutex<Vec<QuietHostResult>>>,
    cancelled: CancellationToken,
}

fn cap(bytes: &[u8]) -> String {
    if bytes.len() <= OUTPUT_CAP {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    let mut s = String::from_utf8_lossy(&bytes[..OUTPUT_CAP]).into_owned();
    s.push_str(&format!(
        "\n… [{} more bytes dropped]",
        bytes.len() - OUTPUT_CAP
    ));
    s
}

impl QuietRun {
    pub(crate) fn launch(
        runtime: tokio::runtime::Handle,
        launch: QuietLaunch,
    ) -> Result<Arc<Self>> {
        let QuietLaunch {
            store,
            secrets,
            snippet_id,
            host_ids,
            vars,
            settings,
            listener,
        } = launch;
        if host_ids.is_empty() {
            return Err(MobileError::invalid("pick at least one host"));
        }
        let snippet = store.require::<termoso_core::model::Snippet>(snippet_id)?;
        let (command, _) = snippets::prepare(&store, snippet_id, &vars, false)?;
        let vault_id = snippet.vault_id;
        let conn = Arc::new(Connector::new(
            store.clone(),
            secrets,
            Arc::new(QuietUi {
                listener: listener.clone(),
            }),
        ));
        let run = Arc::new(Self {
            command: command.clone(),
            conn: conn.clone(),
            results: Arc::new(Mutex::new(Vec::with_capacity(host_ids.len()))),
            cancelled: CancellationToken::new(),
        });
        let results = run.results.clone();
        let cancelled = run.cancelled.clone();
        let settings = Arc::new(settings);
        runtime.spawn(async move {
            let sem = Arc::new(Semaphore::new(PARALLEL));
            let mut handles = Vec::with_capacity(host_ids.len());
            for host_id in host_ids {
                let (store, conn, settings, command, cancelled) = (
                    store.clone(),
                    conn.clone(),
                    settings.clone(),
                    command.clone(),
                    cancelled.clone(),
                );
                let sem = sem.clone();
                handles.push(tokio::spawn(async move {
                    let _permit = sem.acquire().await;
                    let started = Instant::now();
                    let mut result = QuietHostResult {
                        host_id: host_id.to_string(),
                        label: store
                            .require::<Host>(host_id)
                            .map(|h| h.data.label.clone())
                            .unwrap_or_default(),
                        target: String::new(),
                        exit_code: None,
                        stdout: String::new(),
                        stderr: String::new(),
                        error: None,
                        ms: 0,
                    };
                    if cancelled.is_cancelled() {
                        result.error = Some("cancelled".into());
                        return result;
                    }
                    let work = async {
                        let resolved = store.resolve_host_in(host_id, Some(vault_id))?;
                        if resolved.protocol() != "ssh" {
                            return Err(MobileError::invalid(format!(
                                "{} hosts cannot run commands quietly",
                                resolved.protocol()
                            )));
                        }
                        let target = crate::app::ssh_target(&resolved);
                        let display = target.display();
                        let (client, jumps) =
                            connect_resolved(&conn, &settings, target, Some(&resolved)).await?;
                        let out = client.exec(&command, None).await;
                        let _ = client.disconnect().await;
                        for j in jumps {
                            let _ = j.disconnect().await;
                        }
                        Ok::<_, MobileError>((display, out?))
                    };
                    let outcome = tokio::select! {
                        r = tokio::time::timeout(TIMEOUT, work) => r,
                        _ = cancelled.cancelled() => Ok(Err(MobileError::invalid("cancelled"))),
                    };
                    match outcome {
                        Ok(Ok((display, out))) => {
                            result.target = display;
                            result.exit_code = out.exit_code;
                            result.stdout = cap(&out.stdout);
                            result.stderr = cap(&out.stderr);
                        }
                        Ok(Err(e)) => result.error = Some(conn.map_error(e).to_string()),
                        Err(_) => result.error = Some("timed out".into()),
                    }
                    result.ms = started.elapsed().as_millis() as u64;
                    result
                }));
            }
            for h in handles {
                if let Ok(r) = h.await {
                    results.lock().expect("results poisoned").push(r.clone());
                    listener.on_result(r);
                }
            }
            listener.on_done();
        });
        Ok(run)
    }
}

#[uniffi::export]
impl QuietRun {
    /// The expanded script that is being run.
    pub fn command(&self) -> String {
        self.command.clone()
    }

    /// Results of the hosts that finished so far.
    pub fn results(&self) -> Vec<QuietHostResult> {
        self.results.lock().expect("results poisoned").clone()
    }

    /// Reply to a prompt raised through the listener.
    pub fn answer(&self, prompt_id: u64, answer: PromptAnswer) -> bool {
        self.conn.answer(prompt_id, answer)
    }

    /// Stop: hosts not started yet are reported as cancelled, pending prompts fail.
    pub fn cancel(&self) {
        self.conn.cancel_prompts();
        self.cancelled.cancel();
    }
}

pub(crate) fn parse_ids(ids: &[String]) -> Result<Vec<Uuid>> {
    ids.iter().map(|s| parse_id(s)).collect()
}
