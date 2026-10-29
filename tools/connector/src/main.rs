//! External connector for the Aerial Analytics Platform.
//!
//! Reads the app's SQLite database (read-only) so later stages can pick
//! session images without the app UI. No app code is modified.

mod db;
mod models;

use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};

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
    }
    Ok(())
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
