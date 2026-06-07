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

const CONFORMANCE_BIN: &str = "clap-mcp-conformance-http";
const DOCKER_IMAGE: &str = "clap-mcp-conformance:local";
const DEFAULT_LOG_MAX_MB: u64 = 10;
const INITIALIZE_BODY: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"smoke","version":"1.0"}}}"#;
const SERVER_PID_FILE: &str = "target/conformance-server.pid";
const SERVER_LOG_FILE: &str = "target/conformance-server.log";
const SERVER_PORT_FILE: &str = "target/conformance-port";

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
    /// Cap server stdout/stderr capture size (Linux/macOS: `RLIMIT_FSIZE` on the child).
    #[arg(long, default_value_t = DEFAULT_LOG_MAX_MB)]
    pub log_max_mb: u64,
    /// Start even when a conformance server pid file or orphan process is present.
    #[arg(long)]
    pub force: bool,
}

#[derive(Parser)]
pub struct ConformanceStopArgs {}

pub fn run_conformance_stop(_args: ConformanceStopArgs) -> Result<()> {
    let root = workspace_root()?;
    let stopped = stop_conformance_servers(&root)?;
    if stopped.is_empty() {
        eprintln!("No conformance server processes found.");
    } else {
        eprintln!(
            "Stopped conformance server process(es): {}",
            format_pids(&stopped)
        );
    }
    Ok(())
}

pub fn run_conformance(args: ConformanceArgs) -> Result<()> {
    ensure_docker()?;
    ensure_conformance_image(args.rebuild_image)?;

    let root = workspace_root()?;
    let stopped = stop_conformance_servers(&root)?;
    if !stopped.is_empty() {
        eprintln!(
            "Stopped stale conformance server process(es) before run: {}",
            format_pids(&stopped)
        );
    }

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

    let mut server = spawn_server(&binary, &addr, None, 0)?;
    let exit = match wait_ready(&format!("http://{addr}/mcp")) {
        Ok(()) => run_conformance_docker(&root, port, &args.suite, &args.baseline, args.verbose),
        Err(e) => Err(e),
    };
    stop_server(&mut server);
    exit
}

pub fn run_conformance_server(args: ConformanceServerArgs) -> Result<()> {
    let root = workspace_root()?;
    if !args.force {
        ensure_no_stale_conformance_server(&root)?;
    } else {
        let stopped = stop_conformance_servers(&root)?;
        if !stopped.is_empty() {
            eprintln!(
                "Stopped existing conformance server process(es): {}",
                format_pids(&stopped)
            );
        }
    }
    build_server(&root)?;

    let port = args.port.unwrap_or_else(ephemeral_port);
    let addr = format!("127.0.0.1:{port}");

    if let Some(parent) = args.log_file.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let log = File::create(&args.log_file)
        .with_context(|| format!("create log file {}", args.log_file.display()))?;
    let log_err = log.try_clone().context("clone log file handle")?;
    let log_max_bytes = args.log_max_mb.saturating_mul(1024 * 1024);

    let mut server = spawn_server(
        &server_binary_path(&root),
        &addr,
        Some((log, log_err)),
        log_max_bytes,
    )?;
    write_server_pid(&root, server.child.id())?;
    wait_ready(&format!("http://{addr}/mcp"))
        .with_context(|| format!("MCP server not ready at http://{addr}/mcp"))?;

    export_conformance_port(port)?;
    eprintln!(
        "conformance server listening on http://{addr}/mcp (pid {}, log max {} MiB, pid file {SERVER_PID_FILE})",
        server.child.id(),
        args.log_max_mb,
    );
    eprintln!(
        "CI/debug only: prefer `cargo xtask conformance` locally; stop with `cargo xtask conformance-stop`."
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
    eprintln!("Building {CONFORMANCE_BIN} (http + tracing features)...");
    let status = Command::new("cargo")
        .args([
            "build",
            "-p",
            "clap-mcp-examples",
            "--bin",
            CONFORMANCE_BIN,
            "--features",
            "http,tracing",
        ])
        .current_dir(root)
        .status()
        .context("cargo build clap-mcp-conformance-http")?;
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

impl Drop for ServerHandle {
    fn drop(&mut self) {
        stop_server(self);
        if let Ok(root) = workspace_root() {
            remove_server_pid(&root);
        }
    }
}

fn write_server_pid(root: &Path, pid: u32) -> Result<()> {
    let pid_file = root.join(SERVER_PID_FILE);
    if let Some(parent) = pid_file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&pid_file, format!("{pid}\n"))
        .with_context(|| format!("write {}", pid_file.display()))
}

fn remove_server_pid(root: &Path) {
    let pid_file = root.join(SERVER_PID_FILE);
    let _ = fs::remove_file(pid_file);
}

fn spawn_server(
    binary: &Path,
    addr: &str,
    log: Option<(File, File)>,
    log_max_bytes: u64,
) -> Result<ServerHandle> {
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
    configure_child_process(&mut cmd, log_max_bytes);
    let child = cmd
        .spawn()
        .with_context(|| format!("spawn {}", binary.display()))?;
    Ok(ServerHandle { child })
}

#[cfg(unix)]
fn configure_child_process(cmd: &mut Command, log_max_bytes: u64) {
    use std::os::unix::process::CommandExt;

    // SAFETY: `pre_exec` runs in the child between fork and exec. Only async-signal-safe
    // calls are used (`prctl`, `setrlimit`). Failures propagate to `spawn()` as errors.
    unsafe {
        cmd.pre_exec(move || {
            #[cfg(target_os = "linux")]
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            if log_max_bytes > 0 {
                let rlim = libc::rlimit {
                    rlim_cur: log_max_bytes,
                    rlim_max: log_max_bytes,
                };
                if libc::setrlimit(libc::RLIMIT_FSIZE, &rlim) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
}

#[cfg(not(unix))]
fn configure_child_process(_cmd: &mut Command, _log_max_bytes: u64) {}

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
    let port_file = root.join(SERVER_PORT_FILE);
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

fn format_pids(pids: &[u32]) -> String {
    pids.iter()
        .map(|pid| pid.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(unix)]
fn conformance_binary_paths(root: &Path) -> Vec<PathBuf> {
    let exe = env::consts::EXE_SUFFIX;
    ["debug", "release"]
        .into_iter()
        .map(|profile| {
            root.join("target")
                .join(profile)
                .join(format!("{CONFORMANCE_BIN}{exe}"))
        })
        .collect()
}

fn pid_is_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // SAFETY: kill(pid, 0) probes existence without sending a signal.
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

fn terminate_pid(pid: u32) -> Result<()> {
    #[cfg(unix)]
    {
        // SAFETY: signals are used to stop a known child/orphan conformance server.
        unsafe {
            if libc::kill(pid as i32, libc::SIGTERM) == -1 {
                return Err(std::io::Error::last_os_error()).context("SIGTERM conformance server");
            }
        }
        for _ in 0..20 {
            if !pid_is_alive(pid) {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(100));
        }
        // SAFETY: best-effort hard stop when SIGTERM did not exit in time.
        unsafe {
            let _ = libc::kill(pid as i32, libc::SIGKILL);
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let status = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status()
            .context("taskkill conformance server")?;
        if status.success() {
            Ok(())
        } else {
            bail!("taskkill failed with status {status}");
        }
    }
}

fn read_pid_file(root: &Path) -> Option<u32> {
    let pid_file = root.join(SERVER_PID_FILE);
    let contents = fs::read_to_string(&pid_file).ok()?;
    let pid = contents.trim().parse().ok()?;
    Some(pid)
}

#[cfg(unix)]
fn collect_orphan_conformance_pids(root: &Path) -> Result<Vec<u32>> {
    let targets: Vec<String> = conformance_binary_paths(root)
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let output = Command::new("ps")
        .args(["-e", "-o", "pid=,args="])
        .output()
        .context("ps")?;
    if !output.status.success() {
        bail!("ps failed with status {}", output.status);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut pids = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((pid_str, cmd)) = line.split_once(' ') else {
            continue;
        };
        let Ok(pid) = pid_str.trim().parse::<u32>() else {
            continue;
        };
        if targets
            .iter()
            .any(|target| cmd.starts_with(target.as_str()))
        {
            pids.push(pid);
        }
    }
    pids.sort_unstable();
    pids.dedup();
    Ok(pids)
}

#[cfg(not(unix))]
fn collect_orphan_conformance_pids(_root: &Path) -> Result<Vec<u32>> {
    Ok(Vec::new())
}

fn remove_conformance_artifacts(root: &Path) {
    for path in [
        root.join(SERVER_PID_FILE),
        root.join(SERVER_LOG_FILE),
        root.join(SERVER_PORT_FILE),
    ] {
        let _ = fs::remove_file(path);
    }
}

fn stop_conformance_servers(root: &Path) -> Result<Vec<u32>> {
    let mut stopped = Vec::new();

    if let Some(pid) = read_pid_file(root)
        && pid_is_alive(pid)
    {
        terminate_pid(pid)?;
        stopped.push(pid);
    }

    for pid in collect_orphan_conformance_pids(root)? {
        if stopped.contains(&pid) {
            continue;
        }
        if pid_is_alive(pid) {
            terminate_pid(pid)?;
            stopped.push(pid);
        }
    }

    remove_conformance_artifacts(root);
    Ok(stopped)
}

fn ensure_no_stale_conformance_server(root: &Path) -> Result<()> {
    let pid_file = read_pid_file(root);
    if pid_file.is_some_and(pid_is_alive) {
        bail!(
            "conformance server already running (pid file {SERVER_PID_FILE}). \
             Prefer `cargo xtask conformance` locally, or run `cargo xtask conformance-stop` \
             before `cargo xtask conformance-server`."
        );
    }

    let orphans = collect_orphan_conformance_pids(root)?;
    if orphans.iter().any(|pid| pid_is_alive(*pid)) {
        bail!(
            "orphan {CONFORMANCE_BIN} process(es) detected ({orphans:?}). \
             Run `cargo xtask conformance-stop`, or pass `--force` to conformance-server."
        );
    }

    Ok(())
}
