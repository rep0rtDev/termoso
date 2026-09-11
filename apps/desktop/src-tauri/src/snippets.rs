//! Snippets and snippet packages. Rust stores, validates and expands
//! `{{variables}}`, and types the result into live sessions; the webview only
//! renders the tree and collects variable values.

use std::collections::{BTreeSet, HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use termoso_core::model::{Host, HostSnippet, Snippet, SnippetPackage};
use termoso_core::store::Store;
use uuid::Uuid;

use crate::error::{DesktopError, Result};
use crate::state::AppState;

const MAX_SCRIPT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnippetCard {
    pub id: Uuid,
    pub vault_id: Uuid,
    pub label: String,
    pub script: String,
    pub package_id: Option<Uuid>,
    pub close_after_run: bool,
    pub sort_order: i32,
    /// `{{name}}` placeholders in the script, in order of first appearance.
    pub variables: Vec<String>,
    pub updated_at: DateTime<Utc>,
    pub dirty: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnippetForm {
    pub id: Option<Uuid>,
    pub vault_id: Uuid,
    pub label: String,
    pub script: String,
    #[serde(default)]
    pub package_id: Option<Uuid>,
    #[serde(default)]
    pub close_after_run: bool,
    #[serde(default)]
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageNode {
    pub id: Uuid,
    pub vault_id: Uuid,
    pub label: String,
    pub parent_id: Option<Uuid>,
    pub snippet_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunResult {
    /// Sessions the script was typed into.
    pub session_ids: Vec<Uuid>,
    /// The snippet asks for its terminal to be closed once the command ran.
    pub close_after_run: bool,
}

/// `{{name}}` placeholders, deduplicated, in first-appearance order.
pub fn variables(script: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let mut rest = script;
    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        match after.find("}}") {
            Some(end) => {
                let name = after[..end].trim();
                if !name.is_empty()
                    && !name.contains(['{', '}', '\n'])
                    && seen.insert(name.to_string())
                {
                    out.push(name.to_string());
                }
                rest = &after[end + 2..];
            }
            None => break,
        }
    }
    out
}

/// Substitute placeholders. Every variable must be provided.
pub fn expand(script: &str, vars: &HashMap<String, String>) -> Result<String> {
    let names = variables(script);
    let missing: BTreeSet<&str> = names
        .iter()
        .map(String::as_str)
        .filter(|n| !vars.contains_key(*n))
        .collect();
    if !missing.is_empty() {
        return Err(DesktopError::invalid(format!(
            "missing snippet variables: {}",
            missing.into_iter().collect::<Vec<_>>().join(", ")
        )));
    }
    let mut out = String::with_capacity(script.len());
    let mut rest = script;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find("}}") {
            Some(end) => {
                let name = after[..end].trim();
                match vars.get(name) {
                    Some(v) if names.iter().any(|n| n == name) => out.push_str(v),
                    _ => out.push_str(&rest[start..start + 2 + end + 2]),
                }
                rest = &after[end + 2..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    Ok(out)
}

fn card(e: termoso_core::model::Entity<Snippet>) -> SnippetCard {
    SnippetCard {
        id: e.id,
        vault_id: e.vault_id,
        variables: variables(&e.data.script),
        label: e.data.label,
        script: e.data.script,
        package_id: e.data.package_id,
        close_after_run: e.data.close_after_run,
        sort_order: e.data.sort_order,
        updated_at: e.updated_at,
        dirty: e.dirty,
    }
}

pub fn list(store: &Store, vault_id: Option<Uuid>) -> Result<Vec<SnippetCard>> {
    let mut out: Vec<SnippetCard> = store
        .list::<Snippet>(vault_id)?
        .into_iter()
        .map(card)
        .collect();
    out.sort_by(|a, b| {
        a.sort_order
            .cmp(&b.sort_order)
            .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
    });
    Ok(out)
}

fn validate(store: &Store, form: &SnippetForm) -> Result<Snippet> {
    let label = form.label.trim();
    if label.is_empty() {
        return Err(DesktopError::invalid("snippet label is required"));
    }
    if form.script.trim().is_empty() {
        return Err(DesktopError::invalid("snippet script is empty"));
    }
    if form.script.len() > MAX_SCRIPT_BYTES {
        return Err(DesktopError::invalid("snippet script is too large"));
    }
    if let Some(p) = form.package_id {
        let pkg = store.require::<SnippetPackage>(p)?;
        if pkg.vault_id != form.vault_id {
            return Err(DesktopError::invalid("package belongs to another vault"));
        }
    }
    Ok(Snippet {
        label: label.to_string(),
        script: form.script.clone(),
        package_id: form.package_id,
        close_after_run: form.close_after_run,
        sort_order: form.sort_order,
    })
}

pub fn save(store: &Store, form: &SnippetForm) -> Result<SnippetCard> {
    let data = validate(store, form)?;
    let id = match form.id {
        Some(id) => {
            store.require::<Snippet>(id)?;
            store.update(id, &data)?;
            id
        }
        None => store.insert(form.vault_id, &data)?,
    };
    Ok(card(store.require::<Snippet>(id)?))
}

/// Delete a snippet and every host binding / startup reference to it.
pub fn delete(store: &Store, id: Uuid) -> Result<()> {
    let e = store.require::<Snippet>(id)?;
    for hs in store.list::<HostSnippet>(Some(e.vault_id))? {
        if hs.data.snippet_id == id {
            store.delete(hs.id)?;
        }
    }
    for mut host in store.list::<Host>(Some(e.vault_id))? {
        if host.data.startup_snippet_id == Some(id) {
            host.data.startup_snippet_id = None;
            store.update(host.id, &host.data)?;
        }
    }
    store.delete(id)?;
    Ok(())
}

// ───────────────────────────── packages ─────────────────────────────

pub fn packages(store: &Store, vault_id: Option<Uuid>) -> Result<Vec<PackageNode>> {
    let mut counts: HashMap<Uuid, usize> = HashMap::new();
    for s in store.list::<Snippet>(vault_id)? {
        if let Some(p) = s.data.package_id {
            *counts.entry(p).or_insert(0) += 1;
        }
    }
    let mut out: Vec<PackageNode> = store
        .list::<SnippetPackage>(vault_id)?
        .into_iter()
        .map(|p| PackageNode {
            id: p.id,
            vault_id: p.vault_id,
            snippet_count: counts.get(&p.id).copied().unwrap_or(0),
            label: p.data.label,
            parent_id: p.data.parent_id,
        })
        .collect();
    out.sort_by_key(|a| a.label.to_lowercase());
    Ok(out)
}

fn is_descendant(store: &Store, candidate: Uuid, ancestor: Uuid) -> Result<bool> {
    let mut cur = Some(candidate);
    let mut hops = 0;
    while let Some(id) = cur {
        if id == ancestor {
            return Ok(true);
        }
        hops += 1;
        if hops > 64 {
            return Err(DesktopError::invalid("package tree is too deep"));
        }
        cur = store.require::<SnippetPackage>(id)?.data.parent_id;
    }
    Ok(false)
}

pub fn save_package(
    store: &Store,
    vault_id: Uuid,
    id: Option<Uuid>,
    label: &str,
    parent_id: Option<Uuid>,
) -> Result<PackageNode> {
    let label = label.trim();
    if label.is_empty() {
        return Err(DesktopError::invalid("package label is required"));
    }
    if let Some(p) = parent_id {
        let parent = store.require::<SnippetPackage>(p)?;
        if parent.vault_id != vault_id {
            return Err(DesktopError::invalid("parent belongs to another vault"));
        }
        if let Some(me) = id
            && is_descendant(store, p, me)?
        {
            return Err(DesktopError::invalid(
                "a package cannot be moved into itself",
            ));
        }
    }
    let data = SnippetPackage {
        label: label.to_string(),
        parent_id,
    };
    let id = match id {
        Some(id) => {
            store.require::<SnippetPackage>(id)?;
            store.update(id, &data)?;
            id
        }
        None => store.insert(vault_id, &data)?,
    };
    packages(store, Some(vault_id))?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| DesktopError::not_found(format!("package {id}")))
}

/// Delete a package; its snippets and sub-packages move to the parent.
pub fn delete_package(store: &Store, id: Uuid) -> Result<()> {
    let pkg = store.require::<SnippetPackage>(id)?;
    let parent = pkg.data.parent_id;
    for mut s in store.list::<Snippet>(Some(pkg.vault_id))? {
        if s.data.package_id == Some(id) {
            s.data.package_id = parent;
            store.update(s.id, &s.data)?;
        }
    }
    for mut child in store.list::<SnippetPackage>(Some(pkg.vault_id))? {
        if child.data.parent_id == Some(id) {
            child.data.parent_id = parent;
            store.update(child.id, &child.data)?;
        }
    }
    store.delete(id)?;
    Ok(())
}

// ───────────────────────────── run ─────────────────────────────

/// Type the expanded script (plus a trailing newline) into each session.
pub async fn run(
    state: &AppState,
    snippet_id: Uuid,
    session_ids: &[Uuid],
    vars: &HashMap<String, String>,
) -> Result<RunResult> {
    let snippet = state.store.require::<Snippet>(snippet_id)?;
    let text = script_to_send(&expand(&snippet.data.script, vars)?);
    if session_ids.is_empty() {
        return Err(DesktopError::invalid("pick at least one session"));
    }
    let mut done = Vec::new();
    for &sid in session_ids {
        let term = state.sessions.terminal(sid)?;
        term.write(text.as_bytes()).await?;
        done.push(sid);
    }
    Ok(RunResult {
        session_ids: done,
        close_after_run: snippet.data.close_after_run,
    })
}

/// Normalise line endings and make sure the last line is executed.
pub fn script_to_send(script: &str) -> String {
    let mut s = script.replace("\r\n", "\n");
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use termoso_core::termoso_crypto::keys::SymmetricKey;

    fn store() -> Store {
        Store::open_in_memory(SymmetricKey::generate()).expect("store")
    }

    #[test]
    fn variables_are_extracted_once_in_order() {
        assert_eq!(
            variables("ssh {{ user }}@{{host}} -p {{port}} && echo {{user}} {{ }} {{a{b}}"),
            vec!["user", "host", "port"]
        );
        assert!(variables("no vars {{unterminated").is_empty());
    }

    #[test]
    fn expand_requires_every_variable() {
        let mut vars = HashMap::new();
        vars.insert("user".to_string(), "root".to_string());
        assert!(expand("{{user}}@{{host}}", &vars).is_err());
        vars.insert("host".to_string(), "db".to_string());
        assert_eq!(expand("{{ user }}@{{host}}", &vars).unwrap(), "root@db");
        assert_eq!(expand("plain", &vars).unwrap(), "plain");
        assert_eq!(script_to_send("a\r\nb"), "a\nb\n");
        assert_eq!(script_to_send("a\n"), "a\n");
    }

    #[test]
    fn packages_and_snippets_lifecycle() {
        let store = store();
        let vault = store.local_vault().unwrap().id;
        let root = save_package(&store, vault, None, "ops", None).unwrap();
        let child = save_package(&store, vault, None, "db", Some(root.id)).unwrap();
        assert!(save_package(&store, vault, Some(root.id), "ops", Some(child.id)).is_err());

        let s = save(
            &store,
            &SnippetForm {
                id: None,
                vault_id: vault,
                label: "restart".into(),
                script: "systemctl restart {{svc}}".into(),
                package_id: Some(child.id),
                close_after_run: true,
                sort_order: 0,
            },
        )
        .unwrap();
        assert_eq!(s.variables, vec!["svc"]);
        assert_eq!(packages(&store, Some(vault)).unwrap()[0].snippet_count, 1);
        assert!(
            save(
                &store,
                &SnippetForm {
                    id: None,
                    vault_id: vault,
                    label: "empty".into(),
                    script: "  ".into(),
                    package_id: None,
                    close_after_run: false,
                    sort_order: 0,
                },
            )
            .is_err()
        );

        // Deleting the child package lifts its snippet to the parent.
        delete_package(&store, child.id).unwrap();
        let s2 = list(&store, Some(vault)).unwrap();
        assert_eq!(s2.len(), 1);
        assert_eq!(s2[0].package_id, Some(root.id));

        // Host startup reference is cleared on snippet delete.
        let host_id = store
            .insert(
                vault,
                &Host {
                    label: "h".into(),
                    address: "h".into(),
                    startup_snippet_id: Some(s.id),
                    ..Default::default()
                },
            )
            .unwrap();
        delete(&store, s.id).unwrap();
        assert!(
            store
                .require::<Host>(host_id)
                .unwrap()
                .data
                .startup_snippet_id
                .is_none()
        );
        assert!(list(&store, Some(vault)).unwrap().is_empty());
    }
}
