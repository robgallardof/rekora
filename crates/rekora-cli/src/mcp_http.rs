//! Authenticated Streamable HTTP adapter for one project per server.

use std::{net::SocketAddr, path::PathBuf};

use anyhow::{Context, Result, bail};
use axum::{
    Router,
    extract::{Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::Response,
    routing::get,
};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};

use crate::mcp::RekoraServer;

async fn authenticate(
    State(expected): State<String>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Credentials belong in a header, never URLs, command arguments or logs.
    if request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        != Some(expected.as_str())
    {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(next.run(request).await)
}

/// Serves authenticated MCP at `/mcp` with an unauthenticated liveness endpoint.
pub fn serve(start: PathBuf, listen: SocketAddr, hosts: Vec<String>) -> Result<()> {
    let token = std::env::var("REKORA_MCP_TOKEN").context("HTTP requires REKORA_MCP_TOKEN")?;
    if token.trim().len() < 32 || !token.bytes().all(|b| b.is_ascii_graphic()) {
        bail!(
            "REKORA_MCP_TOKEN must contain at least 32 printable ASCII characters without spaces"
        );
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let mut config = StreamableHttpServerConfig::default()
            .with_json_response(true)
            .with_max_request_body_bytes(1024 * 1024)
            .enforce_origin_validation();
        config.allowed_hosts.extend(hosts);
        let cancellation = config.cancellation_token.clone();
        let service: StreamableHttpService<RekoraServer, LocalSessionManager> =
            StreamableHttpService::new(
                move || Ok(RekoraServer::new(start.clone())),
                Default::default(),
                config,
            );
        let protected =
            Router::new()
                .nest_service("/mcp", service)
                .layer(middleware::from_fn_with_state(
                    format!("Bearer {token}"),
                    authenticate,
                ));
        let router = Router::new()
            .route("/healthz", get(|| async { "ok" }))
            .merge(protected);
        let listener = tokio::net::TcpListener::bind(listen)
            .await
            .context("binding MCP HTTP listener")?;
        eprintln!(
            "MCP HTTP listening on http://{}/mcp",
            listener.local_addr()?
        );
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                shutdown().await;
                cancellation.cancel();
            })
            .await
            .context("MCP HTTP server failed")
    })
}

async fn shutdown() {
    #[cfg(unix)]
    {
        if let Ok(mut terminate) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}
