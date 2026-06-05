use anyhow::{Context, Result, bail};
use clap::Parser;
use reqwest::blocking::Client;
use std::{
    env,
    fs::{self, File},
    io::Write,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::Duration,
};

const CONFORMANCE_BIN: &str = "subcommands_http";
const DOCKER_IMAGE: &str = "clap-mcp-conformance:local";
const INITIALIZE_BODY: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"smoke","version":"1.0"}}}"#;

#[derive(Parser)]
pub struct ConformanceArgs {
    #[arg(long, default_value = "active")]
    pub suite: String,
    #[arg(long, default_value = "conformance-baseline.yml")]
    pub baseline: PathBuf,
    #[arg(long)]
    pub port: Option<u16>,
    #[arg(long)]
    pub verbose: bool,
    #[arg(long)]
    pub no_build: bool,
    #[arg(long)]
    pub rebuild_image: bool,
}

#[derive(Parser)]
pub struct ConformanceServerArgs {
    #[arg(long)]
    pub port: Option<u16>,
    #[arg(long, default_value = "target/conformance-server.log")]
    pub log_file: PathBuf,
}

pub fn run_conformance(args: ConformanceArgs) -> Result<()> {
    ensure_docker()?;
    ensure_conformance_image(args.rebuild_image)?;

    let root = workspace_root()?;
    let port = args.port.unwrap_or_else(ephemeral_port);
    let addr = format!("127.0.0.1:{port}");

    if !args.no_build {
        build_server(&root)?;
    }

    let binary = server_binary_path(&root);
    if !binary.exists() {
        bail!(
            "server binary not found at {}; run without --no-build",
            binary.display()
        );
    }

    let mut server = spawn_server(&binary, &addr, None)?;
    let exit = match wait_ready(&format!("http://{addr}/mcp")) {
        Ok(()) => run_conformance_docker(&root, port, &args.suite, &args.baseline, args.verbose),
        Err(e) => Err(e),
    };
    stop_server(&mut server);
    exit
}

pub fn run_conformance_server(args: ConformanceServerArgs) -> Result<()> {
    let root = workspace_root()?;
    build_server(&root)?;

    let port = args.port.unwrap_or_else(ephemeral_port);
    let addr = format!("127.0.0.1:{port}");

    if let Some(parent) = args.log_file.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let log = File::create(&args.log_file)
        .with_context(|| format!("create log file {}", args.log_file.display()))?;
    let log_err = log.try_clone().context("clone log file handle")?;

    let mut server = spawn_server(&server_binary_path(&root), &addr, Some((log, log_err)))?;
    wait_ready(&format!("http://{addr}/mcp"))
        .with_context(|| format!("MCP server not ready at http://{addr}/mcp"))?;

    export_conformance_port(port)?;
    eprintln!(
        "conformance server listening on http://{addr}/mcp (pid {})",
        server.child.id()
    );

    // CI: keep process alive until workflow job ends; local: block on server exit.
    if env::var_os("GITHUB_ACTIONS").is_some() {
        loop {
            if let Some(status) = server.child.try_wait()? {
                bail!("conformance server exited unexpectedly: {status}");
            }
            thread::sleep(Duration::from_secs(1));
        }
    } else {
        let status = server.child.wait()?;
        if !status.success() {
            bail!("conformance server exited with {status}");
        }
        Ok(())
    }
}

fn workspace_root() -> Result<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .map(Path::to_path_buf)
        .context("xtask manifest has no parent (expected workspace root)")
}

fn conformance_version() -> Result<String> {
    let version_path = workspace_root()?.join("docker/conformance/VERSION");
    let version = fs::read_to_string(&version_path)
        .with_context(|| format!("read {}", version_path.display()))?;
    Ok(version.trim().to_string())
}

fn ensure_docker() -> Result<()> {
    let status = Command::new("docker")
        .arg("info")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("run `docker info` (is Docker installed and running?)")?;
    if status.success() {
        Ok(())
    } else {
        bail!("Docker is not available; install Docker and ensure the daemon is running")
    }
}

fn ensure_conformance_image(rebuild: bool) -> Result<()> {
    let root = workspace_root()?;
    let version = conformance_version()?;
    if !rebuild {
        let inspect = Command::new("docker")
            .args(["image", "inspect", DOCKER_IMAGE])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("docker image inspect")?;
        if inspect.success() {
            return Ok(());
        }
    }

    let dockerfile_dir = root.join("docker/conformance");
    let status = Command::new("docker")
        .args([
            "build",
            "-t",
            DOCKER_IMAGE,
            "--build-arg",
            &format!("CONFORMANCE_VERSION={version}"),
            dockerfile_dir.to_str().context("non-utf8 docker path")?,
        ])
        .status()
        .context("docker build conformance image")?;
    if status.success() {
        Ok(())
    } else {
        bail!("docker build failed with status {status}")
    }
}

fn build_server(root: &Path) -> Result<()> {
    eprintln!("Building {CONFORMANCE_BIN} (http feature)...");
    let status = Command::new("cargo")
        .args([
            "build",
            "-p",
            "clap-mcp-examples",
            "--bin",
            CONFORMANCE_BIN,
            "--features",
            "http",
        ])
        .current_dir(root)
        .status()
        .context("cargo build subcommands_http")?;
    if status.success() {
        Ok(())
    } else {
        bail!("cargo build failed with status {status}")
    }
}

fn server_binary_path(root: &Path) -> PathBuf {
    let exe = env::consts::EXE_SUFFIX;
    root.join("target")
        .join("debug")
        .join(format!("{CONFORMANCE_BIN}{exe}"))
}

fn ephemeral_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local_addr")
        .port()
}

struct ServerHandle {
    child: Child,
}

fn spawn_server(binary: &Path, addr: &str, log: Option<(File, File)>) -> Result<ServerHandle> {
    eprintln!("Starting MCP HTTP server on {addr}...");
    let mut cmd = Command::new(binary);
    cmd.arg("--mcp-http").arg(addr);
    match log {
        Some((out, err)) => {
            cmd.stdout(out).stderr(err);
        }
        None => {
            cmd.stdout(Stdio::null()).stderr(Stdio::null());
        }
    }
    let child = cmd
        .spawn()
        .with_context(|| format!("spawn {}", binary.display()))?;
    Ok(ServerHandle { child })
}

fn stop_server(handle: &mut ServerHandle) {
    let _ = handle.child.kill();
    let _ = handle.child.wait();
}

fn wait_ready(mcp_url: &str) -> Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .context("reqwest client")?;
    for attempt in 0..60 {
        let resp = client
            .post(mcp_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .body(INITIALIZE_BODY)
            .send();
        if resp.is_ok() && resp.unwrap().status().is_success() {
            return Ok(());
        }
        if attempt == 59 {
            bail!("server not ready at {mcp_url} after 15s");
        }
        thread::sleep(Duration::from_millis(250));
    }
    unreachable!()
}

fn run_conformance_docker(
    root: &Path,
    port: u16,
    suite: &str,
    baseline: &Path,
    verbose: bool,
) -> Result<()> {
    let baseline_abs = if baseline.is_absolute() {
        baseline.to_path_buf()
    } else {
        root.join(baseline)
    };
    if !baseline_abs.exists() {
        bail!(
            "baseline file not found: {}; create it or pass --baseline",
            baseline_abs.display()
        );
    }

    let url = conformance_url(port);
    eprintln!("Running conformance harness in Docker against {url} (suite {suite})...");

    let mut docker = Command::new("docker");
    docker.arg("run").arg("--rm");

    if cfg!(target_os = "linux") {
        docker.arg("--network").arg("host");
    } else {
        docker.arg("--add-host=host.docker.internal:host-gateway");
    }

    docker
        .arg("-v")
        .arg(format!("{}:/baseline.yml:ro", baseline_abs.display()))
        .arg(DOCKER_IMAGE)
        .args(["server", "--url", &url, "--suite", suite])
        .arg("--expected-failures")
        .arg("/baseline.yml");

    if verbose {
        docker.arg("--verbose");
    }

    let status = docker.status().context("docker run conformance harness")?;
    eprintln!("Conformance exit code: {status}");
    if status.success() {
        Ok(())
    } else {
        bail!("conformance harness failed with {status}")
    }
}

fn conformance_url(port: u16) -> String {
    if cfg!(target_os = "linux") {
        format!("http://127.0.0.1:{port}/mcp")
    } else {
        format!("http://host.docker.internal:{port}/mcp")
    }
}

fn export_conformance_port(port: u16) -> Result<()> {
    let root = workspace_root()?;
    let port_file = root.join("target/conformance-port");
    if let Some(parent) = port_file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&port_file, format!("CONFORMANCE_PORT={port}\n"))
        .with_context(|| format!("write {}", port_file.display()))?;

    if let Ok(github_env) = env::var("GITHUB_ENV") {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(github_env)
            .context("open GITHUB_ENV")?;
        writeln!(file, "CONFORMANCE_PORT={port}").context("write CONFORMANCE_PORT")?;
    }
    Ok(())
}
