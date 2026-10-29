//! Spawns a plugin subprocess under the platform CLI contract:
//! `<venv>/bin/python <plugin>/main.py --input payload.json --output result.json`
//! with cwd = plugin dir (mirrors `plugin_manager::executor`).
//!
//! Enforces the process-group kill on timeout like the app, and validates the
//! result file against the schema rules that matter here: execution_id match,
//! status enum, and artifact paths existing inside `output_dir` (the app's
//! sandbox rule — the plugin may not write outside its assigned dir).

use std::io::{BufRead, BufReader};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde_json::Value;

#[derive(Debug)]
pub struct RunOutcome {
    pub result: Value,
    pub wall_time: Duration,
}

/// Default cap when the manifest has no `execution.timeout_seconds`.
const DEFAULT_TIMEOUT_SECS: u64 = 300;

pub fn manifest_timeout(plugin_dir: &Path) -> u64 {
    let path = plugin_dir.join("manifest.json");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .and_then(|m| {
            m.get("execution")
                .and_then(|e| e.get("timeout_seconds"))
                .and_then(Value::as_u64)
        })
        .unwrap_or(DEFAULT_TIMEOUT_SECS)
}

fn interpreter(plugin_dir: &Path) -> PathBuf {
    let venv = plugin_dir.join(".venv").join("bin").join("python");
    if venv.exists() {
        venv
    } else {
        PathBuf::from("python3")
    }
}

fn kill_process_group(pid: u32) {
    #[cfg(unix)]
    {
        // Child was spawned with process_group(0); negative pid signals the group.
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
    }
}

fn spawn(plugin_dir: &Path, payload_path: &Path, result_path: &Path) -> Result<Child> {
    // Command::new resolves relative paths against the child's cwd, so an
    // absolute plugin dir is required before current_dir() takes effect.
    let plugin_dir = plugin_dir
        .canonicalize()
        .with_context(|| format!("canonicalize {}", plugin_dir.display()))?;
    let python = interpreter(&plugin_dir);
    let entry = plugin_dir.join("main.py");
    if !entry.exists() {
        bail!("plugin entrypoint not found: {}", entry.display());
    }
    // Plugin runs with cwd = plugin_dir; payload/result paths must be absolute.
    let payload_path = payload_path
        .canonicalize()
        .with_context(|| format!("canonicalize {}", payload_path.display()))?;
    let result_path = std::path::absolute(result_path)
        .with_context(|| format!("absolute path {}", result_path.display()))?;
    let mut cmd = Command::new(&python);
    cmd.arg(&entry)
        .arg("--input")
        .arg(payload_path)
        .arg("--output")
        .arg(result_path)
        .current_dir(plugin_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        cmd.process_group(0);
    }
    cmd.spawn()
        .with_context(|| format!("spawn {} {}", python.display(), entry.display()))
}

/// Drain a pipe on a worker thread; echo `PROGRESS:` lines, log the rest.
fn pump<R: std::io::Read + Send + 'static>(reader: R, label: &'static str) {
    std::thread::spawn(move || {
        for line in BufReader::new(reader).lines().map_while(Result::ok) {
            if line.trim_start().starts_with("PROGRESS:") {
                println!("[progress] {label}: {}", line.trim());
            } else {
                eprintln!("[{label}] {line}");
            }
        }
    });
}

pub fn run_plugin(
    plugin_dir: &Path,
    payload_path: &Path,
    result_path: &Path,
    timeout: Duration,
) -> Result<RunOutcome> {
    let mut child = spawn(plugin_dir, payload_path, result_path)?;
    let stdout = child.stdout.take().context("child stdout")?;
    let stderr = child.stderr.take().context("child stderr")?;
    pump(stdout, "stdout");
    pump(stderr, "stderr");

    let started = Instant::now();
    let pid = child.id();
    let (tx, rx) = mpsc::channel();
    let waiter = std::thread::spawn(move || {
        let status = child.wait();
        let _ = tx.send(status);
    });

    let status = match rx.recv_timeout(timeout) {
        Ok(status) => status,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            kill_process_group(pid);
            // Waiter reaps the corpse after the kill.
            waiter.join().ok();
            bail!(
                "plugin timed out after {}s; process group killed",
                timeout.as_secs()
            );
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            kill_process_group(pid);
            bail!("plugin waiter thread died without result");
        }
    };
    drop(waiter);

    let wall_time = started.elapsed();
    let status = status.context("wait for plugin process")?;
    if !status.success() {
        // Some plugins write a failure result.json and exit non-zero; check
        // for the file before declaring the run dead.
        if !result_path.exists() {
            bail!("plugin exited with {status} and wrote no result.json");
        }
    }

    let raw = std::fs::read_to_string(result_path)
        .with_context(|| format!("read {}", result_path.display()))?;
    let result: Value =
        serde_json::from_str(&raw).with_context(|| format!("parse {}", result_path.display()))?;
    Ok(RunOutcome { result, wall_time })
}

/// Schema-level checks from `execution_result.schema.json`:
/// required fields, status enum, execution_id match, artifact paths inside
/// `output_dir` (canonicalized so `..` tricks are caught).
pub fn validate_result(result: &Value, execution_id: &str, output_dir: &Path) -> Result<()> {
    let Some(obj) = result.as_object() else {
        bail!("result.json is not a JSON object");
    };
    for key in [
        "execution_id",
        "status",
        "execution_time_ms",
        "metrics",
        "artifacts",
    ] {
        if !obj.contains_key(key) {
            bail!("result.json missing required field {key:?}");
        }
    }
    if obj["execution_id"].as_str() != Some(execution_id) {
        bail!(
            "result execution_id {:?} does not match payload {execution_id:?}",
            obj["execution_id"].as_str().unwrap_or("<non-string>")
        );
    }
    match obj["status"].as_str() {
        Some("success") | Some("warning") | Some("failure") => {}
        other => bail!("invalid result status {other:?}"),
    }
    let out_canon = output_dir
        .canonicalize()
        .with_context(|| format!("canonicalize {}", output_dir.display()))?;
    let artifacts = obj["artifacts"]
        .as_object()
        .context("artifacts not an object")?;
    for (key, art) in artifacts {
        let Some(path_str) = art.get("file_path").and_then(Value::as_str) else {
            bail!("artifact {key:?} missing file_path");
        };
        let path = PathBuf::from(path_str);
        if !path.exists() {
            bail!("artifact {key:?} file missing: {path_str}");
        }
        let canon = path
            .canonicalize()
            .with_context(|| format!("canonicalize artifact {path_str}"))?;
        if !canon.starts_with(&out_canon) {
            bail!("artifact {key:?} escapes output_dir sandbox: {path_str}");
        }
    }
    Ok(())
}
