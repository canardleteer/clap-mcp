//! Custom MCP resources, prompts, and agent skills export.
//!
//! This module provides types to declare custom resources and prompts (static or
//! async dynamic), and a function to export Cursor Agent Skills (SKILL.md) from
//! the exposed tools, resources, and prompts.

use async_trait::async_trait;
use rust_mcp_sdk::schema::{Prompt, PromptMessage, Resource};
use std::sync::Arc;

/// Content of a custom MCP resource: either static text or provided by an async callback.
#[derive(Clone)]
pub enum ResourceContent {
    /// Fixed content known at serve start.
    Static(String),
    /// Content provided asynchronously when the resource is read.
    Dynamic(Arc<dyn ResourceContentProvider>),
}

impl std::fmt::Debug for ResourceContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceContent::Static(s) => f.debug_tuple("Static").field(&s.len()).finish(),
            ResourceContent::Dynamic(_) => f.write_str("Dynamic(_)"),
        }
    }
}

/// Async provider for custom resource content.
/// Implement this trait (or use a closure adapter) for dynamic resources.
#[async_trait]
pub trait ResourceContentProvider: Send + Sync {
    /// Return the resource content for the given URI.
    async fn read(
        &self,
        uri: &str,
    ) -> std::result::Result<String, Box<dyn std::error::Error + Send + Sync>>;
}

/// Descriptor for a custom MCP resource.
/// Add to [`crate::ClapMcpServeOptions::custom_resources`] to expose it when serving.
#[derive(Clone, Debug)]
pub struct CustomResource {
    /// MCP resource URI (e.g. `myapp://config`). Must be unique.
    pub uri: String,
    /// Short name for listing.
    pub name: String,
    /// Optional human-readable title.
    pub title: Option<String>,
    /// Optional description.
    pub description: Option<String>,
    /// Optional MIME type (e.g. `text/plain`, `application/json`).
    pub mime_type: Option<String>,
    /// Content: static string or async provider.
    pub content: ResourceContent,
}

impl CustomResource {
    /// Build an MCP `Resource` for list_resources.
    pub fn to_list_resource(&self) -> Resource {
        Resource {
            name: self.name.clone(),
            uri: self.uri.clone(),
            title: self.title.clone(),
            description: self.description.clone(),
            mime_type: self.mime_type.clone(),
            annotations: None,
            icons: vec![],
            meta: None,
            size: None,
        }
    }
}

/// Content of a custom MCP prompt: either static messages or provided by an async callback.
#[derive(Clone)]
pub enum PromptContent {
    /// Fixed messages known at serve start.
    Static(Vec<PromptMessage>),
    /// Messages provided asynchronously when the prompt is requested.
    Dynamic(Arc<dyn PromptContentProvider>),
}

impl std::fmt::Debug for PromptContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PromptContent::Static(msgs) => f.debug_tuple("Static").field(&msgs.len()).finish(),
            PromptContent::Dynamic(_) => f.write_str("Dynamic(_)"),
        }
    }
}

/// Async provider for custom prompt content.
/// Implement this trait (or use a closure adapter) for dynamic prompts.
#[async_trait]
pub trait PromptContentProvider: Send + Sync {
    /// Return the prompt messages for the given name and optional arguments.
    async fn get(
        &self,
        name: &str,
        arguments: &serde_json::Map<String, serde_json::Value>,
    ) -> std::result::Result<Vec<PromptMessage>, Box<dyn std::error::Error + Send + Sync>>;
}

/// Descriptor for a custom MCP prompt.
/// Add to [`crate::ClapMcpServeOptions::custom_prompts`] to expose it when serving.
#[derive(Clone, Debug)]
pub struct CustomPrompt {
    /// MCP prompt name. Must be unique (built-in uses `clap-mcp-logging-guide`).
    pub name: String,
    /// Optional human-readable title.
    pub title: Option<String>,
    /// Optional description.
    pub description: Option<String>,
    /// Optional prompt arguments (MCP list declares these; get can receive values).
    pub arguments: Vec<rust_mcp_sdk::schema::PromptArgument>,
    /// Content: static messages or async provider.
    pub content: PromptContent,
}

impl CustomPrompt {
    /// Build an MCP `Prompt` for list_prompts.
    pub fn to_list_prompt(&self) -> Prompt {
        Prompt {
            name: self.name.clone(),
            description: self.description.clone(),
            arguments: self.arguments.clone(),
            icons: vec![],
            meta: None,
            title: self.title.clone(),
        }
    }
}

/// Resolve custom resource content (static or await dynamic).
pub async fn resolve_resource_content(
    r: &CustomResource,
    uri: &str,
) -> std::result::Result<String, rust_mcp_sdk::schema::RpcError> {
    match &r.content {
        ResourceContent::Static(s) => Ok(s.clone()),
        ResourceContent::Dynamic(provider) => provider.read(uri).await.map_err(|e| {
            rust_mcp_sdk::schema::RpcError::internal_error().with_message(e.to_string())
        }),
    }
}

/// Resolve custom prompt content (static or await dynamic).
pub async fn resolve_prompt_content(
    p: &CustomPrompt,
    name: &str,
    arguments: &serde_json::Map<String, serde_json::Value>,
) -> std::result::Result<Vec<PromptMessage>, rust_mcp_sdk::schema::RpcError> {
    match &p.content {
        PromptContent::Static(msgs) => Ok(msgs.clone()),
        PromptContent::Dynamic(provider) => provider.get(name, arguments).await.map_err(|e| {
            rust_mcp_sdk::schema::RpcError::internal_error().with_message(e.to_string())
        }),
    }
}

/// Export Cursor Agent Skills (SKILL.md) from the given schema, tools, custom resources, and prompts.
/// Writes into `output_dir` (e.g. `.agent/skills/<app_name>`).
///
/// Each tool gets a skill; resources and prompts can be grouped or one per skill.
/// See the plan for exact SKILL.md format (frontmatter name, description; body with instructions).
pub fn export_skills(
    _schema: &crate::ClapSchema,
    _metadata: &crate::ClapMcpSchemaMetadata,
    tools: &[Tool],
    custom_resources: &[CustomResource],
    custom_prompts: &[CustomPrompt],
    output_dir: &std::path::Path,
    app_name: &str,
) -> std::result::Result<(), crate::ClapMcpError> {
    use std::io::Write;
    std::fs::create_dir_all(output_dir)?;
    let skills_dir = output_dir.join(sanitize_skill_name(app_name));
    std::fs::create_dir_all(&skills_dir)?;

    if tools.len() == 1 {
        let tool = &tools[0];
        let name = sanitize_skill_name(&tool.name);
        let description = tool
            .description
            .as_deref()
            .unwrap_or("MCP tool from clap-mcp")
            .to_string();
        let body = format!(
            r#"# {}

This skill describes the MCP tool `{}`. Use when the user wants to invoke this CLI capability via MCP.

## Tool: `{}`

{}
"#,
            tool.name, tool.name, tool.name, description
        );
        let content = format!(
            r#"---
name: {}
description: {}
---

{}"#,
            name,
            description.replace('\n', " "),
            body
        );
        std::fs::File::create(skills_dir.join("SKILL.md"))?.write_all(content.as_bytes())?;
    } else {
        for tool in tools {
            let name = sanitize_skill_name(&tool.name);
            let description = tool
                .description
                .as_deref()
                .unwrap_or("MCP tool from clap-mcp")
                .to_string();
            let body = format!(
                r#"# {}

This skill describes the MCP tool `{}`. Use when the user wants to invoke this CLI capability via MCP.

## Tool: `{}`

{}
"#,
                tool.name, tool.name, tool.name, description
            );
            let content = format!(
                r#"---
name: {}
description: {}
---

{}"#,
                name,
                description.replace('\n', " "),
                body
            );
            let tool_dir = skills_dir.join(&name);
            std::fs::create_dir_all(&tool_dir)?;
            std::fs::File::create(tool_dir.join("SKILL.md"))?.write_all(content.as_bytes())?;
        }
    }

    if !custom_resources.is_empty() || !custom_prompts.is_empty() {
        let mut sections = Vec::new();
        for r in custom_resources {
            sections.push(format!(
                "- **{}** (`{}`): {}",
                r.name,
                r.uri,
                r.description.as_deref().unwrap_or("Custom resource")
            ));
        }
        for p in custom_prompts {
            sections.push(format!(
                "- **{}**: {}",
                p.name,
                p.description.as_deref().unwrap_or("Custom prompt")
            ));
        }
        let name = "resources-and-prompts";
        let description = "Custom MCP resources and prompts exposed by this server.";
        let body = format!(
            r#"# Resources and prompts

{}
"#,
            sections.join("\n")
        );
        let content = format!(
            r#"---
name: {}
description: {}
---

{}"#,
            name, description, body
        );
        let res_dir = skills_dir.join(name);
        std::fs::create_dir_all(&res_dir)?;
        std::fs::File::create(res_dir.join("SKILL.md"))?.write_all(content.as_bytes())?;
    }

    Ok(())
}

fn sanitize_skill_name(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

// Tool type for export_skills signature
use rust_mcp_sdk::schema::Tool;
