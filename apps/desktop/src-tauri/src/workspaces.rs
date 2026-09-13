//! Workspace templates and the last open session, kept in the local store so
//! the UI can offer "Restore previous session" and reopen saved workspaces.
//! Only connection targets and layout are stored — never terminal contents.

use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use crate::error::{DesktopError, Result};
use crate::sessions::OpenTarget;
use crate::state::AppState;

const META_KEY: &str = "desktop.workspaces";
const MAX_TEMPLATES: usize = 200;
const MAX_NAME: usize = 120;

/// Split tree of a saved tab; leaves are connection targets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LayoutTemplate {
    Leaf {
        target: OpenTarget,
    },
    Split {
        /// `row` | `column`.
        direction: String,
        ratio: f32,
        first: Box<LayoutTemplate>,
        second: Box<LayoutTemplate>,
    },
}

impl LayoutTemplate {
    fn leaves(&self) -> usize {
        match self {
            Self::Leaf { .. } => 1,
            Self::Split { first, second, .. } => first.leaves() + second.leaves(),
        }
    }

    fn validate(&self) -> Result<()> {
        if let Self::Split {
            direction,
            ratio,
            first,
            second,
        } = self
        {
            if !matches!(direction.as_str(), "row" | "column") {
                return Err(DesktopError::invalid(
                    "split direction must be row or column",
                ));
            }
            if !(0.0..=1.0).contains(ratio) {
                return Err(DesktopError::invalid("split ratio out of range"));
            }
            first.validate()?;
            second.validate()?;
        }
        Ok(())
    }
}

/// A saved workspace: named layout of connections, reopened from the New Tab page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTemplate {
    pub id: Uuid,
    pub name: String,
    /// `split` | `list`.
    pub view_mode: String,
    pub layout: LayoutTemplate,
    pub created_at: String,
    pub updated_at: String,
}

/// One tab as it was open when the app last saved its state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotTab {
    /// Workspace name; `None` for a plain session tab.
    pub name: Option<String>,
    pub view_mode: String,
    pub template_id: Option<Uuid>,
    pub layout: LayoutTemplate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSnapshot {
    pub saved_at: String,
    pub tabs: Vec<SnapshotTab>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WorkspacesState {
    pub templates: Vec<WorkspaceTemplate>,
    pub last_session: Option<SessionSnapshot>,
}

impl WorkspacesState {
    fn validate(&self) -> Result<()> {
        if self.templates.len() > MAX_TEMPLATES {
            return Err(DesktopError::invalid("too many workspace templates"));
        }
        for t in &self.templates {
            if t.name.trim().is_empty() || t.name.chars().count() > MAX_NAME {
                return Err(DesktopError::invalid(
                    "workspace name must be 1–120 characters",
                ));
            }
            validate_view_mode(&t.view_mode)?;
            t.layout.validate()?;
        }
        if let Some(s) = &self.last_session {
            for t in &s.tabs {
                validate_view_mode(&t.view_mode)?;
                t.layout.validate()?;
                if t.layout.leaves() > 16 {
                    return Err(DesktopError::invalid("too many panes in a saved tab"));
                }
            }
        }
        Ok(())
    }
}

fn validate_view_mode(mode: &str) -> Result<()> {
    if matches!(mode, "split" | "list") {
        Ok(())
    } else {
        Err(DesktopError::invalid("viewMode must be split or list"))
    }
}

pub fn load(state: &AppState) -> Result<WorkspacesState> {
    Ok(match state.store.secret_meta(META_KEY)? {
        Some(json) => serde_json::from_str(&json)?,
        None => WorkspacesState::default(),
    })
}

#[tauri::command]
pub fn workspaces_get(state: State<'_, AppState>) -> Result<WorkspacesState> {
    load(&state)
}

#[tauri::command]
pub fn workspaces_set(
    state: State<'_, AppState>,
    workspaces: WorkspacesState,
) -> Result<WorkspacesState> {
    workspaces.validate()?;
    state
        .store
        .set_secret_meta(META_KEY, &serde_json::to_string(&workspaces)?)?;
    Ok(workspaces)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(target: OpenTarget) -> LayoutTemplate {
        LayoutTemplate::Leaf { target }
    }

    #[test]
    fn roundtrips_through_json_with_camel_case_keys() {
        let st = WorkspacesState {
            templates: vec![WorkspaceTemplate {
                id: Uuid::nil(),
                name: "Prod".into(),
                view_mode: "split".into(),
                layout: LayoutTemplate::Split {
                    direction: "row".into(),
                    ratio: 0.5,
                    first: Box::new(leaf(OpenTarget::Local)),
                    second: Box::new(leaf(OpenTarget::Quick {
                        address: "example.org".into(),
                        username: Some("root".into()),
                        port: None,
                        protocol: None,
                    })),
                },
                created_at: "2026-01-01T00:00:00Z".into(),
                updated_at: "2026-01-01T00:00:00Z".into(),
            }],
            last_session: Some(SessionSnapshot {
                saved_at: "2026-01-01T00:00:00Z".into(),
                tabs: vec![SnapshotTab {
                    name: None,
                    view_mode: "list".into(),
                    template_id: None,
                    layout: leaf(OpenTarget::Host {
                        host_id: Uuid::nil(),
                        protocol: None,
                    }),
                }],
            }),
        };
        st.validate().unwrap();
        let json = serde_json::to_string(&st).unwrap();
        assert!(json.contains("\"viewMode\""));
        assert!(json.contains("\"lastSession\""));
        assert!(json.contains("\"host_id\""));
        let back: WorkspacesState = serde_json::from_str(&json).unwrap();
        assert_eq!(back, st);
    }

    #[test]
    fn rejects_bad_names_and_layouts() {
        let bad_name = WorkspacesState {
            templates: vec![WorkspaceTemplate {
                id: Uuid::nil(),
                name: "   ".into(),
                view_mode: "split".into(),
                layout: leaf(OpenTarget::Local),
                created_at: String::new(),
                updated_at: String::new(),
            }],
            last_session: None,
        };
        assert!(bad_name.validate().is_err());

        let bad_split = WorkspacesState {
            templates: vec![],
            last_session: Some(SessionSnapshot {
                saved_at: String::new(),
                tabs: vec![SnapshotTab {
                    name: Some("x".into()),
                    view_mode: "split".into(),
                    template_id: None,
                    layout: LayoutTemplate::Split {
                        direction: "diagonal".into(),
                        ratio: 0.5,
                        first: Box::new(leaf(OpenTarget::Local)),
                        second: Box::new(leaf(OpenTarget::Local)),
                    },
                }],
            }),
        };
        assert!(bad_split.validate().is_err());
    }

    #[test]
    fn missing_fields_default() {
        let st: WorkspacesState = serde_json::from_str("{}").unwrap();
        assert!(st.templates.is_empty());
        assert!(st.last_session.is_none());
    }
}
