//! File watcher for MCP notifications.
//!
//! Watches project YAML files and sends `resources/updated` notifications
//! to subscribed MCP clients when files change.
//!
//! In workspace mode, URIs are scoped to project IDs:
//! `hlv://projects/{id}/milestones` instead of `hlv://milestones`.

use notify::{RecursiveMode, Watcher};
use rmcp::{model::ResourceUpdatedNotificationParam, service::Peer, RoleServer};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Maps watched file names to base HLV resource URI suffixes.
/// Returns `Vec<&str>` of URI suffixes (without `hlv://` prefix).
fn file_to_uri_suffixes(file_name: &str) -> Vec<&'static str> {
    match file_name {
        "milestones.yaml" => vec!["milestones", "tasks", "workflow"],
        "project.yaml" => vec!["project"],
        "gates-policy.yaml" => vec!["gates"],
        _ => vec![],
    }
}

/// Build full URIs from file name and optional project_id.
///
/// - Single mode (`project_id = None`): `hlv://milestones`
/// - Workspace mode (`project_id = Some("backend")`): `hlv://projects/backend/milestones`
fn file_to_uris(file_name: &str, project_id: Option<&str>) -> Vec<String> {
    file_to_uri_suffixes(file_name)
        .into_iter()
        .map(|suffix| match project_id {
            Some(id) => format!("hlv://projects/{id}/{suffix}"),
            None => format!("hlv://{suffix}"),
        })
        .collect()
}

/// Resolve the list of files to watch (absolute paths) from the project root.
/// `milestones.yaml` and `project.yaml` are always at root; gates-policy path
/// is read from `project.yaml` to respect the configured location.
fn watched_files(project_root: &std::path::Path) -> Vec<PathBuf> {
    let mut files = vec![
        project_root.join("milestones.yaml"),
        project_root.join("project.yaml"),
    ];

    // Try to read gates-policy path from project.yaml
    let gates_path = crate::model::project::ProjectMap::load(&project_root.join("project.yaml"))
        .ok()
        .map(|pm| project_root.join(&pm.paths.validation.gates_policy))
        .unwrap_or_else(|| project_root.join("gates-policy.yaml"));
    files.push(gates_path);

    files
}

/// A subscription entry: peer + its unique ID for equality comparison.
#[derive(Debug, Clone)]
pub struct SubEntry {
    pub peer: Peer<RoleServer>,
    pub peer_id: u64,
}

/// Subscription store: URI → list of subscribed peers.
pub type Subscriptions = Arc<Mutex<HashMap<String, Vec<SubEntry>>>>;

/// Creates a new empty subscription store.
pub fn new_subscriptions() -> Subscriptions {
    Arc::new(Mutex::new(HashMap::new()))
}

/// Atomic counter for assigning unique peer IDs.
static NEXT_PEER_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Allocate a unique peer ID (used by the server to track which peer is calling).
pub fn next_peer_id() -> u64 {
    NEXT_PEER_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// Add a peer subscription for a URI.
pub async fn subscribe(subs: &Subscriptions, uri: String, peer: Peer<RoleServer>, peer_id: u64) {
    let mut map = subs.lock().await;
    let entries = map.entry(uri).or_default();
    // Avoid duplicate subscriptions from the same peer
    if !entries.iter().any(|e| e.peer_id == peer_id) {
        entries.push(SubEntry { peer, peer_id });
    }
}

/// Remove subscription for a specific peer on a URI.
pub async fn unsubscribe(subs: &Subscriptions, uri: &str, peer_id: u64) {
    let mut map = subs.lock().await;
    if let Some(entries) = map.get_mut(uri) {
        entries.retain(|e| e.peer_id != peer_id);
        if entries.is_empty() {
            map.remove(uri);
        }
    }
}

/// Start the file watcher. Returns a handle that stops watching when dropped.
///
/// - `project_id`: `None` for single-project mode, `Some("backend")` for workspace.
///   When set, notifications use scoped URIs like `hlv://projects/backend/milestones`.
pub fn start_watcher(
    project_root: PathBuf,
    project_id: Option<String>,
    subs: Subscriptions,
    rt: tokio::runtime::Handle,
) -> Option<notify::RecommendedWatcher> {
    let (tx, rx) = std::sync::mpsc::channel();

    let mut watcher =
        notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                if event.kind.is_modify() || event.kind.is_create() {
                    for path in &event.paths {
                        let _ = tx.send(path.clone());
                    }
                }
            }
        })
        .ok()?;

    // Watch each known file (resolved from project config)
    for path in watched_files(&project_root) {
        if path.exists() {
            let _ = watcher.watch(&path, RecursiveMode::NonRecursive);
        }
    }

    // Spawn a thread to process file change events and send notifications
    std::thread::spawn(move || {
        process_events(rx, project_id.as_deref(), &subs, &rt);
    });

    Some(watcher)
}

fn process_events(
    rx: std::sync::mpsc::Receiver<PathBuf>,
    project_id: Option<&str>,
    subs: &Subscriptions,
    rt: &tokio::runtime::Handle,
) {
    // Debounce: collapse rapid changes
    while let Ok(path) = rx.recv() {
        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        // Drain any queued events for a short debounce window
        std::thread::sleep(std::time::Duration::from_millis(100));
        while rx.try_recv().is_ok() {}

        let uris = file_to_uris(&file_name, project_id);
        if uris.is_empty() {
            continue;
        }

        let subs = subs.clone();
        rt.spawn(async move {
            let map = subs.lock().await;
            for uri in &uris {
                if let Some(entries) = map.get(uri.as_str()) {
                    for entry in entries {
                        let _ = entry
                            .peer
                            .notify_resource_updated(ResourceUpdatedNotificationParam::new(uri))
                            .await;
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_to_uris_single_mode() {
        let uris = file_to_uris("milestones.yaml", None);
        assert_eq!(
            uris,
            vec!["hlv://milestones", "hlv://tasks", "hlv://workflow"]
        );
    }

    #[test]
    fn file_to_uris_workspace_mode() {
        let uris = file_to_uris("milestones.yaml", Some("backend"));
        assert_eq!(
            uris,
            vec![
                "hlv://projects/backend/milestones",
                "hlv://projects/backend/tasks",
                "hlv://projects/backend/workflow",
            ]
        );
    }

    #[test]
    fn file_to_uris_project_yaml_workspace() {
        let uris = file_to_uris("project.yaml", Some("api"));
        assert_eq!(uris, vec!["hlv://projects/api/project"]);
    }

    #[test]
    fn file_to_uris_unknown_file() {
        assert!(file_to_uris("random.txt", None).is_empty());
        assert!(file_to_uris("random.txt", Some("x")).is_empty());
    }
}
