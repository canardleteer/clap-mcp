//! Downstream catalog acceptance: stderr policy, schema fidelity, per-tool
//! output schemas, and global-arg filtering over fresh stdio client/server pairs.

#![cfg(feature = "output-schema")]
#![allow(deprecated)]

use clap::{Parser, Subcommand};
use clap_mcp::{
    AsStructured, ClapMcp, ClapMcpToolOutput, Implementation, IntoClapMcpResult, McpListen,
    ServeMcpBuilder, SubprocessStderr,
};
use rmcp::model::{
    CallToolRequestParams, ClientCapabilities, Implementation as ClientImplementation,
    LoggingMessageNotificationParam, ProtocolVersion, RequestMetaObject,
};
use rmcp::service::NotificationContext;
use rmcp::{ClientHandler, RoleClient, ServiceExt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const ROUTING_RULE: &str =
    "Prefer project-setup for offline YAML validation before compare or apply.";
const INSTRUCTIONS: &str = "Prefer project-setup for offline YAML validation before compare or apply. \
Use compare for live drift and apply only after approval. Doctor diagnoses setup; version returns build identity.";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct VersionOut {
    version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct DoctorOut {
    status: String,
    checks: Vec<String>,
}

#[derive(Debug, Parser, ClapMcp)]
#[clap_mcp(
    reinvocation_safe,
    skip_root_when_subcommands,
    skip_global = "api_token"
)]
#[clap_mcp_output_from = "run_catalog"]
#[command(name = "project-settings", subcommand_required = true)]
struct CatalogRoot {
    /// Native-CLI only transport credential; omitted from MCP schemas.
    #[arg(long, global = true)]
    api_token: Option<String>,
    /// Global verbosity retained on most tools unless skipped per-variant.
    #[arg(long, global = true, action = clap::ArgAction::SetTrue)]
    verbose: bool,
    #[command(subcommand)]
    command: CatalogCmd,
}

#[derive(Debug, Subcommand, ClapMcp)]
#[clap_mcp(schema_only)]
enum CatalogCmd {
    #[clap_mcp(read_only, idempotent, tool_title = "Validate Project Configuration")]
    #[command(
        name = "project-setup",
        about = "Use this tool when the user wants to validate, lint, or check a project-settings YAML file without contacting the remote service."
    )]
    ProjectSetup {
        #[arg(long, default_value = "cwd")]
        path: String,
        #[arg(long, value_parser = ["strict", "lenient"], default_value = "strict")]
        mode: String,
    },
    #[clap_mcp(
        destructive,
        open_world,
        idempotent,
        tool_title = "Apply Project Settings"
    )]
    #[command(
        name = "apply",
        about = "Use this tool when the user wants to make, update, sync, reconcile, or enforce live project settings declared in YAML."
    )]
    Apply {
        #[arg(long, value_parser = ["plan", "apply"], default_value = "plan")]
        mode: String,
        #[arg(long, action = clap::ArgAction::SetTrue, conflicts_with = "force")]
        dry_run: bool,
        #[arg(long, action = clap::ArgAction::SetTrue)]
        force: bool,
        #[arg(long, num_args = 1..=3, action = clap::ArgAction::Append)]
        tags: Vec<String>,
    },
    #[clap_mcp(
        read_only,
        open_world,
        idempotent,
        tool_title = "Compare Project Settings"
    )]
    #[command(
        name = "compare",
        about = "Use this tool when the user wants to preview, compare, audit, check, or show drift between declared YAML and the live project without changing it."
    )]
    Compare {
        #[arg(long)]
        path: Option<String>,
    },
    #[clap_mcp(
        destructive,
        open_world,
        tool_title = "Initialize Project Configuration"
    )]
    #[command(
        name = "init",
        about = "Use this tool when the user wants to create, generate, bootstrap, or pull a working project-settings file."
    )]
    Init {
        #[arg(long, default_value = "project-settings.yaml")]
        output: String,
    },
    #[clap_mcp(destructive, tool_title = "Export Configuration Reference")]
    #[command(
        name = "export",
        about = "Use this tool when the user wants to view, save, print, or obtain a defaults template, complete configuration reference, or JSON Schema."
    )]
    Export {
        #[arg(long, value_parser = ["defaults", "schema", "reference"], default_value = "defaults")]
        kind: String,
    },
    #[clap_mcp(destructive, open_world, tool_title = "Set Project Avatar")]
    #[command(
        name = "set-avatar",
        about = "Use this tool when the user wants to set, change, replace, upload, or remove a project icon, repository icon, logo, avatar, or repository image."
    )]
    SetAvatar {
        #[arg(long, conflicts_with = "remove")]
        image: Option<String>,
        #[arg(long, action = clap::ArgAction::SetTrue)]
        remove: bool,
    },
    #[clap_mcp(destructive, tool_title = "Migrate Legacy Configuration")]
    #[command(
        name = "migrate",
        about = "Use this tool when the user wants to convert, upgrade, rename, or move legacy project or user configuration to canonical paths and schema."
    )]
    Migrate {
        #[arg(long, action = clap::ArgAction::SetTrue)]
        dry_run: bool,
    },
    #[clap_mcp(
        skip = "verbose",
        output_type = "DoctorOut",
        idempotent,
        open_world,
        tool_title = "Diagnose Project Setup"
    )]
    #[command(
        name = "doctor",
        about = "Use this tool when the user wants to diagnose, troubleshoot, health-check, or explain configuration discovery, defaults, origin, target, credentials, or access."
    )]
    Doctor {
        #[arg(long, action = clap::ArgAction::SetTrue)]
        online: bool,
    },
    #[clap_mcp(
        read_only,
        idempotent,
        output_type = "VersionOut",
        tool_title = "Show Build Identity"
    )]
    #[command(
        name = "version",
        about = "Use this tool when the user asks for the server or CLI version, build, source commit, build time, or clean or dirty tree state."
    )]
    Version,
}

fn run_catalog(root: CatalogRoot) -> Result<ClapMcpToolOutput, String> {
    match root.command {
        CatalogCmd::ProjectSetup { path, mode } => {
            Ok(ClapMcpToolOutput::Text(format!("validated:{path}:{mode}")))
        }
        CatalogCmd::Apply { mode, .. } => Ok(ClapMcpToolOutput::Text(format!("applied:{mode}"))),
        CatalogCmd::Compare { path } => Ok(ClapMcpToolOutput::Text(format!(
            "compared:{}",
            path.unwrap_or_else(|| "default".into())
        ))),
        CatalogCmd::Init { output } => Ok(ClapMcpToolOutput::Text(format!("init:{output}"))),
        CatalogCmd::Export { kind } => Ok(ClapMcpToolOutput::Text(format!("export:{kind}"))),
        CatalogCmd::SetAvatar { image, remove } => Ok(ClapMcpToolOutput::Text(format!(
            "avatar:{}:{}",
            image.unwrap_or_default(),
            remove
        ))),
        CatalogCmd::Migrate { dry_run } => {
            Ok(ClapMcpToolOutput::Text(format!("migrate:{dry_run}")))
        }
        CatalogCmd::Doctor { online } => Ok(ClapMcpToolOutput::Structured(
            serde_json::to_value(DoctorOut {
                status: if online {
                    "online".into()
                } else {
                    "local".into()
                },
                checks: vec!["pass:config".into()],
            })
            .unwrap(),
        )),
        CatalogCmd::Version => AsStructured(VersionOut {
            version: "0.1.0-rc.2".into(),
        })
        .into_tool_result()
        .map_err(|e| e.message),
    }
}

#[derive(Clone, Default)]
struct LoggingProbeHandler {
    messages: Arc<Mutex<Vec<String>>>,
}

impl ClientHandler for LoggingProbeHandler {
    async fn on_logging_message(
        &self,
        params: LoggingMessageNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        self.messages
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(params.data.to_string());
    }
}

const EXPECTED_TOOLS: &[(&str, &str)] = &[
    ("project-setup", "Validate Project Configuration"),
    ("apply", "Apply Project Settings"),
    ("compare", "Compare Project Settings"),
    ("init", "Initialize Project Configuration"),
    ("export", "Export Configuration Reference"),
    ("set-avatar", "Set Project Avatar"),
    ("migrate", "Migrate Legacy Configuration"),
    ("doctor", "Diagnose Project Setup"),
    ("version", "Show Build Identity"),
];

async fn launch_catalog_server(
    stderr_policy: SubprocessStderr,
) -> (
    rmcp::service::RunningService<rmcp::RoleClient, LoggingProbeHandler>,
    tokio::task::JoinHandle<()>,
    Arc<Mutex<Vec<String>>>,
) {
    let (io1, io2) = tokio::io::duplex(16_384);
    let (server_read, server_write) = tokio::io::split(io1);
    let (client_read, client_write) = tokio::io::split(io2);

    let server_info = Implementation::new("project-settings", "9.9.9")
        .with_title("Project Settings")
        .with_description("Downstream catalog fixture");

    let server_task = tokio::spawn(async move {
        ServeMcpBuilder::for_cli::<CatalogRoot>(McpListen::Stdio)
            .stdio_io(server_read, server_write)
            .server_info(server_info)
            .instructions(INSTRUCTIONS)
            .subprocess_stderr(stderr_policy)
            .serve()
            .await
            .expect("catalog server should run");
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let messages = Arc::new(Mutex::new(Vec::new()));
    let handler = LoggingProbeHandler {
        messages: messages.clone(),
    };
    let client = handler
        .serve((client_read, client_write))
        .await
        .expect("catalog client should connect");

    (client, server_task, messages)
}

async fn shutdown_pair(
    client: rmcp::service::RunningService<rmcp::RoleClient, LoggingProbeHandler>,
    server: tokio::task::JoinHandle<()>,
) {
    client.cancel().await.ok();
    server.abort();
    let _ = server.await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn downstream_catalog_initialize_discover_and_tools_list() {
    let schema_meta =
        <CatalogRoot as clap_mcp::ClapMcpSchemaMetadataProvider>::clap_mcp_schema_metadata();
    assert!(
        schema_meta.tool_output_schemas.contains_key("doctor"),
        "doctor output schema missing from metadata; keys={:?}",
        schema_meta.tool_output_schemas.keys().collect::<Vec<_>>()
    );
    assert!(schema_meta.tool_output_schemas.contains_key("version"));
    assert!(
        schema_meta
            .skip_global_args
            .iter()
            .any(|g| g == "api_token")
    );

    let (client, server, messages) = launch_catalog_server(SubprocessStderr::Capture).await;

    let peer_info = client.peer_info().expect("initialize peer info");
    let server_info = peer_info.server_info.as_ref().expect("server_info");
    assert_eq!(server_info.name, "project-settings");
    assert_eq!(server_info.version, "9.9.9");
    let instructions = peer_info.instructions.as_deref().expect("instructions");
    assert!(!instructions.is_empty());
    assert!(instructions[..instructions.len().min(512)].contains(ROUTING_RULE));
    assert!(peer_info.capabilities.logging.is_none());

    let meta = RequestMetaObject::with_client_context(
        ProtocolVersion::LATEST,
        ClientImplementation::new("test-client", "1.0.0"),
        ClientCapabilities::default(),
    );
    let discover = client.discover(meta).await.expect("discover");
    let discover_info = discover.server_info().expect("discover server_info");
    assert_eq!(discover_info.name, "project-settings");
    assert_eq!(
        discover.instructions.as_deref(),
        peer_info.instructions.as_deref()
    );

    let tools = client.list_tools(None).await.expect("tools/list").tools;
    assert_eq!(tools.len(), EXPECTED_TOOLS.len());
    for (name, title) in EXPECTED_TOOLS {
        let tool = tools.iter().find(|t| t.name == *name).unwrap_or_else(|| {
            panic!("missing tool {name}");
        });
        let desc = tool.description.as_deref().unwrap_or("");
        assert!(
            desc.starts_with("Use this tool when"),
            "{name} description should start with routing phrase: {desc}"
        );
        let ann = tool.annotations.as_ref().expect("annotations");
        assert_eq!(
            ann.title.as_deref(),
            Some(*title),
            "{name} annotation title mismatch"
        );
        assert_eq!(
            tool.title.as_deref(),
            Some(*title),
            "{name} tool.title mismatch"
        );
        let props = tool
            .input_schema
            .get("properties")
            .and_then(|v| v.as_object())
            .expect("properties");
        assert_eq!(
            tool.input_schema.get("additionalProperties"),
            Some(&json!(false))
        );
        assert!(
            !props.contains_key("api_token"),
            "{name} must omit global api_token"
        );
    }

    let apply = tools.iter().find(|t| t.name == "apply").unwrap();
    let apply_props = apply.input_schema.get("properties").unwrap();
    assert_eq!(
        apply_props.get("mode").and_then(|m| m.get("enum")),
        Some(&json!(["plan", "apply"]))
    );
    assert_eq!(
        apply_props.get("mode").and_then(|m| m.get("default")),
        Some(&json!("plan"))
    );
    let tags = apply_props.get("tags").unwrap();
    assert_eq!(tags.get("minItems"), Some(&json!(1)));
    assert_eq!(tags.get("maxItems"), Some(&json!(3)));
    assert!(
        apply.input_schema.get("dependentSchemas").is_some(),
        "apply should encode conflicts"
    );

    let doctor = tools.iter().find(|t| t.name == "doctor").unwrap();
    let doctor_props = doctor
        .input_schema
        .get("properties")
        .unwrap()
        .as_object()
        .unwrap();
    assert!(!doctor_props.contains_key("verbose"));
    assert!(doctor.output_schema.is_some());
    let version = tools.iter().find(|t| t.name == "version").unwrap();
    assert!(version.output_schema.is_some());
    let setup = tools.iter().find(|t| t.name == "project-setup").unwrap();
    assert!(setup.output_schema.is_none());

    assert!(
        messages.lock().unwrap().is_empty(),
        "instructions must not arrive as logging notifications"
    );

    shutdown_pair(client, server).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn downstream_catalog_tool_calls_structured_and_text() {
    let (client, server, messages) = launch_catalog_server(SubprocessStderr::Capture).await;

    let ok = client
        .call_tool(CallToolRequestParams::new("project-setup").with_arguments(
            serde_json::Map::from_iter([("path".into(), json!("/tmp/proj"))]),
        ))
        .await
        .expect("project-setup");
    assert_ne!(ok.is_error, Some(true));
    let text = ok
        .content
        .iter()
        .filter_map(|b| match b {
            rmcp::model::ContentBlock::Text(t) => Some(t.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("validated:/tmp/proj"));

    let missing = client
        .call_tool(CallToolRequestParams::new("compare").with_arguments(
            serde_json::Map::from_iter([("unknown_arg".into(), json!("x"))]),
        ))
        .await;
    assert!(missing.is_err(), "unknown argument should fail");

    let recovered = client
        .call_tool(CallToolRequestParams::new("compare"))
        .await
        .expect("compare recovery");
    assert_ne!(recovered.is_error, Some(true));

    let version = client
        .call_tool(CallToolRequestParams::new("version"))
        .await
        .expect("version");
    let structured = version.structured_content.expect("structuredContent");
    assert_eq!(
        structured.get("version").and_then(|v| v.as_str()),
        Some("0.1.0-rc.2")
    );

    let doctor = client
        .call_tool(CallToolRequestParams::new("doctor"))
        .await
        .expect("doctor");
    assert!(doctor.structured_content.is_some());

    assert!(
        messages.lock().unwrap().is_empty(),
        "Capture policy must not emit notifications/message"
    );

    shutdown_pair(client, server).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn downstream_catalog_notify_stderr_advertises_logging() {
    let (client, server, _messages) = launch_catalog_server(SubprocessStderr::Notify).await;
    let peer_info = client.peer_info().expect("initialize");
    assert!(peer_info.capabilities.logging.is_some());
    assert!(
        peer_info
            .instructions
            .as_deref()
            .is_some_and(|s| s.contains("notifications/message"))
    );
    shutdown_pair(client, server).await;
}
