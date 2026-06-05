//! Streamable HTTP MCP transport (optional `http` feature).

use crate::{
    ClapMcpConfig, ClapMcpError, ClapMcpSchemaMetadata, ClapMcpServeOptions, InProcessToolHandler,
    server::{self, ClapMcpServer, build_clap_mcp_server},
};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use std::{net::SocketAddr, path::PathBuf, sync::Arc};

fn allowed_hosts_for_listen(addr: SocketAddr) -> Vec<String> {
    let mut hosts = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ];
    let port = addr.port();
    hosts.push(format!("127.0.0.1:{port}"));
    hosts.push(format!("localhost:{port}"));
    hosts.push(format!("[::1]:{port}"));
    if addr.ip().is_unspecified() {
        hosts.push(format!("0.0.0.0:{port}"));
    } else {
        hosts.push(format!("{}:{port}", addr.ip()));
    }
    hosts
}

async fn build_server_from_schema_json(
    schema_json: String,
    executable_path: Option<PathBuf>,
    config: &ClapMcpConfig,
    in_process_handler: Option<InProcessToolHandler>,
    serve_options: &ClapMcpServeOptions,
    metadata: &ClapMcpSchemaMetadata,
) -> Result<ClapMcpServer, ClapMcpError> {
    let schema: crate::ClapSchema = serde_json::from_str(&schema_json)?;
    let tools = crate::tools_from_schema_with_metadata(&schema, config, metadata);
    let root_name = schema.root.name.clone();
    build_clap_mcp_server(
        schema_json,
        tools,
        executable_path,
        in_process_handler,
        root_name,
        config,
        serve_options,
        metadata,
    )
}

/// Starts an MCP server over Streamable HTTP at `listen`, exposing `clap://schema`.
pub(crate) async fn serve_schema_json_over_http(
    listen: SocketAddr,
    schema_json: String,
    executable_path: Option<PathBuf>,
    config: ClapMcpConfig,
    in_process_handler: Option<InProcessToolHandler>,
    mut serve_options: ClapMcpServeOptions,
    metadata: &ClapMcpSchemaMetadata,
) -> Result<(), ClapMcpError> {
    let server = build_server_from_schema_json(
        schema_json,
        executable_path,
        &config,
        in_process_handler,
        &serve_options,
        metadata,
    )
    .await?;

    server::spawn_log_forwarder(&server, serve_options.log_rx.take());

    let server = Arc::new(server);
    let http_config =
        StreamableHttpServerConfig::default().with_allowed_hosts(allowed_hosts_for_listen(listen));

    let service = StreamableHttpService::new(
        {
            let server = server.clone();
            move || Ok((*server).clone())
        },
        Arc::new(LocalSessionManager::default()),
        http_config,
    );

    let router = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind(listen).await?;
    axum::serve(listener, router).await?;
    Ok(())
}
