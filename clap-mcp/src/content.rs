//! Custom MCP resources, prompts, and agent skills export.
//!
//! This module provides types to declare custom resources and prompts (static or
//! async dynamic), and a function to export [Agent Skills](https://agentskills.io/specification)
//! (SKILL.md) from the exposed tools, resources, and prompts.

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

/// Export [Agent Skills](https://agentskills.io/specification) (SKILL.md) from the given
/// schema, tools, custom resources, and prompts.
///
/// Writes into `output_dir` (e.g. `.agents/skills/`). Each tool gets its own skill
/// directory with a SKILL.md; resources and prompts are grouped into a single skill.
///
/// Generated files follow the [Agent Skills specification](https://agentskills.io/specification):
/// YAML frontmatter (`name`, `description`, `allowed-tools`) and a markdown body with
/// usage instructions. The `name` field matches the parent directory name as required by
/// the spec. The `allowed-tools` field lists the MCP tool the skill describes; note that
/// this field is still experimental in the spec with no defined syntax convention.
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
    let app_dir = output_dir.join(sanitize_skill_name(app_name));
    std::fs::create_dir_all(&app_dir)?;

    if tools.len() == 1 {
        let tool = &tools[0];
        let dir_name = sanitize_skill_name(app_name);
        let description = build_tool_description(tool);
        let body = build_tool_body(tool);
        let content = format_skill_md(&dir_name, &description, Some(&tool.name), &body);
        std::fs::File::create(app_dir.join("SKILL.md"))?.write_all(content.as_bytes())?;
    } else {
        for tool in tools {
            let dir_name = sanitize_skill_name(&tool.name);
            let description = build_tool_description(tool);
            let body = build_tool_body(tool);
            let content = format_skill_md(&dir_name, &description, Some(&tool.name), &body);
            let tool_dir = app_dir.join(&dir_name);
            std::fs::create_dir_all(&tool_dir)?;
            std::fs::File::create(tool_dir.join("SKILL.md"))?.write_all(content.as_bytes())?;
        }
    }

    if !custom_resources.is_empty() || !custom_prompts.is_empty() {
        let mut sections = Vec::new();
        if !custom_resources.is_empty() {
            sections.push("## Resources\n".to_string());
            for r in custom_resources {
                sections.push(format!(
                    "- **{}** (`{}`): {}",
                    r.name,
                    r.uri,
                    r.description.as_deref().unwrap_or("Custom resource")
                ));
            }
            sections.push(String::new());
        }
        if !custom_prompts.is_empty() {
            sections.push("## Prompts\n".to_string());
            for p in custom_prompts {
                let mut line = format!(
                    "- **{}**: {}",
                    p.name,
                    p.description.as_deref().unwrap_or("Custom prompt")
                );
                if !p.arguments.is_empty() {
                    let arg_names: Vec<_> = p.arguments.iter().map(|a| a.name.as_str()).collect();
                    line.push_str(&format!(" (arguments: {})", arg_names.join(", ")));
                }
                sections.push(line);
            }
            sections.push(String::new());
        }
        let dir_name = "resources-and-prompts";
        let description = format!(
            "Custom MCP resources and prompts exposed by {}. Use when interacting with this server's non-tool capabilities.",
            app_name
        );
        let body = format!(
            "# Resources and Prompts\n\nThis skill describes the custom MCP resources and prompts provided by `{}`.\n\n{}",
            app_name,
            sections.join("\n")
        );
        let content = format_skill_md(dir_name, &description, None, &body);
        let res_dir = app_dir.join(dir_name);
        std::fs::create_dir_all(&res_dir)?;
        std::fs::File::create(res_dir.join("SKILL.md"))?.write_all(content.as_bytes())?;
    }

    Ok(())
}

fn build_tool_description(tool: &Tool) -> String {
    let base = tool
        .description
        .as_deref()
        .unwrap_or("MCP tool from clap-mcp");
    let sanitized = base.replace('\n', " ");
    let desc = format!(
        "{}. Use when invoking the `{}` tool via MCP.",
        sanitized.trim_end_matches('.'),
        tool.name
    );
    truncate_to_char_boundary(&desc, 1024)
}

fn build_tool_body(tool: &Tool) -> String {
    let description = tool
        .description
        .as_deref()
        .unwrap_or("MCP tool from clap-mcp");
    let mut body = format!("# {}\n\n{}\n", tool.name, description);

    if let Some(ref props) = tool.input_schema.properties
        && !props.is_empty()
    {
        body.push_str("\n## Arguments\n\n");
        let required_set: std::collections::HashSet<&str> = tool
            .input_schema
            .required
            .iter()
            .map(|s| s.as_str())
            .collect();
        let mut names: Vec<_> = props.keys().collect();
        names.sort();
        for name in names {
            let prop = &props[name];
            let type_str = prop
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("string");
            let required = if required_set.contains(name.as_str()) {
                "required"
            } else {
                "optional"
            };
            let desc = prop
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if desc.is_empty() {
                body.push_str(&format!("- `{}` ({}, {})\n", name, type_str, required));
            } else {
                body.push_str(&format!(
                    "- `{}` ({}, {}): {}\n",
                    name, type_str, required, desc
                ));
            }
        }
    }

    body
}

fn format_skill_md(
    name: &str,
    description: &str,
    allowed_tools: Option<&str>,
    body: &str,
) -> String {
    let mut frontmatter = format!(
        "---\nname: {}\ndescription: {}",
        name,
        description.replace('\n', " "),
    );
    if let Some(tools) = allowed_tools {
        frontmatter.push_str(&format!("\nallowed-tools: {}", tools));
    }
    frontmatter.push_str("\n---");
    format!("{}\n\n{}\n", frontmatter, body.trim_end())
}

/// Sanitize a string into a valid Agent Skills `name` field.
///
/// Per the [specification](https://agentskills.io/specification): lowercase alphanumeric
/// and hyphens only, no leading/trailing/consecutive hyphens, max 64 characters.
fn sanitize_skill_name(s: &str) -> String {
    let raw: String = s
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let name = raw
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    truncate_to_char_boundary(&name, 64)
}

fn truncate_to_char_boundary(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        s[..s.floor_char_boundary(max)].to_string()
    }
}

use rust_mcp_sdk::schema::Tool;
