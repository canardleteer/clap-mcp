use anyhow::{Context, Result, bail};
use clap::Parser;
use std::{
    env,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Parser)]
pub struct CodeCoverageHtmlArgs {
    /// Open the HTML report in a browser after generation.
    #[arg(long)]
    pub open: bool,
    /// Directory for the HTML report (default: `target/llvm-cov/html`).
    #[arg(long)]
    pub output_dir: Option<PathBuf>,
}

pub fn run_code_coverage_html(args: CodeCoverageHtmlArgs) -> Result<()> {
    ensure_llvm_cov()?;

    let root = workspace_root()?;
    let mut cmd = Command::new("cargo");
    cmd.current_dir(&root);
    cmd.args([
        "llvm-cov",
        "test",
        "-p",
        "clap-mcp",
        "-p",
        "clap-mcp-macros",
        "--all-features",
    ]);

    if args.open {
        cmd.arg("--open");
    } else {
        cmd.arg("--html");
    }

    if let Some(output_dir) = &args.output_dir {
        cmd.arg("--output-dir").arg(output_dir);
    }

    let status = cmd
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("run `cargo llvm-cov` (is cargo-llvm-cov installed?)")?;

    if status.success() {
        if !args.open {
            let report_dir = args
                .output_dir
                .unwrap_or_else(|| root.join("target/llvm-cov/html"));
            eprintln!(
                "HTML coverage report: {}",
                report_dir.join("index.html").display()
            );
        }
        Ok(())
    } else {
        bail!("cargo llvm-cov failed with {status}");
    }
}

fn workspace_root() -> Result<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .map(Path::to_path_buf)
        .context("xtask manifest has no parent (expected workspace root)")
}

fn ensure_llvm_cov() -> Result<()> {
    let status = Command::new("cargo")
        .args(["llvm-cov", "--version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("run `cargo llvm-cov --version`")?;
    if status.success() {
        Ok(())
    } else {
        bail!("cargo-llvm-cov is not installed; run `cargo install cargo-llvm-cov` and retry")
    }
}
