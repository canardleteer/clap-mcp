use anyhow::{Context, Result, bail};
use clap::Parser;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// Examples excluded from CI release `--help` smoke (all other `[[bin]]` names run).
const RELEASE_VALIDATION_EXCLUDE: &[&str] = &[
    // Maintainer conformance fixture; not a user-facing demo.
    "clap-mcp-conformance-http",
    // Imperative `Command::new` only; no clap `Parser` `--help`.
    "placeholder_server",
    // Test fixture for bad executable paths; same.
    "invalid_executable_server",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExamplesProfile {
    Release,
    Http,
    All,
}

impl ExamplesProfile {
    fn parse(s: &str) -> Result<Self> {
        match s {
            "release" | "ci" => Ok(Self::Release),
            "http" => Ok(Self::Http),
            "all" => Ok(Self::All),
            other => bail!("unknown profile `{other}` (expected `release`, `http`, or `all`)"),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Release => "release",
            Self::Http => "http",
            Self::All => "all",
        }
    }
}

#[derive(Debug, Clone)]
struct ExampleBin {
    name: String,
    required_features: Vec<String>,
}

#[derive(Parser)]
pub struct ExamplesHelpArgs {
    /// Print selected example binary names (one per line) and exit.
    #[arg(long)]
    pub list: bool,
    /// Which examples to include: `release` (CI default), `http`, or `all`.
    #[arg(long, default_value = "release")]
    pub profile: String,
    /// Skip `cargo build -p clap-mcp-examples` before running `--help`.
    #[arg(long)]
    pub no_build: bool,
    /// Cargo feature set for build/run (default: `--all-features`).
    #[arg(long)]
    pub features: Option<String>,
}

pub fn run_examples_help(args: ExamplesHelpArgs) -> Result<()> {
    let root = workspace_root()?;
    let profile = ExamplesProfile::parse(&args.profile)?;
    let bins = resolve_bins(&root, profile)?;

    if args.list {
        for name in &bins {
            println!("{name}");
        }
        return Ok(());
    }

    if !args.no_build {
        build_examples(&root, args.features.as_deref())?;
    }

    eprintln!(
        "Running `--help` smoke for {} example(s) (profile `{}`)...",
        bins.len(),
        profile.as_str()
    );
    for name in &bins {
        run_example_help(&root, name, args.features.as_deref())
            .with_context(|| format!("example `{name}` --help smoke"))?;
    }
    eprintln!(
        "Examples `--help` smoke passed (profile `{}`).",
        profile.as_str()
    );
    Ok(())
}

fn resolve_bins(root: &Path, profile: ExamplesProfile) -> Result<Vec<String>> {
    let manifest_bins = load_example_bins(root)?;
    match profile {
        ExamplesProfile::Release => release_validation_bins(&manifest_bins),
        ExamplesProfile::Http => Ok(manifest_bins
            .into_iter()
            .filter(|bin| bin.required_features.iter().any(|f| f == "http"))
            .map(|bin| bin.name)
            .collect()),
        ExamplesProfile::All => Ok(manifest_bins.into_iter().map(|bin| bin.name).collect()),
    }
}

fn release_validation_bins(manifest_bins: &[ExampleBin]) -> Result<Vec<String>> {
    validate_release_excludes(manifest_bins)?;
    Ok(manifest_bins
        .iter()
        .filter(|bin| !RELEASE_VALIDATION_EXCLUDE.contains(&bin.name.as_str()))
        .map(|bin| bin.name.clone())
        .collect())
}

fn validate_release_excludes(manifest_bins: &[ExampleBin]) -> Result<()> {
    let mut seen = std::collections::HashSet::new();
    for name in RELEASE_VALIDATION_EXCLUDE {
        if !seen.insert(*name) {
            bail!("duplicate release validation exclude `{name}`");
        }
        if !manifest_bins.iter().any(|bin| bin.name == *name) {
            bail!("release exclude lists `{name}` but examples/Cargo.toml has no such [[bin]]");
        }
    }
    Ok(())
}

fn load_example_bins(root: &Path) -> Result<Vec<ExampleBin>> {
    let manifest_path = root.join("examples/Cargo.toml");
    let content = fs::read_to_string(&manifest_path)
        .with_context(|| format!("read {}", manifest_path.display()))?;
    parse_example_bins(&content)
}

fn parse_example_bins(content: &str) -> Result<Vec<ExampleBin>> {
    let mut bins = Vec::new();
    let mut in_bin_table = false;
    let mut current_name: Option<String> = None;
    let mut current_features: Vec<String> = Vec::new();

    let mut flush = |name: Option<String>, features: Vec<String>| -> Result<()> {
        if let Some(name) = name {
            bins.push(ExampleBin {
                name,
                required_features: features,
            });
        }
        Ok(())
    };

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[[bin]]" {
            flush(current_name.take(), std::mem::take(&mut current_features))?;
            in_bin_table = true;
            continue;
        }
        if !in_bin_table {
            continue;
        }
        if trimmed.starts_with("[[") && trimmed != "[[bin]]" {
            flush(current_name.take(), std::mem::take(&mut current_features))?;
            in_bin_table = false;
            continue;
        }
        if let Some(name) = trimmed.strip_prefix("name = ") {
            if current_name.is_some() {
                flush(current_name.take(), std::mem::take(&mut current_features))?;
            }
            current_name = Some(parse_toml_string(name)?);
            continue;
        }
        if let Some(raw) = trimmed.strip_prefix("required-features = ") {
            current_features = parse_toml_string_array(raw)?;
        }
    }
    flush(current_name.take(), current_features)?;

    if bins.is_empty() {
        bail!("no [[bin]] targets found in examples/Cargo.toml");
    }
    Ok(bins)
}

fn parse_toml_string(raw: &str) -> Result<String> {
    let raw = raw.trim();
    if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
        Ok(raw[1..raw.len() - 1].to_string())
    } else {
        bail!("expected quoted string, got `{raw}`");
    }
}

fn parse_toml_string_array(raw: &str) -> Result<Vec<String>> {
    let raw = raw.trim();
    if raw == "[]" {
        return Ok(Vec::new());
    }
    if !raw.starts_with('[') || !raw.ends_with(']') {
        bail!("expected array, got `{raw}`");
    }
    let inner = &raw[1..raw.len() - 1];
    inner
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(parse_toml_string)
        .collect()
}

fn build_examples(root: &Path, features: Option<&str>) -> Result<()> {
    eprintln!("Building clap-mcp-examples...");
    let mut cmd = Command::new("cargo");
    cmd.current_dir(root);
    cmd.arg("build").args(["-p", "clap-mcp-examples"]);
    apply_features(&mut cmd, features);
    let status = cmd
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("cargo build clap-mcp-examples")?;
    if status.success() {
        Ok(())
    } else {
        bail!("cargo build failed with {status}");
    }
}

fn run_example_help(root: &Path, bin: &str, features: Option<&str>) -> Result<()> {
    eprintln!("  {bin} --help");
    let mut cmd = Command::new("cargo");
    cmd.current_dir(root);
    cmd.args(["run", "-p", "clap-mcp-examples"]);
    apply_features(&mut cmd, features);
    cmd.args(["--bin", bin, "--", "--help"]);
    let status = cmd
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("cargo run --bin {bin} -- --help"))?;
    if status.success() {
        Ok(())
    } else {
        bail!("`{bin} --help` failed with {status}");
    }
}

fn apply_features(cmd: &mut Command, features: Option<&str>) {
    match features {
        Some(list) if !list.is_empty() => {
            cmd.arg("--features").arg(list);
        }
        Some(_) => {
            cmd.arg("--all-features");
        }
        None => {
            cmd.arg("--all-features");
        }
    }
}

fn workspace_root() -> Result<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .map(Path::to_path_buf)
        .context("xtask manifest has no parent (expected workspace root)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_example_bins_reads_names_and_required_features() {
        let sample = r#"
[package]
name = "not-a-bin"

[[bin]]
name = "plain"

[[bin]]
name = "with_http"
required-features = ["http", "tracing"]
"#;
        let bins = parse_example_bins(sample).expect("parse");
        assert_eq!(bins.len(), 2);
        assert_eq!(bins[0].name, "plain");
        assert!(bins[0].required_features.is_empty());
        assert_eq!(bins[1].name, "with_http");
        assert_eq!(bins[1].required_features, vec!["http", "tracing"]);
    }

    #[test]
    fn release_profile_includes_manifest_bins_unless_excluded() {
        let sample = r#"
[[bin]]
name = "demo"

[[bin]]
name = "clap-mcp-conformance-http"

[[bin]]
name = "placeholder_server"

[[bin]]
name = "invalid_executable_server"
"#;
        let manifest = parse_example_bins(sample).expect("parse");
        let bins = release_validation_bins(&manifest).expect("release");
        assert_eq!(bins, vec!["demo".to_string()]);
    }

    #[test]
    fn release_exclude_names_exist_in_manifest() {
        let root = workspace_root().expect("root");
        let manifest = load_example_bins(&root).expect("manifest");
        validate_release_excludes(&manifest).expect("excludes valid");
    }

    #[test]
    fn release_profile_includes_new_examples() {
        let root = workspace_root().expect("root");
        let bins = resolve_bins(&root, ExamplesProfile::Release).expect("release bins");
        assert!(bins.iter().any(|name| name == "nested_subcommands"));
        assert!(bins.iter().any(|name| name == "struct_subcommand_globals"));
        assert!(!bins.iter().any(|name| name == "clap-mcp-conformance-http"));
    }
}
