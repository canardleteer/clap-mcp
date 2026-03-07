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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Arg, Command};
    use rust_mcp_sdk::schema::{ContentBlock, PromptArgument, ToolInputSchema};
    use std::error::Error;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Debug)]
    struct TestError(&'static str);

    impl std::fmt::Display for TestError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(self.0)
        }
    }

    impl Error for TestError {}

    struct TestResourceProvider {
        response: Result<String, &'static str>,
        seen_uri: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl ResourceContentProvider for TestResourceProvider {
        async fn read(
            &self,
            uri: &str,
        ) -> std::result::Result<String, Box<dyn Error + Send + Sync>> {
            self.seen_uri.lock().unwrap().push(uri.to_string());
            match &self.response {
                Ok(text) => Ok(text.clone()),
                Err(message) => Err(Box::new(TestError(message))),
            }
        }
    }

    struct TestPromptProvider {
        response: Result<Vec<PromptMessage>, &'static str>,
        seen: Mutex<Vec<(String, serde_json::Map<String, serde_json::Value>)>>,
    }

    #[async_trait]
    impl PromptContentProvider for TestPromptProvider {
        async fn get(
            &self,
            name: &str,
            arguments: &serde_json::Map<String, serde_json::Value>,
        ) -> std::result::Result<Vec<PromptMessage>, Box<dyn Error + Send + Sync>> {
            self.seen
                .lock()
                .unwrap()
                .push((name.to_string(), arguments.clone()));
            match &self.response {
                Ok(messages) => Ok(messages.clone()),
                Err(message) => Err(Box::new(TestError(message))),
            }
        }
    }

    fn temp_output_dir(test_name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be monotonic enough for test tempdirs")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "clap-mcp-content-tests-{test_name}-{}-{nanos}",
            std::process::id()
        ))
    }

    fn sample_messages() -> Vec<PromptMessage> {
        vec![PromptMessage {
            role: rust_mcp_sdk::schema::Role::User,
            content: ContentBlock::text_content("hello from prompt".to_string()),
        }]
    }

    fn sample_tool(name: &str, description: Option<&str>) -> Tool {
        let mut count_property = serde_json::Map::new();
        count_property.insert("type".to_string(), serde_json::json!("integer"));
        count_property.insert(
            "description".to_string(),
            serde_json::json!("How many items to process"),
        );
        let mut properties = std::collections::HashMap::new();
        properties.insert("count".to_string(), count_property);
        Tool {
            name: name.to_string(),
            title: None,
            description: description.map(str::to_string),
            input_schema: ToolInputSchema::new(vec!["count".to_string()], Some(properties), None),
            annotations: None,
            execution: None,
            icons: vec![],
            meta: None,
            output_schema: None,
        }
    }

    fn sample_schema() -> crate::ClapSchema {
        crate::schema_from_command(
            &Command::new("sample-app").arg(
                Arg::new("verbose")
                    .long("verbose")
                    .help("Enable verbose logging"),
            ),
        )
    }

    #[tokio::test]
    async fn resolve_resource_content_handles_static_and_dynamic() {
        let static_resource = CustomResource {
            uri: "test://static".to_string(),
            name: "static".to_string(),
            title: None,
            description: None,
            mime_type: Some("text/plain".to_string()),
            content: ResourceContent::Static("fixed text".to_string()),
        };
        assert_eq!(
            resolve_resource_content(&static_resource, &static_resource.uri)
                .await
                .expect("static content should resolve"),
            "fixed text"
        );

        let provider = Arc::new(TestResourceProvider {
            response: Ok("dynamic text".to_string()),
            seen_uri: Mutex::new(Vec::new()),
        });
        let dynamic_resource = CustomResource {
            uri: "test://dynamic".to_string(),
            name: "dynamic".to_string(),
            title: Some("Dynamic".to_string()),
            description: Some("dynamic provider".to_string()),
            mime_type: None,
            content: ResourceContent::Dynamic(provider.clone()),
        };
        assert_eq!(
            resolve_resource_content(&dynamic_resource, &dynamic_resource.uri)
                .await
                .expect("dynamic content should resolve"),
            "dynamic text"
        );
        assert_eq!(
            provider.seen_uri.lock().unwrap().as_slice(),
            ["test://dynamic"]
        );
    }

    #[tokio::test]
    async fn resolve_resource_content_maps_provider_errors() {
        let provider = Arc::new(TestResourceProvider {
            response: Err("resource boom"),
            seen_uri: Mutex::new(Vec::new()),
        });
        let resource = CustomResource {
            uri: "test://broken".to_string(),
            name: "broken".to_string(),
            title: None,
            description: None,
            mime_type: None,
            content: ResourceContent::Dynamic(provider),
        };

        let error = resolve_resource_content(&resource, &resource.uri)
            .await
            .expect_err("dynamic error should map to rpc error");
        assert_eq!(error.message, "resource boom");
    }

    #[tokio::test]
    async fn resolve_prompt_content_handles_static_and_dynamic() {
        let static_prompt = CustomPrompt {
            name: "static-prompt".to_string(),
            title: None,
            description: Some("static prompt".to_string()),
            arguments: vec![],
            content: PromptContent::Static(sample_messages()),
        };
        assert_eq!(
            resolve_prompt_content(&static_prompt, &static_prompt.name, &serde_json::Map::new())
                .await
                .expect("static prompt should resolve")
                .len(),
            1
        );

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "topic".to_string(),
            serde_json::Value::String("coverage".into()),
        );
        let provider = Arc::new(TestPromptProvider {
            response: Ok(sample_messages()),
            seen: Mutex::new(Vec::new()),
        });
        let dynamic_prompt = CustomPrompt {
            name: "dynamic-prompt".to_string(),
            title: Some("Dynamic Prompt".to_string()),
            description: Some("dynamic prompt".to_string()),
            arguments: vec![PromptArgument {
                name: "topic".to_string(),
                title: None,
                description: Some("Topic to discuss".to_string()),
                required: Some(true),
            }],
            content: PromptContent::Dynamic(provider.clone()),
        };

        let messages = resolve_prompt_content(&dynamic_prompt, &dynamic_prompt.name, &arguments)
            .await
            .expect("dynamic prompt should resolve");
        assert_eq!(messages.len(), 1);

        let seen = provider.seen.lock().unwrap();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0, "dynamic-prompt");
        assert_eq!(
            seen[0].1.get("topic").and_then(|value| value.as_str()),
            Some("coverage")
        );
    }

    #[tokio::test]
    async fn resolve_prompt_content_maps_provider_errors() {
        let provider = Arc::new(TestPromptProvider {
            response: Err("prompt boom"),
            seen: Mutex::new(Vec::new()),
        });
        let prompt = CustomPrompt {
            name: "broken-prompt".to_string(),
            title: None,
            description: None,
            arguments: vec![],
            content: PromptContent::Dynamic(provider),
        };

        let error = resolve_prompt_content(&prompt, &prompt.name, &serde_json::Map::new())
            .await
            .expect_err("prompt provider error should map to rpc error");
        assert_eq!(error.message, "prompt boom");
    }

    #[test]
    fn export_skills_writes_single_tool_skill() {
        let output_dir = temp_output_dir("single");
        let schema = sample_schema();
        let tools = vec![sample_tool(
            "Run-Task",
            Some("Runs the task.\nWith details."),
        )];

        export_skills(
            &schema,
            &crate::ClapMcpSchemaMetadata::default(),
            &tools,
            &[],
            &[],
            &output_dir,
            "My App",
        )
        .expect("single-tool export should succeed");

        let skill_path = output_dir.join("my-app").join("SKILL.md");
        let content = std::fs::read_to_string(&skill_path).expect("skill file should exist");
        assert!(content.contains("name: my-app"));
        assert!(content.contains("allowed-tools: Run-Task"));
        assert!(content.contains(
            "Runs the task. With details. Use when invoking the `Run-Task` tool via MCP."
        ));
        assert!(content.contains("- `count` (integer, required): How many items to process"));

        std::fs::remove_dir_all(output_dir).expect("temp output dir should be removable");
    }

    #[test]
    fn export_skills_writes_multi_tool_and_resources_skill() {
        let output_dir = temp_output_dir("multi");
        let schema = sample_schema();
        let tools = vec![
            sample_tool("First Tool", Some("First tool.")),
            sample_tool("Second/Tool", None),
        ];
        let resources = vec![CustomResource {
            uri: "app://config".to_string(),
            name: "Config".to_string(),
            title: None,
            description: Some("Configuration snapshot".to_string()),
            mime_type: Some("application/json".to_string()),
            content: ResourceContent::Static("{\"ok\":true}".to_string()),
        }];
        let prompts = vec![CustomPrompt {
            name: "guidance".to_string(),
            title: Some("Guidance".to_string()),
            description: Some("Prompt guidance".to_string()),
            arguments: vec![PromptArgument {
                name: "audience".to_string(),
                title: None,
                description: None,
                required: Some(false),
            }],
            content: PromptContent::Static(sample_messages()),
        }];

        export_skills(
            &schema,
            &crate::ClapMcpSchemaMetadata::default(),
            &tools,
            &resources,
            &prompts,
            &output_dir,
            "App With Extras",
        )
        .expect("multi export should succeed");

        let first_tool_path = output_dir
            .join("app-with-extras")
            .join("first-tool")
            .join("SKILL.md");
        let second_tool_path = output_dir
            .join("app-with-extras")
            .join("second-tool")
            .join("SKILL.md");
        let resources_path = output_dir
            .join("app-with-extras")
            .join("resources-and-prompts")
            .join("SKILL.md");

        let first_tool = std::fs::read_to_string(first_tool_path).expect("first tool skill exists");
        let second_tool =
            std::fs::read_to_string(second_tool_path).expect("second tool skill exists");
        let resource_prompt_skill =
            std::fs::read_to_string(resources_path).expect("resource prompt skill exists");

        assert!(first_tool.contains("name: first-tool"));
        assert!(second_tool.contains("name: second-tool"));
        assert!(second_tool.contains("allowed-tools: Second/Tool"));
        assert!(resource_prompt_skill.contains("## Resources"));
        assert!(
            resource_prompt_skill.contains("**Config** (`app://config`): Configuration snapshot")
        );
        assert!(resource_prompt_skill.contains("## Prompts"));
        assert!(
            resource_prompt_skill.contains("**guidance**: Prompt guidance (arguments: audience)")
        );

        std::fs::remove_dir_all(output_dir).expect("temp output dir should be removable");
    }

    #[test]
    fn sanitize_and_truncate_helpers_follow_skill_rules() {
        let long = format!("{}{}", "A".repeat(80), "! invalid suffix");
        let sanitized = sanitize_skill_name(&long);
        assert_eq!(sanitized.len(), 64);
        assert!(
            sanitized
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        );
        assert!(!sanitized.starts_with('-'));
        assert!(!sanitized.ends_with('-'));
        assert_eq!(sanitize_skill_name("My   Fancy__Tool"), "my-fancy-tool");

        let formatted = format_skill_md(
            "tool-name",
            "First line.\nSecond line.",
            Some("tool-name"),
            "# Body\n\nDetails\n",
        );
        assert!(formatted.contains("description: First line. Second line."));
        assert!(formatted.ends_with("# Body\n\nDetails\n"));
    }

    #[test]
    fn custom_resource_and_prompt_list_items_preserve_metadata() {
        let resource = CustomResource {
            uri: "test://resource".to_string(),
            name: "Resource".to_string(),
            title: Some("Resource Title".to_string()),
            description: Some("Helpful resource".to_string()),
            mime_type: Some("text/plain".to_string()),
            content: ResourceContent::Static("resource body".to_string()),
        };
        let prompt = CustomPrompt {
            name: "prompt-name".to_string(),
            title: Some("Prompt Title".to_string()),
            description: Some("Helpful prompt".to_string()),
            arguments: vec![PromptArgument {
                name: "subject".to_string(),
                title: Some("Subject".to_string()),
                description: Some("What to discuss".to_string()),
                required: Some(true),
            }],
            content: PromptContent::Static(sample_messages()),
        };

        let listed_resource = resource.to_list_resource();
        let listed_prompt = prompt.to_list_prompt();
        assert_eq!(listed_resource.uri, "test://resource");
        assert_eq!(listed_resource.mime_type.as_deref(), Some("text/plain"));
        assert_eq!(listed_prompt.name, "prompt-name");
        assert_eq!(listed_prompt.arguments.len(), 1);
        assert_eq!(listed_prompt.arguments[0].name, "subject");
    }
}
