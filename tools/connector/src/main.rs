//! External connector for the Aerial Analytics Platform.
//!
//! Reads the app's SQLite database (read-only) so later stages can pick
//! session images without the app UI. No app code is modified.

mod db;
mod models;
mod payload;
mod publish;
mod runner;

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use serde_json::Value;

use models::{ImageRow, ProjectRow, SessionRow};

#[derive(Parser)]
#[command(
    name = "connector",
    about = "Bridge between app data (SQLite) and external plugin/map operations"
)]
struct Cli {
    /// Override database path (default: AAP_DB_PATH, then app-data location).
    #[arg(long, global = true)]
    db: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List projects with session/image counts.
    Projects,
    /// List sessions in a project (id or name).
    Sessions {
        /// Project id, or unique project name prefix.
        project: String,
    },
    /// List images in a session (id, or unique session name prefix).
    Session {
        /// Session id, or unique session name.
        session: String,
        /// Also print the absolute file path of each image.
        #[arg(long)]
        paths: bool,
        /// Emit JSON instead of a table.
        #[arg(long)]
        json: bool,
    },
    /// Run a plugin on one session image (payload + subprocess + result check).
    Run {
        /// Session id, or unique session name.
        #[arg(long)]
        session: String,
        /// Image file name (e.g. 01_0001.JPG) or image id.
        #[arg(long)]
        image: String,
        /// Plugin directory (contains main.py + manifest.json).
        #[arg(long)]
        plugin: PathBuf,
        /// JSON object overriding plugin parameter defaults.
        #[arg(long)]
        params_file: Option<PathBuf>,
        /// Override the manifest timeout (seconds).
        #[arg(long)]
        timeout: Option<u64>,
        /// Publish the result into map_storage after a successful run.
        #[arg(long)]
        publish: bool,
        /// Layer name for the published layer (default: "Connector: <file>").
        #[arg(long)]
        layer_name: Option<String>,
        /// Map storage root (default: AAP_STORAGE_DIR, then ./map_storage).
        #[arg(long)]
        storage: Option<PathBuf>,
    },
    /// Publish an existing run directory into map_storage (Map Explorer).
    Publish {
        /// Run directory containing payload.json + result.json.
        #[arg(long)]
        run: PathBuf,
        /// Layer name (default: "Connector: <file>").
        #[arg(long)]
        layer_name: Option<String>,
        /// Map storage root (default: AAP_STORAGE_DIR, then ./map_storage).
        #[arg(long)]
        storage: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    // Behave like a normal Unix CLI: die silently on SIGPIPE (`... | head`).
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
    let cli = Cli::parse();
    let db_path = db::resolve_db_path(cli.db.as_deref())?;
    let conn = db::open_readonly(&db_path)?;

    match cli.command {
        Command::Projects => {
            let projects = db::list_projects(&conn)?;
            if projects.is_empty() {
                bail!("no projects in {}", db_path.display());
            }
            print_projects(&projects);
        }
        Command::Sessions { project } => {
            let project_id = resolve_project(&conn, &project)?;
            let sessions = db::list_sessions(&conn, &project_id)?;
            if sessions.is_empty() {
                bail!("no sessions in project {project_id}");
            }
            print_sessions(&sessions);
        }
        Command::Session {
            session,
            paths,
            json,
        } => {
            let session_id = resolve_session(&conn, &session)?;
            let images = db::list_images(&conn, &session_id)?;
            if images.is_empty() {
                bail!("no images in session {session_id}");
            }
            if json {
                println!("{}", serde_json::to_string_pretty(&images)?);
            } else {
                print_images(&images, paths);
            }
        }
        Command::Run {
            session,
            image,
            plugin,
            params_file,
            timeout,
            publish: do_publish,
            layer_name,
            storage,
        } => {
            run(
                &conn,
                &session,
                &image,
                &plugin,
                params_file.as_deref(),
                timeout,
                PublishOpts {
                    do_publish,
                    layer_name: layer_name.as_deref(),
                    storage: storage.as_deref(),
                },
            )?;
        }
        Command::Publish {
            run,
            layer_name,
            storage,
        } => {
            let out = publish::publish(
                &run,
                &publish::resolve_storage_root(storage.as_deref()),
                layer_name.as_deref(),
            )?;
            println!(
                "# published layer {} ({} features) -> {}",
                out.layer_id,
                out.feature_count,
                out.layer_path.display()
            );
        }
    }
    Ok(())
}

struct PublishOpts<'a> {
    do_publish: bool,
    layer_name: Option<&'a str>,
    storage: Option<&'a Path>,
}

fn run(
    conn: &rusqlite::Connection,
    session: &str,
    image_ref: &str,
    plugin_dir: &Path,
    params_file: Option<&Path>,
    timeout_secs: Option<u64>,
    publish_opts: PublishOpts<'_>,
) -> Result<()> {
    if !plugin_dir.join("main.py").exists() {
        bail!("no main.py in {}", plugin_dir.display());
    }
    let session_id = resolve_session(conn, session)?;
    let images = db::list_images(conn, &session_id)?;
    if images.is_empty() {
        bail!("no images in session {session_id}");
    }
    let image = resolve_image(&images, image_ref)?;
    if image.analysis_ready == Some(false) {
        bail!(
            "image {} is not analysis-ready (analysis_ready=false)",
            image.file_name()
        );
    }

    let defaults = payload::default_parameters(plugin_dir)?;
    let overrides = match params_file {
        Some(path) => {
            let raw = std::fs::read_to_string(path)
                .with_context(|| format!("read {}", path.display()))?;
            serde_json::from_str::<serde_json::Map<String, Value>>(&raw)
                .with_context(|| format!("{} must be a JSON object", path.display()))?
        }
        None => serde_json::Map::new(),
    };
    let parameters = payload::merge_parameters(defaults, overrides);

    let execution_id = format!("conn_{}", uuid::Uuid::new_v4());
    // Repo-root relative: `runs/` is gitignored; absolute would hide the
    // results from the project tree.
    let runs_root = std::env::var("AAP_RUNS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("runs"));
    let output_dir = runs_root.join(&execution_id);
    std::fs::create_dir_all(&output_dir)
        .with_context(|| format!("create {}", output_dir.display()))?;
    // Plugin cwd is the plugin dir; payload fields must be absolute.
    let output_dir = output_dir
        .canonicalize()
        .with_context(|| format!("canonicalize {}", output_dir.display()))?;

    let payload = payload::build(&execution_id, &output_dir, &session_id, image, parameters)?;
    let payload_path = output_dir.join("payload.json");
    let result_path = output_dir.join("result.json");
    std::fs::write(&payload_path, serde_json::to_string_pretty(&payload)?)
        .with_context(|| format!("write {}", payload_path.display()))?;

    println!(
        "running {} on {} (session {session})",
        plugin_dir.display(),
        image.file_name()
    );
    println!("output: {}", output_dir.display());

    let timeout =
        Duration::from_secs(timeout_secs.unwrap_or_else(|| runner::manifest_timeout(plugin_dir)));
    let outcome = runner::run_plugin(plugin_dir, &payload_path, &result_path, timeout)
        .with_context(|| {
            format!(
                "plugin run failed; artifacts left in {}",
                output_dir.display()
            )
        })?;
    runner::validate_result(&outcome.result, &execution_id, &output_dir)?;

    print_run_report(&outcome.result, outcome.wall_time);
    println!("result: {}", result_path.display());
    if publish_opts.do_publish {
        let out = publish::publish(
            &output_dir,
            &publish::resolve_storage_root(publish_opts.storage),
            publish_opts.layer_name,
        )?;
        println!(
            "# published layer {} ({} features) -> {} (reopen Map Explorer to see it)",
            out.layer_id,
            out.feature_count,
            out.layer_path.display()
        );
    }
    Ok(())
}

/// Exact file name, exact image id, or unique file-name prefix — else error.
fn resolve_image<'a>(images: &'a [ImageRow], image_ref: &str) -> Result<&'a ImageRow> {
    if let Some(hit) = images.iter().find(|i| i.id == image_ref) {
        return Ok(hit);
    }
    if let Some(hit) = images.iter().find(|i| i.file_name() == image_ref) {
        return Ok(hit);
    }
    let matches: Vec<&ImageRow> = images
        .iter()
        .filter(|i| i.file_name().starts_with(image_ref))
        .collect();
    match matches.len() {
        0 => bail!("no image matches {image_ref:?}"),
        1 => Ok(matches.into_iter().next().expect("len checked")),
        _ => bail!(
            "ambiguous image {image_ref:?}: {}",
            matches
                .iter()
                .take(5)
                .map(|i| i.file_name())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn print_run_report(result: &Value, wall_time: Duration) {
    let status = result["status"].as_str().unwrap_or("?");
    let ms = result["execution_time_ms"].as_i64().unwrap_or(-1);
    println!(
        "# status={status} plugin_time={ms}ms wall={:.1}s",
        wall_time.as_secs_f64()
    );
    if let Some(err) = result.get("error").and_then(Value::as_str) {
        println!("# error: {err}");
    }
    println!("{:<24}  METRIC", "KEY");
    if let Some(metrics) = result["metrics"].as_object() {
        for (k, v) in metrics {
            println!("{k:<24} {v:>10}");
        }
    }
    println!("{:<24} ARTIFACT", "KEY");
    if let Some(artifacts) = result["artifacts"].as_object() {
        for (k, a) in artifacts {
            println!("{k:<24} {}", a["file_path"].as_str().unwrap_or("?"));
        }
    }
}

fn resolve_project(conn: &rusqlite::Connection, project: &str) -> Result<String> {
    if project.contains('-') {
        // UUID-shaped: trust it, but confirm.
        let known: Vec<ProjectRow> = db::list_projects(conn)?;
        if let Some(hit) = known.iter().find(|p| p.id == project) {
            return Ok(hit.id.clone());
        }
    }
    let matches: Vec<ProjectRow> = db::list_projects(conn)?
        .into_iter()
        .filter(|p| p.id.starts_with(project) || p.name.starts_with(project))
        .collect();
    match matches.len() {
        0 => bail!("no project matches {project:?}"),
        1 => Ok(matches.into_iter().next().expect("len checked").id),
        _ => bail!(
            "ambiguous project {project:?}: {}",
            matches
                .iter()
                .map(|p| format!("{} ({})", p.name, p.id))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn resolve_session(conn: &rusqlite::Connection, session: &str) -> Result<String> {
    // Session ids are UUIDs; otherwise match by name across all sessions.
    let mut stmt = conn.prepare(
        "SELECT id FROM session WHERE id = ?1
          UNION
         SELECT id FROM session WHERE name = ?1",
    )?;
    let ids: Vec<String> = stmt
        .query_map([session], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    match ids.len() {
        0 => bail!("no session matches {session:?}"),
        1 => Ok(ids.into_iter().next().expect("len checked")),
        _ => bail!(
            "ambiguous session {session:?}: pass the id from `sessions` command ({})",
            ids.join(", ")
        ),
    }
}

fn print_projects(projects: &[ProjectRow]) {
    println!(
        "{:<38} {:<24} {:>4} {:>4}  DESCRIPTION",
        "PROJECT_ID", "NAME", "SESS", "IMGS"
    );
    for p in projects {
        println!(
            "{:<38} {:<24} {:>4} {:>4}  {}",
            p.id,
            p.name,
            p.session_count,
            p.image_count,
            p.description.as_deref().unwrap_or("-")
        );
    }
}

fn print_sessions(sessions: &[SessionRow]) {
    println!(
        "{:<38} {:<20} {:<8} {:>4} {:>4}  DATE",
        "SESSION_ID", "NAME", "STATUS", "IMGS", "GEO"
    );
    for s in sessions {
        println!(
            "{:<38} {:<20} {:<8} {:>4} {:>4}  {}",
            s.id,
            s.name,
            s.status,
            s.image_count,
            s.geotagged_count,
            s.date_start.as_deref().unwrap_or("-")
        );
    }
}

fn print_images(images: &[ImageRow], with_paths: bool) {
    let geo = images.iter().filter(|i| i.has_gps()).count();
    println!("# {} images, {geo} geotagged", images.len());
    if with_paths {
        println!(
            "{:<28} {:>10} {:>11} {:>20} {:>7} PATH",
            "FILE", "LAT", "LON", "CAPTURED", "READY"
        );
    } else {
        println!(
            "{:<28} {:>10} {:>11} {:>20} {:>7}",
            "FILE", "LAT", "LON", "CAPTURED", "READY"
        );
    }
    for i in images {
        let lat = i
            .location_lat
            .map(|v| format!("{v:.7}"))
            .unwrap_or_else(|| "-".into());
        let lon = i
            .location_lon
            .map(|v| format!("{v:.7}"))
            .unwrap_or_else(|| "-".into());
        let ready = i
            .analysis_ready
            .map(|b| if b { "yes" } else { "no" })
            .unwrap_or("-");
        if with_paths {
            println!(
                "{:<28} {:>10} {:>11} {:>20} {:>7} {}",
                i.file_name(),
                lat,
                lon,
                i.captured_at.as_deref().unwrap_or("-"),
                ready,
                i.file_path
            );
        } else {
            println!(
                "{:<28} {:>10} {:>11} {:>20} {:>7}",
                i.file_name(),
                lat,
                lon,
                i.captured_at.as_deref().unwrap_or("-"),
                ready
            );
        }
    }
}
