//! Snippets and packages for the mobile UI: thin DTO layer over
//! [`termoso_client::snippets`] (the store logic shared with the desktop) plus
//! running a snippet into live [`SshSession`]s. Variable expansion and line
//! normalisation happen here in Rust; Kotlin only collects the values.

use std::collections::HashMap;
use std::sync::Arc;

use termoso_client::snippets::{self as core, PackageNode, SnippetCard, SnippetForm};
use termoso_core::model::Snippet;
use termoso_core::store::Store;
use uuid::Uuid;

use crate::dto::{millis, parse_id, parse_ids, parse_opt_id};
use crate::error::{MobileError, Result};
use crate::session::SshSession;

/// A stored snippet, ready to display.
#[derive(Debug, Clone, uniffi::Record)]
pub struct SnippetItem {
    pub id: String,
    pub vault_id: String,
    pub label: String,
    pub script: String,
    pub package_id: Option<String>,
    pub close_after_run: bool,
    /// `{{name}}` placeholders in first-appearance order.
    pub variables: Vec<String>,
    /// Hosts the snippet is meant to run on (desktop "targets").
    pub target_host_ids: Vec<String>,
    pub updated_at: i64,
}

impl From<SnippetCard> for SnippetItem {
    fn from(c: SnippetCard) -> Self {
        Self {
            id: c.id.to_string(),
            vault_id: c.vault_id.to_string(),
            label: c.label,
            script: c.script,
            package_id: c.package_id.map(|p| p.to_string()),
            close_after_run: c.close_after_run,
            variables: c.variables,
            target_host_ids: c.target_host_ids.iter().map(ToString::to_string).collect(),
            updated_at: millis(c.updated_at),
        }
    }
}

/// Editor payload; `id == None` creates.
#[derive(Debug, Clone, uniffi::Record)]
pub struct SnippetDraft {
    pub id: Option<String>,
    pub vault_id: String,
    pub label: String,
    pub script: String,
    pub package_id: Option<String>,
    pub close_after_run: bool,
    pub target_host_ids: Vec<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct SnippetPackageItem {
    pub id: String,
    pub vault_id: String,
    pub label: String,
    pub parent_id: Option<String>,
    pub snippet_count: u32,
}

impl From<PackageNode> for SnippetPackageItem {
    fn from(p: PackageNode) -> Self {
        Self {
            id: p.id.to_string(),
            vault_id: p.vault_id.to_string(),
            label: p.label,
            parent_id: p.parent_id.map(|p| p.to_string()),
            snippet_count: p.snippet_count as u32,
        }
    }
}

/// What a run did.
#[derive(Debug, Clone, uniffi::Record)]
pub struct SnippetRun {
    /// Sessions the script was typed into.
    pub session_ids: Vec<String>,
    /// The snippet asks for its terminals to be closed once the command ran
    /// (never set for a paste).
    pub close_after_run: bool,
}

pub(crate) fn list(store: &Store, vault_id: &Option<String>) -> Result<Vec<SnippetItem>> {
    Ok(core::list(store, parse_opt_id(vault_id)?)?
        .into_iter()
        .map(Into::into)
        .collect())
}

pub(crate) fn get(store: &Store, id: Uuid) -> Result<SnippetItem> {
    core::list(store, None)?
        .into_iter()
        .find(|s| s.id == id)
        .map(Into::into)
        .ok_or_else(|| MobileError::not_found(format!("snippet {id}")))
}

pub(crate) fn save(store: &Store, draft: &SnippetDraft) -> Result<SnippetItem> {
    let form = SnippetForm {
        id: parse_opt_id(&draft.id)?,
        vault_id: parse_id(&draft.vault_id)?,
        label: draft.label.clone(),
        script: draft.script.clone(),
        package_id: parse_opt_id(&draft.package_id)?,
        close_after_run: draft.close_after_run,
        sort_order: 0,
    };
    let saved = core::save(store, &form)?;
    let targets = parse_ids(&draft.target_host_ids)?;
    Ok(core::set_targets(store, saved.id, &targets)?.into())
}

/// Copy a snippet within its vault; the copy keeps package and targets.
pub(crate) fn duplicate(store: &Store, id: Uuid) -> Result<SnippetItem> {
    let src = store.require::<Snippet>(id)?;
    let targets = core::list(store, Some(src.vault_id))?
        .into_iter()
        .find(|s| s.id == id)
        .map(|s| s.target_host_ids)
        .unwrap_or_default();
    let saved = core::save(
        store,
        &SnippetForm {
            id: None,
            vault_id: src.vault_id,
            label: format!("{} copy", src.data.label),
            script: src.data.script.clone(),
            package_id: src.data.package_id,
            close_after_run: src.data.close_after_run,
            sort_order: src.data.sort_order,
        },
    )?;
    Ok(core::set_targets(store, saved.id, &targets)?.into())
}

pub(crate) fn delete(store: &Store, id: Uuid) -> Result<()> {
    Ok(core::delete(store, id)?)
}

pub(crate) fn packages(
    store: &Store,
    vault_id: &Option<String>,
) -> Result<Vec<SnippetPackageItem>> {
    Ok(core::packages(store, parse_opt_id(vault_id)?)?
        .into_iter()
        .map(Into::into)
        .collect())
}

pub(crate) fn save_package(
    store: &Store,
    vault_id: &str,
    id: &Option<String>,
    label: &str,
    parent_id: &Option<String>,
) -> Result<SnippetPackageItem> {
    Ok(core::save_package(
        store,
        parse_id(vault_id)?,
        parse_opt_id(id)?,
        label,
        parse_opt_id(parent_id)?,
    )?
    .into())
}

pub(crate) fn delete_package(store: &Store, id: Uuid) -> Result<()> {
    Ok(core::delete_package(store, id)?)
}

pub(crate) fn copy_to_vault(
    store: &Store,
    id: Uuid,
    vault_id: Uuid,
    mv: bool,
) -> Result<SnippetItem> {
    Ok(core::copy_to_vault(store, id, vault_id, mv)?.into())
}

pub(crate) fn copy_package_to_vault(
    store: &Store,
    id: Uuid,
    vault_id: Uuid,
    mv: bool,
) -> Result<SnippetPackageItem> {
    Ok(core::copy_package_to_vault(store, id, vault_id, mv)?.into())
}

/// Expand and normalise the script exactly as [`run`] would type it.
pub(crate) fn prepare(
    store: &Store,
    id: Uuid,
    vars: &HashMap<String, String>,
    paste: bool,
) -> Result<(String, bool)> {
    let snippet = store.require::<Snippet>(id)?;
    let expanded = core::expand(&snippet.data.script, vars)?;
    let text = if paste {
        core::script_to_paste(&expanded)
    } else {
        core::script_to_send(&expanded)
    };
    Ok((text, snippet.data.close_after_run && !paste))
}

/// Type the expanded script into every session. `paste` leaves it on the
/// command line without the trailing newline so the user can edit first.
pub(crate) fn run(
    store: &Store,
    id: Uuid,
    sessions: &[Arc<SshSession>],
    vars: &HashMap<String, String>,
    paste: bool,
) -> Result<SnippetRun> {
    if sessions.is_empty() {
        return Err(MobileError::invalid("pick at least one session"));
    }
    let (text, close_after_run) = prepare(store, id, vars, paste)?;
    let mut done = Vec::with_capacity(sessions.len());
    for s in sessions {
        s.write(text.as_bytes().to_vec());
        done.push(s.id());
    }
    Ok(SnippetRun {
        session_ids: done,
        close_after_run,
    })
}

/// Startup script of a host, ready to type once its shell is up.
pub(crate) fn startup_script(store: &Store, snippet_id: Option<Uuid>) -> Option<String> {
    let id = snippet_id?;
    let s = store.require::<Snippet>(id).ok()?;
    Some(core::script_to_send(&s.data.script))
}

#[cfg(test)]
mod tests {
    use super::*;
    use termoso_core::model::Host;
    use termoso_crypto::keys::SymmetricKey;

    fn store() -> Store {
        Store::open_in_memory(SymmetricKey::generate()).expect("store")
    }

    #[test]
    fn save_duplicate_and_targets_round_trip() {
        let store = store();
        let vault = store.local_vault().unwrap().id;
        let host = store
            .insert(
                vault,
                &Host {
                    label: "db".into(),
                    address: "db".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        let pkg = save_package(&store, &vault.to_string(), &None, "Ops", &None).unwrap();
        let saved = save(
            &store,
            &SnippetDraft {
                id: None,
                vault_id: vault.to_string(),
                label: "Disk".into(),
                script: "df -h {{ path }}\r\n".into(),
                package_id: Some(pkg.id.clone()),
                close_after_run: true,
                target_host_ids: vec![host.to_string(), host.to_string()],
            },
        )
        .unwrap();
        assert_eq!(saved.variables, vec!["path"]);
        assert_eq!(saved.target_host_ids, vec![host.to_string()]);
        assert_eq!(saved.package_id, Some(pkg.id.clone()));
        assert_eq!(
            packages(&store, &Some(vault.to_string())).unwrap()[0].snippet_count,
            1
        );

        let copy = duplicate(&store, parse_id(&saved.id).unwrap()).unwrap();
        assert_eq!(copy.label, "Disk copy");
        assert_eq!(copy.target_host_ids, saved.target_host_ids);
        assert_eq!(copy.package_id, saved.package_id);
        assert_eq!(list(&store, &Some(vault.to_string())).unwrap().len(), 2);

        let got = get(&store, parse_id(&saved.id).unwrap()).unwrap();
        assert_eq!(got.script, "df -h {{ path }}\r\n");

        let mut vars = HashMap::new();
        assert!(prepare(&store, parse_id(&saved.id).unwrap(), &vars, false).is_err());
        vars.insert("path".into(), "/var".into());
        let (text, close) = prepare(&store, parse_id(&saved.id).unwrap(), &vars, false).unwrap();
        assert_eq!(text, "df -h /var\n");
        assert!(close);
        let (text, close) = prepare(&store, parse_id(&saved.id).unwrap(), &vars, true).unwrap();
        assert_eq!(text, "df -h /var");
        assert!(!close);

        assert_eq!(
            startup_script(&store, Some(parse_id(&saved.id).unwrap())).as_deref(),
            Some("df -h {{ path }}\n")
        );
        assert!(startup_script(&store, None).is_none());

        delete_package(&store, parse_id(&pkg.id).unwrap()).unwrap();
        assert!(
            get(&store, parse_id(&saved.id).unwrap())
                .unwrap()
                .package_id
                .is_none()
        );
        delete(&store, parse_id(&saved.id).unwrap()).unwrap();
        assert!(get(&store, parse_id(&saved.id).unwrap()).is_err());
        assert!(run(&store, parse_id(&copy.id).unwrap(), &[], &vars, false).is_err());
    }
}
