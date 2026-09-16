//! Suggestions for the line being typed in a terminal: the shared engine
//! in `termoso_core::autocomplete` fed with this device's command history,
//! the session's vault snippets and a directory listing fetched through the
//! session itself.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use termoso_core::autocomplete::{
    self as engine, DirEntry, PathQuery, SnippetSource, Suggestion, suffix_for,
};
use termoso_core::store::Store;
use uuid::Uuid;

use crate::snippets;

/// Distinct history lines offered at most (the engine caps again).
const HISTORY_LIMIT: usize = 8;
/// How long a fetched directory listing is reused for further keystrokes.
const DIR_CACHE_TTL: Duration = Duration::from_secs(15);
/// Longest wait for a remote listing before paths are simply left out.
pub(crate) const LIST_TIMEOUT: Duration = Duration::from_secs(3);

/// Where a suggestion came from (drives the icon in the strip).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SuggestionKind {
    Command,
    Option,
    Subcommand,
    Path,
    History,
    Snippet,
}

impl From<engine::SuggestionKind> for SuggestionKind {
    fn from(k: engine::SuggestionKind) -> Self {
        match k {
            engine::SuggestionKind::Command => Self::Command,
            engine::SuggestionKind::Option => Self::Option,
            engine::SuggestionKind::Subcommand => Self::Subcommand,
            engine::SuggestionKind::Path => Self::Path,
            engine::SuggestionKind::History => Self::History,
            engine::SuggestionKind::Snippet => Self::Snippet,
        }
    }
}

/// One entry of the suggestions strip.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SuggestionItem {
    pub kind: SuggestionKind,
    /// Full token / line as shown.
    pub label: String,
    /// One-line description (may be empty).
    pub desc: String,
    /// Exactly what to type to accept: the missing part of `label` plus the
    /// trailing space for commands / flags. Never contains a newline.
    pub insert: String,
}

impl From<Suggestion> for SuggestionItem {
    fn from(s: Suggestion) -> Self {
        let insert = format!("{}{}", s.insert, suffix_for(s.kind));
        Self {
            kind: s.kind.into(),
            label: s.label,
            desc: s.desc,
            insert,
        }
    }
}

struct DirCache {
    dir: String,
    at: Instant,
    entries: Vec<DirEntry>,
}

/// Per-session completion state: remembers the last directory listing so
/// typing inside one directory costs a single round trip.
pub(crate) struct Completer {
    dirs: Mutex<Option<DirCache>>,
}

impl Completer {
    pub(crate) fn new() -> Self {
        Self {
            dirs: Mutex::new(None),
        }
    }

    /// Everything that can be answered without the session: history,
    /// snippets and the catalogue. Returns the items and, when the token may
    /// be a path, the directory to list. A span covering several screen
    /// lines is not a command line any more (the shell printed a completion
    /// list or a prompt in between) and gets nothing.
    pub(crate) fn offline(
        store: &Store,
        vault_id: Option<Uuid>,
        line: &str,
    ) -> (Vec<Suggestion>, Option<PathQuery>) {
        if line.trim().is_empty() || line.contains('\n') {
            return (Vec::new(), None);
        }
        let history = store
            .complete_command(line.trim_start(), HISTORY_LIMIT)
            .unwrap_or_default();
        let snippets: Vec<SnippetSource> = snippets::list(store, &vault_id.map(|v| v.to_string()))
            .unwrap_or_default()
            .into_iter()
            .map(|s| SnippetSource {
                label: s.label,
                script: s.script,
            })
            .collect();
        let completion = engine::complete(line, &history, &snippets);
        (completion.items, completion.path)
    }

    /// Cached listing of `dir`, if still fresh.
    pub(crate) fn cached(&self, dir: &str) -> Option<Vec<DirEntry>> {
        let guard = self.dirs.lock().expect("dir cache poisoned");
        guard
            .as_ref()
            .filter(|c| c.dir == dir && c.at.elapsed() < DIR_CACHE_TTL)
            .map(|c| c.entries.clone())
    }

    pub(crate) fn remember(&self, dir: &str, entries: Vec<DirEntry>) {
        *self.dirs.lock().expect("dir cache poisoned") = Some(DirCache {
            dir: dir.to_string(),
            at: Instant::now(),
            entries,
        });
    }

    /// Drop the cached listing (the command line was submitted, so the
    /// working directory may have changed).
    pub(crate) fn invalidate(&self) {
        *self.dirs.lock().expect("dir cache poisoned") = None;
    }
}

/// Combine the offline items with a listing into the final strip.
pub(crate) fn finish(
    items: Vec<Suggestion>,
    path: Option<(&PathQuery, Vec<DirEntry>)>,
) -> Vec<SuggestionItem> {
    let merged = match path {
        Some((query, entries)) => engine::merge_paths(&items, query, &entries),
        None => items,
    };
    merged.into_iter().map(Into::into).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use termoso_core::autocomplete::SuggestionKind as K;

    #[test]
    fn insert_carries_suffix() {
        let item: SuggestionItem = Suggestion {
            kind: K::Command,
            label: "git".into(),
            desc: "vcs".into(),
            insert: "t".into(),
        }
        .into();
        assert_eq!(item.insert, "t ");
        assert_eq!(item.kind, SuggestionKind::Command);
        let hist: SuggestionItem = Suggestion {
            kind: K::History,
            label: "git status".into(),
            desc: String::new(),
            insert: " status".into(),
        }
        .into();
        assert_eq!(hist.insert, " status");
    }

    #[test]
    fn dir_cache_round_trip() {
        let c = Completer::new();
        assert!(c.cached("src/").is_none());
        c.remember(
            "src/",
            vec![DirEntry {
                name: "main.rs".into(),
                dir: false,
            }],
        );
        assert_eq!(c.cached("src/").map(|e| e.len()), Some(1));
        assert!(c.cached("lib/").is_none());
        c.invalidate();
        assert!(c.cached("src/").is_none());
    }

    #[test]
    fn offline_uses_history_and_vault_snippets() {
        use termoso_core::store::CommandHistory;
        use termoso_crypto::keys::SymmetricKey;

        let store = Store::open_in_memory(SymmetricKey::generate()).expect("store");
        let vault = store.local_vault().unwrap().id;
        for c in ["git status", "git push origin main", "ls -la"] {
            store
                .record_command(&CommandHistory {
                    host_id: None,
                    command: c.into(),
                })
                .unwrap();
        }
        for (label, script) in [
            ("Disk", "df -h"),
            ("Two lines", "cd /var\r\nls"),
            ("Git log", "git log --oneline"),
        ] {
            snippets::save(
                &store,
                &snippets::SnippetDraft {
                    id: None,
                    vault_id: vault.to_string(),
                    label: label.into(),
                    script: script.into(),
                    package_id: None,
                    close_after_run: false,
                    target_host_ids: Vec::new(),
                },
            )
            .unwrap();
        }

        let (items, path) = Completer::offline(&store, Some(vault), "gi");
        assert!(path.is_none());
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        // Most recent history first, then the snippet, then the catalogue.
        assert_eq!(labels[0], "git push origin main");
        assert_eq!(labels[1], "git status");
        assert!(items.iter().any(|i| i.kind == K::Snippet
            && i.label == "git log --oneline"
            && i.desc == "Git log"));
        assert!(
            items
                .iter()
                .any(|i| i.kind == K::Command && i.label == "git")
        );
        assert_eq!(items[0].insert, "t push origin main");

        // The multi-line snippet never reaches the one-line strip.
        let (items, _) = Completer::offline(&store, Some(vault), "cd");
        assert!(items.iter().all(|i| i.desc != "Two lines"));
        // Another vault sees no snippets of this one.
        let (items, _) = Completer::offline(&store, Some(Uuid::new_v4()), "df");
        assert!(items.iter().all(|i| i.kind != K::Snippet));
        // Empty input: nothing.
        assert!(Completer::offline(&store, Some(vault), "   ").0.is_empty());
        // A span that grew over several screen lines is not a command line.
        assert!(
            Completer::offline(&store, Some(vault), "git\ncommit  config\ngi")
                .0
                .is_empty()
        );
        // A path token asks for a listing.
        let (_, path) = Completer::offline(&store, Some(vault), "cat src/ma");
        let q = path.expect("path query");
        assert_eq!((q.dir.as_str(), q.prefix.as_str()), ("src/", "ma"));
    }

    #[test]
    fn finish_merges_paths() {
        let query = PathQuery {
            dir: "src/".into(),
            prefix: "m".into(),
            dirs_only: false,
        };
        let out = finish(
            Vec::new(),
            Some((
                &query,
                vec![
                    DirEntry {
                        name: "main.rs".into(),
                        dir: false,
                    },
                    DirEntry {
                        name: "mod".into(),
                        dir: true,
                    },
                ],
            )),
        );
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].label, "src/mod/");
        assert_eq!(out[0].insert, "od/");
        assert_eq!(out[1].insert, "ain.rs ");
    }
}
