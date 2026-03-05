use anyhow::Result;
use std::path::Path;

use crate::mcp::router::ServerMode;
use crate::mcp::workspace::WorkspaceConfig;
use crate::mcp::HlvMcpServer;

/// Wait for SIGINT (Ctrl+C) or SIGTERM.
#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{signal, SignalKind};
    let mut sigterm = signal(SignalKind::terminate()).expect("failed to listen for SIGTERM");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = sigterm.recv() => {}
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to listen for ctrl+c");
}

/// Run the MCP server over the specified transport.
///
/// - `project_root`: if Some, single-project mode (auto-detected or --root)
/// - `workspace_path`: if Some, workspace (multi-project) mode
pub fn run(
    project_root: Option<&Path>,
    workspace_path: Option<&str>,
    transport: &str,
    port: u16,
) -> Result<()> {
    let mode = build_mode(project_root, workspace_path)?;

    match transport {
        "stdio" => run_stdio(mode),
        "sse" => run_sse(mode, port),
        other => anyhow::bail!("Unknown transport: {other}. Supported: stdio, sse"),
    }
}

/// Determine server mode from CLI arguments.
fn build_mode(project_root: Option<&Path>, workspace_path: Option<&str>) -> Result<ServerMode> {
    if let Some(ws_path) = workspace_path {
        let config = WorkspaceConfig::load(Path::new(ws_path))?;
        eprintln!("MCP workspace mode: {} projects", config.projects.len());
        Ok(ServerMode::Workspace(config))
    } else {
        let root = project_root
            .ok_or_else(|| anyhow::anyhow!("No project root (use --root or --workspace)"))?;
        Ok(ServerMode::Single(root.to_path_buf()))
    }
}

fn run_stdio(mode: ServerMode) -> Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async {
            // Logging must go to stderr (stdout is the MCP JSON-RPC channel)
            tracing_subscriber::fmt()
                .with_writer(std::io::stderr)
                .with_ansi(false)
                .init();

            let server = HlvMcpServer::new(mode);

            let service = rmcp::ServiceExt::serve(server, rmcp::transport::stdio())
                .await
                .map_err(|e| anyhow::anyhow!("MCP server error: {e}"))?;

            service
                .waiting()
                .await
                .map_err(|e| anyhow::anyhow!("MCP server error: {e}"))?;

            Ok(())
        })
}

fn run_sse(mode: ServerMode, port: u16) -> Result<()> {
    use rmcp::transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    };
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;
    use tower_http::cors::{Any, CorsLayer};

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async {
            tracing_subscriber::fmt()
                .with_writer(std::io::stderr)
                .with_ansi(false)
                .init();

            let ct = CancellationToken::new();

            // Shared subscriptions for file-change notifications
            let subs = crate::mcp::watcher::new_subscriptions();

            // Start file watcher(s) — one per project root
            let _watchers = start_watchers(&mode, &subs);

            let mcp_service = StreamableHttpService::new(
                {
                    let mode = mode.clone();
                    let subs = subs.clone();
                    move || Ok(HlvMcpServer::with_subscriptions(mode.clone(), subs.clone()))
                },
                Arc::new(LocalSessionManager::default()),
                StreamableHttpServerConfig {
                    stateful_mode: true,
                    cancellation_token: ct.child_token(),
                    ..Default::default()
                },
            );

            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
                .expose_headers(Any);

            let app = axum::Router::new()
                .nest_service("/mcp", mcp_service)
                .layer(cors);

            let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
            let listener = tokio::net::TcpListener::bind(addr).await?;
            let mode_label = if mode.is_workspace() {
                "workspace"
            } else {
                "single-project"
            };
            eprintln!("HLV MCP server ({mode_label}, SSE) listening on http://0.0.0.0:{port}/mcp");

            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    shutdown_signal().await;
                    eprintln!("Shutting down MCP server...");
                    ct.cancel();
                })
                .await
                .map_err(|e| anyhow::anyhow!("Server error: {e}"))?;

            Ok(())
        })
}

/// Start file watchers for all project roots.
/// In single mode, watches one project (no project_id prefix).
/// In workspace mode, watches all projects with scoped URIs.
fn start_watchers(
    mode: &ServerMode,
    subs: &crate::mcp::watcher::Subscriptions,
) -> Vec<Option<notify::RecommendedWatcher>> {
    let rt = tokio::runtime::Handle::current();
    match mode {
        ServerMode::Single(root) => {
            vec![crate::mcp::watcher::start_watcher(
                root.clone(),
                None,
                subs.clone(),
                rt,
            )]
        }
        ServerMode::Workspace(config) => config
            .projects
            .iter()
            .map(|p| {
                crate::mcp::watcher::start_watcher(
                    p.root.clone(),
                    Some(p.id.clone()),
                    subs.clone(),
                    rt.clone(),
                )
            })
            .collect(),
    }
}
