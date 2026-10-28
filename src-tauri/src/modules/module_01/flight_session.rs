#![allow(dead_code)]

//! Flight Session Management (Module 1.4).
//!
//! Backs the `commands::session` Tauri command stubs with real logic:
//! persisting sessions to SQLite, and deriving a session's date range from
//! the EXIF `captured_at` of the images assigned to it.
//!
//! ## Purity Boundary
//!
//! Every function that touches the database (i.e. takes a `&Connection`) is
//! impure. [`compute_date_range`] and [`default_session_name`] are the only
//! pure functions in this module — they can be unit-tested with in-memory
//! fixtures, no database required.
//!
//! ## Open integration question
//!
//! Resolved 2026-10-08 (M1-24): this module owns the `image` table,
//! including the extracted-metadata columns. [`insert_image`] persists a
//! record built by the pure [`image_from_metadata`] from an
//! [`ImageMetadata`](crate::models::image::ImageMetadata); [`get_image`]
//! and [`get_images_by_session`] read records back.

use chrono::{DateTime, NaiveDateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use uuid::Uuid;

use super::blur_detection::SessionBlurStats;
use super::error::SessionError;
use crate::models::image::ImageMetadata;
use crate::models::{Image, Project, Session, SessionStatus};

/// Column definitions for the `image` table, shared by [`init_schema`]
/// (fresh databases) and [`migrate_image_schema`] (table rebuild for
/// outdated databases).
const IMAGE_TABLE_COLUMNS_DDL: &str = "id            TEXT PRIMARY KEY,
            session_id    TEXT NOT NULL REFERENCES session(id),
            file_path     TEXT NOT NULL UNIQUE,
            location_lat  REAL,
            location_lon  REAL,
            captured_at   TEXT,
            gps_altitude_m REAL,
            width         INTEGER,
            height        INTEGER,
            format        TEXT,
            make          TEXT,
            camera_model  TEXT,
            source_path   TEXT,
            content_hash  TEXT,
            file_size     INTEGER,
            raw_sibling_id TEXT,
            analysis_ready INTEGER,
            blur_score    REAL,
            is_blurry     INTEGER";

/// Columns of the `image` table in canonical order, for `SELECT`s and for
/// the data-preserving copy in [`migrate_image_schema`].
const IMAGE_COLUMNS: &str = "id, session_id, file_path, location_lat, location_lon, \
    captured_at, gps_altitude_m, width, height, format, make, camera_model, \
    source_path, content_hash, file_size, raw_sibling_id, analysis_ready, \
    blur_score, is_blurry";

/// Canonical `image` columns. A table carrying all of them with nullable
/// location columns is already current and is never rebuilt, so any columns
/// a user added beyond this list survive untouched.
const IMAGE_COLUMN_NAMES: &[&str] = &[
    "id",
    "session_id",
    "file_path",
    "location_lat",
    "location_lon",
    "captured_at",
    "gps_altitude_m",
    "width",
    "height",
    "format",
    "make",
    "camera_model",
    "source_path",
    "content_hash",
    "file_size",
    "raw_sibling_id",
    "analysis_ready",
    "blur_score",
    "is_blurry",
];

/// Index used by project-scope duplicate detection, which looks images up by
/// content hash.
const IMAGE_CONTENT_HASH_INDEX: &str = "idx_image_content_hash";

/// One `PRAGMA table_info` row, as far as a rebuild needs it.
struct ColumnInfo {
    name: String,
    type_: String,
    notnull: i64,
    default: Option<String>,
    /// 1-based position in the primary key, `0` when not part of it.
    pk: i64,
}

/// Quotes an identifier so a column name that needs quoting cannot break
/// the generated DDL.
fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// Column DDL for an existing column, preserved across a rebuild so custom
/// columns keep their type, `NOT NULL`, and default.
///
/// The default is always parenthesised: `PRAGMA table_info` reports an
/// expression default (`DEFAULT (datetime('now'))`) without its parentheses,
/// and `DEFAULT (<literal>)` is valid for constants too. CHECK and REFERENCES
/// clauses on custom columns are not carried over; PRIMARY KEY and UNIQUE
/// ones are refused by [`reject_unrebuildable_columns`] rather than dropped.
fn column_ddl(column: &ColumnInfo) -> String {
    let mut ddl = format!("{} {}", quote_ident(&column.name), column.type_);
    if column.notnull != 0 {
        ddl.push_str(" NOT NULL");
    }
    if let Some(default) = &column.default {
        ddl.push_str(&format!(" DEFAULT ({default})"));
    }
    ddl
}

fn table_info(conn: &Connection, table: &str) -> Result<Vec<ColumnInfo>, SessionError> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = stmt
        .query_map([], |row| {
            Ok(ColumnInfo {
                name: row.get(1)?,
                type_: row.get(2)?,
                notnull: row.get(3)?,
                default: row.get(4)?,
                pk: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(columns)
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool, SessionError> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            params![table],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .is_some())
}

/// Indexes a table rebuild would otherwise lose: user-defined ones on the
/// `image` table, as `(name, original_sql)` pairs.
///
/// `sql IS NULL` marks auto-created indexes (constraint enforcement); the new
/// table recreates those itself. Only explicit `CREATE INDEX` statements are
/// captured here so the migration can replay them.
fn user_indexes_on_image(conn: &Connection) -> Result<Vec<(String, String)>, SessionError> {
    let mut stmt = conn.prepare(
        "SELECT name, sql FROM sqlite_master
         WHERE type = 'index' AND tbl_name = 'image' AND sql IS NOT NULL
         ORDER BY name",
    )?;
    let indexes = stmt
        .query_map([], |row| {
            let name: String = row.get(0)?;
            let sql: String = row.get(1)?;
            Ok((name, sql))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(indexes)
}

fn table_is_empty(conn: &Connection, table: &str) -> Result<bool, SessionError> {
    let quoted = quote_ident(table);
    Ok(conn
        .query_row(&format!("SELECT 1 FROM {quoted} LIMIT 1"), [], |row| {
            row.get::<_, i64>(0)
        })
        .optional()?
        .is_none())
}

/// Runs `body` inside one immediate transaction, rolling back on error.
///
/// Uses explicit BEGIN/COMMIT rather than rusqlite's `Transaction` because
/// this module's public functions take `&Connection` and must keep doing
/// so. `IMMEDIATE` takes the write lock up front, so the DDL sequence below
/// it cannot interleave with another writer.
fn with_immediate_transaction<T>(
    conn: &Connection,
    body: impl FnOnce(&Connection) -> Result<T, SessionError>,
) -> Result<T, SessionError> {
    conn.execute("BEGIN IMMEDIATE", [])?;
    match body(conn) {
        Ok(value) => match conn.execute("COMMIT", []) {
            Ok(_) => Ok(value),
            Err(err) => {
                // A failed COMMIT leaves the transaction open on the shared
                // connection, which would break every later BEGIN.
                let _ = conn.execute("ROLLBACK", []);
                Err(err.into())
            }
        },
        Err(err) => {
            let _ = conn.execute("ROLLBACK", []);
            Err(err)
        }
    }
}

/// Fails before a rebuild when a custom column carries a constraint the
/// rebuild cannot reproduce, so the migration errors instead of silently
/// weakening the table.
fn reject_unrebuildable_columns(
    conn: &Connection,
    columns: &[ColumnInfo],
) -> Result<(), SessionError> {
    let custom = |name: &str| !IMAGE_COLUMN_NAMES.contains(&name);
    let refuse = |name: &str, what: &str| {
        SessionError::Db(rusqlite::Error::InvalidColumnName(format!(
            "custom image column '{name}' is part of a {what}; refusing to rebuild the table"
        )))
    };
    if let Some(column) = columns.iter().find(|c| custom(&c.name) && c.pk != 0) {
        return Err(refuse(&column.name, "PRIMARY KEY"));
    }
    let mut indexes = conn.prepare("PRAGMA index_list(image)")?;
    let unique_indexes = indexes
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for (index, unique) in unique_indexes {
        if unique == 0 {
            continue;
        }
        let mut info = conn.prepare(&format!("PRAGMA index_info({})", quote_ident(&index)))?;
        let indexed = info
            .query_map([], |row| row.get::<_, Option<String>>(2))?
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(name) = indexed.into_iter().flatten().find(|name| custom(name)) {
            return Err(refuse(&name, "UNIQUE constraint"));
        }
    }
    Ok(())
}

/// Recovers an `image` table left half-migrated by a killed rebuild.
///
/// An interrupted migration used to leave the only copy of the rows in
/// `image_new`. `CREATE TABLE IF NOT EXISTS image` then created an empty
/// table on the next start and those rows were stranded. This runs before
/// that create, in one transaction, so a kill during recovery rolls back
/// instead of losing either table.
///
/// - no `image_new`: nothing to do
/// - `image` missing: `image_new` becomes `image`
/// - `image` still has rows: `image` is the pre-DROP original, so the
///   leftover copy is dropped
/// - `image` is empty and `image_new` has rows: `image` is the table created
///   after the crash, so the rows in `image_new` win
/// - both empty: the leftover copy is dropped
fn recover_image_table(conn: &Connection) -> Result<(), SessionError> {
    if !table_exists(conn, "image_new")? {
        return Ok(());
    }
    // Keep the old table only when it is the real one: it exists and is
    // either non-empty, or the leftover copy is empty too.
    let keep_old = table_exists(conn, "image")?
        && (table_is_empty(conn, "image_new")? || !table_is_empty(conn, "image")?);
    with_immediate_transaction(conn, |conn| {
        if keep_old {
            conn.execute("DROP TABLE image_new", [])?;
        } else {
            if table_exists(conn, "image")? {
                conn.execute("DROP TABLE image", [])?;
            }
            conn.execute("ALTER TABLE image_new RENAME TO image", [])?;
        }
        Ok(())
    })
}

/// Creates the `session` and `image` tables if they do not already exist,
/// then upgrades the `image` table to the current schema when it predates
/// M1-24 (missing metadata columns or `NOT NULL` location/capture fields).
///
/// The `image` table is defined here (rather than by Module 1's Image
/// Import work) per team decision on 2026-09-26: Adit is building the
/// import function against this schema as the source of truth.
///
/// # Purity
///
/// Impure — DDL against the database.
pub fn init_schema(conn: &Connection) -> Result<(), SessionError> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS project (
            id          TEXT PRIMARY KEY,
            name        TEXT NOT NULL,
            description TEXT,
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS session (
            id          TEXT PRIMARY KEY,
            project_id  TEXT NOT NULL,
            name        TEXT NOT NULL,
            date_start  TEXT,
            date_end    TEXT,
            status      TEXT NOT NULL CHECK (status IN ('active', 'archived')) DEFAULT 'active',
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        )",
        [],
    )?;
    recover_image_table(conn)?;
    conn.execute(
        &format!("CREATE TABLE IF NOT EXISTS image ({IMAGE_TABLE_COLUMNS_DDL})"),
        [],
    )?;
    migrate_image_schema(conn)?;
    // Created after the migration: a rebuild drops the old table's indexes,
    // so this also restores the index on upgraded databases.
    conn.execute(
        &format!("CREATE INDEX IF NOT EXISTS {IMAGE_CONTENT_HASH_INDEX} ON image(content_hash)"),
        [],
    )?;
    Ok(())
}

/// Upgrades a pre-M1-24 `image` table to the current schema, preserving
/// every existing row.
///
/// A pre-M1-24 table is detected by introspection (`PRAGMA table_info`):
/// either a canonical column is absent, or `location_lat`, `location_lon`,
/// or `captured_at` still carries a `NOT NULL` constraint (SQLite cannot
/// drop such a constraint with `ALTER TABLE`, hence the rebuild). The
/// upgrade copies every existing column, so rows keep their data and the new
/// metadata columns are `NULL`. That includes the copy-on-import provenance
/// columns (`source_path`, `content_hash`, `file_size`, `raw_sibling_id`,
/// `analysis_ready`): an upgraded row keeps pointing at its original
/// `file_path` and is not rewritten.
///
/// Idempotent: a table already on the current schema is left untouched,
/// including any columns a user added beyond the canonical set — an extra
/// column is not a migration. The rebuild runs in one transaction, so a
/// crash rolls it back instead of publishing a half-moved table. (A crash
/// before this function ran at all is handled by [`recover_image_table`],
/// which `init_schema` calls first.)
///
/// # Purity
///
/// Impure — DDL and data copy against the database.
pub fn migrate_image_schema(conn: &Connection) -> Result<(), SessionError> {
    if !table_exists(conn, "image")? {
        conn.execute(
            &format!("CREATE TABLE image ({IMAGE_TABLE_COLUMNS_DDL})"),
            [],
        )?;
        return Ok(());
    }

    let columns = table_info(conn, "image")?;
    let names: Vec<&str> = columns.iter().map(|c| c.name.as_str()).collect();
    // Current means every canonical column is present and the three
    // location columns are nullable. Extra columns do not force a rebuild.
    let current = IMAGE_COLUMN_NAMES.iter().all(|name| names.contains(name))
        && columns
            .iter()
            .filter(|c| {
                matches!(
                    c.name.as_str(),
                    "location_lat" | "location_lon" | "captured_at"
                )
            })
            .all(|c| c.notnull == 0);
    if current {
        return Ok(());
    }

    reject_unrebuildable_columns(conn, &columns)?;

    // Copy the canonical columns the old table already has, plus anything
    // else it carries, so no user column is dropped by the rebuild.
    let mut shared: Vec<String> = IMAGE_COLUMN_NAMES
        .iter()
        .filter(|name| names.contains(*name))
        .map(|name| quote_ident(name))
        .collect();
    let mut extra_ddl: Vec<String> = Vec::new();
    for column in &columns {
        if !IMAGE_COLUMN_NAMES.contains(&column.name.as_str()) {
            extra_ddl.push(column_ddl(column));
            shared.push(quote_ident(&column.name));
        }
    }
    if !names.contains(&"id") {
        return Err(SessionError::Db(rusqlite::Error::InvalidColumnName(
            "image table lacks an id column; cannot migrate".to_string(),
        )));
    }
    let shared_list = shared.join(", ");
    let mut ddl = IMAGE_TABLE_COLUMNS_DDL.to_string();
    if !extra_ddl.is_empty() {
        ddl.push_str(",\n            ");
        ddl.push_str(&extra_ddl.join(",\n            "));
    }

    // Indexes with `sql IS NULL` are auto-created (UNIQUE etc.) and come back
    // with the table. Only user-defined ones need explicit re-creation.
    let user_indexes = user_indexes_on_image(conn)?;

    // One transaction: either the whole table moves or nothing does.
    // User indexes are re-created from their original SQL inside the same
    // transaction: `DROP TABLE` drops an index with it, and losing one after
    // a schema upgrade would quietly make duplicate detection or file-path
    // lookups table scans.
    with_immediate_transaction(conn, |conn| {
        conn.execute("DROP TABLE IF EXISTS image_new", [])?;
        conn.execute(&format!("CREATE TABLE image_new ({ddl})"), [])?;
        conn.execute(
            &format!("INSERT INTO image_new ({shared_list}) SELECT {shared_list} FROM image"),
            [],
        )?;
        conn.execute("DROP TABLE image", [])?;
        conn.execute("ALTER TABLE image_new RENAME TO image", [])?;
        for (index_name, index_sql) in user_indexes {
            conn.execute(&index_sql, [])?;
            eprintln!("info: recreated index '{index_name}' after the image table migration");
        }
        Ok(())
    })
}

/// Creates a new, empty session (no images assigned yet).
///
/// # Purity
///
/// Impure — writes a new row.
///
/// # Arguments
///
/// * `conn` — open SQLite connection.
/// * `project_id` — the parent project this session belongs to.
/// * `name` — explicit session name; when `None`, a default name is
///   generated from the current time via [`default_session_name`].
///
/// # Errors
///
/// * [`SessionError::ProjectNotFound`] if `project_id` does not match any existing project.
/// * [`SessionError::Db`] if the insert fails.
pub fn create_session(
    conn: &Connection,
    project_id: Uuid,
    name: Option<String>,
) -> Result<Session, SessionError> {
    get_project(conn, project_id)?;

    let now = Utc::now();
    let session = Session {
        id: Uuid::new_v4(),
        project_id,
        name: name.unwrap_or_else(|| default_session_name(now.naive_utc())),
        date_start: now.naive_utc(),
        date_end: now.naive_utc(),
        status: SessionStatus::Active,
        created_at: now,
        updated_at: now,
    };

    conn.execute(
        "INSERT INTO session (id, project_id, name, date_start, date_end, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            session.id.to_string(),
            session.project_id.to_string(),
            session.name,
            naive_to_db(session.date_start),
            naive_to_db(session.date_end),
            session.status.as_str(),
            session.created_at.to_rfc3339(),
            session.updated_at.to_rfc3339(),
        ],
    )?;

    Ok(session)
}

/// Fetches a single session by id.
///
/// # Purity
///
/// Impure — reads from the database.
///
/// # Errors
///
/// [`SessionError::NotFound`] if no session with that id exists.
/// [`SessionError::Db`] on any other database error.
pub fn get_session(conn: &Connection, session_id: Uuid) -> Result<Session, SessionError> {
    conn.query_row(
        "SELECT id, project_id, name, date_start, date_end, status, created_at, updated_at
         FROM session WHERE id = ?1",
        params![session_id.to_string()],
        row_to_session,
    )
    .optional()?
    .ok_or(SessionError::NotFound { session_id })
}

/// Lists every session belonging to a project.
///
/// # Purity
///
/// Impure.
pub fn get_sessions_by_project(
    conn: &Connection,
    project_id: Uuid,
) -> Result<Vec<Session>, SessionError> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, name, date_start, date_end, status, created_at, updated_at
         FROM session WHERE project_id = ?1 ORDER BY date_start",
    )?;
    let sessions = stmt
        .query_map(params![project_id.to_string()], row_to_session)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(sessions)
}

/// Overwrites a session's auto-generated name with a user-provided one.
///
/// # Purity
///
/// Impure.
pub fn update_session_name(
    conn: &Connection,
    session_id: Uuid,
    new_name: String,
) -> Result<Session, SessionError> {
    let now = Utc::now();
    let rows = conn.execute(
        "UPDATE session SET name = ?1, updated_at = ?2 WHERE id = ?3",
        params![new_name, now.to_rfc3339(), session_id.to_string()],
    )?;
    require_changed(rows, session_id)?;
    get_session(conn, session_id)
}

/// Transitions a session between `active` and `archived`.
///
/// # Purity
///
/// Impure.
pub fn update_session_status(
    conn: &Connection,
    session_id: Uuid,
    status: SessionStatus,
) -> Result<Session, SessionError> {
    let now = Utc::now();
    let rows = conn.execute(
        "UPDATE session SET status = ?1, updated_at = ?2 WHERE id = ?3",
        params![status.as_str(), now.to_rfc3339(), session_id.to_string()],
    )?;
    require_changed(rows, session_id)?;
    get_session(conn, session_id)
}

/// Associates an already-imported image with a session (manual assignment
/// at upload time — see project decision log: images are never dangling).
///
/// # Purity
///
/// Impure. Also triggers [`recalculate_session_date_range`] for the target
/// session, since adding an image can shift `date_start`/`date_end`, and for
/// the previous one when it still exists. Everything runs in one
/// transaction: on error nothing is changed.
///
/// # Open question
///
/// Assumes an `image` table with a `session_id` column already exists.
/// See module-level docs.
pub fn assign_image_to_session(
    conn: &Connection,
    session_id: Uuid,
    image_id: Uuid,
) -> Result<(), SessionError> {
    // One transaction: the move and both recalculations commit together, so
    // a failure never leaves the image moved while the caller sees an error.
    with_immediate_transaction(conn, |conn| {
        let previous_session_id: String = conn
            .query_row(
                "SELECT session_id FROM image WHERE id = ?1",
                params![image_id.to_string()],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(SessionError::ImageNotFound { image_id })?;

        conn.execute(
            "UPDATE image SET session_id = ?1 WHERE id = ?2",
            params![session_id.to_string(), image_id.to_string()],
        )?;

        recalculate_session_date_range(conn, session_id)?;

        if let Some(previous) = Uuid::parse_str(&previous_session_id)
            .ok()
            .filter(|previous| *previous != session_id)
        {
            // The previous session may be gone (the image row outlived a
            // `delete_session`); there is nothing to recalculate then.
            match recalculate_session_date_range(conn, previous) {
                Ok(()) | Err(SessionError::NotFound { .. }) => {}
                Err(err) => return Err(err),
            }
        }
        Ok(())
    })
}

/// Recomputes `date_start`/`date_end` for a session from the `captured_at`
/// of every image currently assigned to it.
///
/// # Purity
///
/// Impure (reads images, writes the session) — but delegates the actual
/// min/max computation to the pure [`compute_date_range`].
///
/// # Errors
///
/// [`SessionError::NotFound`] if the session doesn't exist. A session with
/// zero dated images assigned is reset to the range a fresh session starts
/// with (`date_start = date_end = created_at`), not left at the dates of
/// images it no longer holds (not an error — `image_ids` may be empty on a
/// freshly created session).
pub fn recalculate_session_date_range(
    conn: &Connection,
    session_id: Uuid,
) -> Result<(), SessionError> {
    let created_at: Option<String> = conn
        .query_row(
            "SELECT created_at FROM session WHERE id = ?1",
            params![session_id.to_string()],
            |row| row.get(0),
        )
        .optional()?;
    let Some(created_at) = created_at else {
        return Err(SessionError::NotFound { session_id });
    };

    let images = get_images_by_session(conn, session_id)?;

    // No dated image left: fall back to the range a fresh session starts
    // with (`created_at` for both ends) rather than keeping dates of images
    // that are no longer in the session.
    let (start, end) = match compute_date_range(&images) {
        Some(range) => range,
        None => {
            let created = DateTime::parse_from_rfc3339(&created_at)
                .map_err(|err| {
                    SessionError::Db(conversion_err(6, rusqlite::types::Type::Text, err))
                })?
                .naive_utc();
            (created, created)
        }
    };

    let rows = conn.execute(
        "UPDATE session SET date_start = ?1, date_end = ?2, updated_at = ?3 WHERE id = ?4",
        params![
            naive_to_db(start),
            naive_to_db(end),
            Utc::now().to_rfc3339(),
            session_id.to_string(),
        ],
    )?;
    require_changed(rows, session_id)
}

/// Every stored absolute copy path.
///
/// Used by the orphan sweep: a file under the library root that is not in
/// this list has no row pointing at it.
///
/// # Purity
///
/// Impure — reads the database.
pub(crate) fn all_image_paths(conn: &Connection) -> Result<Vec<String>, SessionError> {
    let mut statement = conn.prepare("SELECT file_path FROM image")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    let mut paths = Vec::new();
    for row in rows {
        // A row that cannot be read is not "no path". Skipping it would make
        // the sweep treat that file as an orphan and delete it.
        paths.push(row?);
    }
    Ok(paths)
}

/// `(project_id, session_id)` pairs for every session currently in SQLite.
///
/// Used by the orphan sweep to restrict its search strictly to sessions this
/// database knows about. A dev database, an older backup, or another database
/// sharing the library root will never touch or trash another database's sessions.
///
/// # Purity
///
/// Impure — reads the database.
pub(crate) fn known_session_scopes(
    conn: &Connection,
) -> Result<std::collections::HashSet<(String, String)>, SessionError> {
    let mut statement = conn.prepare("SELECT project_id, id FROM session")?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut scopes = std::collections::HashSet::new();
    for row in rows {
        scopes.insert(row?);
    }
    Ok(scopes)
}

/// Deletes a session and, by cascade, the image rows that belong to it.
///
/// Bundled SQLite is compiled with `SQLITE_DEFAULT_FOREIGN_KEYS`, so foreign
/// keys are always on and `image.session_id REFERENCES session(id)` is
/// enforced. The child rows are therefore deleted **first**: dropping the
/// session before its images would fail every statement with
/// `FOREIGN KEY constraint failed` and roll back. If the session is missing,
/// the empty child delete is undone by the rollback and
/// [`SessionError::NotFound`] is returned.
///
/// The copied files are removed by
/// [`crate::modules::module_01::import_io::remove_session_files`] (the
/// database layer never touches the disk).
///
/// # Cascade is the decided behaviour
///
/// The team chose cascade over block-delete or unassign, because a session
/// only ever owns the copies it imported. Because that makes the operation
/// destructive, `commands::session::delete_session` requires an explicit
/// `confirm` flag from the UI; this layer assumes the caller confirmed.
///
/// # Purity
///
/// Impure.
pub fn delete_session(conn: &Connection, session_id: Uuid) -> Result<(), SessionError> {
    with_immediate_transaction(conn, |conn| {
        let deleted = conn.execute(
            "DELETE FROM image WHERE session_id = ?1",
            params![session_id.to_string()],
        )?;
        let rows = conn.execute(
            "DELETE FROM session WHERE id = ?1",
            params![session_id.to_string()],
        )?;
        require_changed(rows, session_id)?;
        Ok(deleted)
    })
    .map(|_| ())
}

/// The row that already holds an imported image's content.
///
/// Returned by [`insert_images_and_recalculate`] when a second import of the
/// same bytes (a concurrent run, or a re-import) reaches the insert after the
/// plan was made.
pub(crate) struct ExistingContent {
    /// Index into the batch passed to [`insert_images_and_recalculate`].
    pub index: usize,
    /// Image that already stores those bytes, in the same project.
    pub existing_image_id: Uuid,
}

/// Inserts one import's records and recalculates the session range atomically.
///
/// Duplicate detection normally runs in the pure planner before any file is
/// copied. That check is a snapshot: two imports started at the same time can
/// both pass it and both copy the same bytes. The `IMMEDIATE` transaction
/// serialises writers, so re-checking every content hash **inside** it makes
/// the second writer see the first one's rows.
///
/// # Purity
///
/// Impure — writes the database.
///
/// # Returns
///
/// `Ok(None)` when every row was inserted. `Ok(Some(existing))` when a hash
/// is already stored in the project: nothing from this batch is inserted, and
/// the caller decides what to report (and removes the copies).
pub(crate) fn insert_images_and_recalculate(
    conn: &Connection,
    session_id: Uuid,
    images: &[Image],
) -> Result<Option<ExistingContent>, SessionError> {
    with_immediate_transaction(conn, |conn| {
        let project_id: String = conn.query_row(
            "SELECT project_id FROM session WHERE id = ?1",
            params![session_id.to_string()],
            |row| row.get(0),
        )?;
        for (index, image) in images.iter().enumerate() {
            let Some(hash) = image
                .content_hash
                .as_deref()
                .filter(|hash| !hash.is_empty())
            else {
                continue;
            };
            let existing: Option<String> = conn
                .query_row(
                    "SELECT i.id FROM image i
                     JOIN session s ON s.id = i.session_id
                     WHERE s.project_id = ?1 AND i.content_hash = ?2
                     LIMIT 1",
                    params![project_id, hash],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(raw) = existing {
                let existing_image_id = Uuid::parse_str(&raw).map_err(|err| {
                    SessionError::Storage(format!("stored image id '{raw}' is not a UUID: {err}"))
                })?;
                return Ok(Some(ExistingContent {
                    index,
                    existing_image_id,
                }));
            }
        }
        for image in images {
            insert_image(conn, image)?;
        }
        recalculate_session_date_range(conn, session_id)?;
        Ok(None)
    })
}

// ─────────────────────────────────────────────────────────────────────
// Project CRUD
// ─────────────────────────────────────────────────────────────────────

/// Creates a new mapping project.
///
/// # Purity
///
/// Impure — writes a new row.
pub fn create_project(
    conn: &Connection,
    name: String,
    description: Option<String>,
) -> Result<Project, SessionError> {
    let now = Utc::now();
    let project = Project {
        id: Uuid::new_v4(),
        name,
        description,
        created_at: now,
        updated_at: now,
    };
    conn.execute(
        "INSERT INTO project (id, name, description, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            project.id.to_string(),
            project.name,
            project.description,
            project.created_at.to_rfc3339(),
            project.updated_at.to_rfc3339(),
        ],
    )?;
    Ok(project)
}

/// Fetches a single project by id.
///
/// # Purity
///
/// Impure — reads from the database.
pub fn get_project(conn: &Connection, project_id: Uuid) -> Result<Project, SessionError> {
    conn.query_row(
        "SELECT id, name, description, created_at, updated_at
         FROM project WHERE id = ?1",
        params![project_id.to_string()],
        row_to_project,
    )
    .optional()?
    .ok_or(SessionError::ProjectNotFound { project_id })
}

/// Lists every project, ordered by most recently updated first.
///
/// # Purity
///
/// Impure.
pub fn list_projects(conn: &Connection) -> Result<Vec<Project>, SessionError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, created_at, updated_at
         FROM project ORDER BY updated_at DESC",
    )?;
    let projects = stmt
        .query_map([], row_to_project)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(projects)
}

/// Overwrites a project's name and/or description.
///
/// # Purity
///
/// Impure.
pub fn update_project(
    conn: &Connection,
    project_id: Uuid,
    name: String,
    description: Option<String>,
) -> Result<Project, SessionError> {
    let now = Utc::now();
    let rows = conn.execute(
        "UPDATE project SET name = ?1, description = ?2, updated_at = ?3 WHERE id = ?4",
        params![name, description, now.to_rfc3339(), project_id.to_string()],
    )?;
    if rows == 0 {
        return Err(SessionError::ProjectNotFound { project_id });
    }
    get_project(conn, project_id)
}

/// Deletes a project, its sessions, and all associated images atomically.
///
/// # Errors
///
/// Returns [`SessionError::ProjectNotFound`] if the project does not exist.
///
/// # Purity
///
/// Impure.
pub fn delete_project(conn: &Connection, project_id: Uuid) -> Result<(), SessionError> {
    get_project(conn, project_id)?;

    with_immediate_transaction(conn, |conn| {
        conn.execute(
            "DELETE FROM image WHERE session_id IN (SELECT id FROM session WHERE project_id = ?1)",
            params![project_id.to_string()],
        )?;
        conn.execute(
            "DELETE FROM session WHERE project_id = ?1",
            params![project_id.to_string()],
        )?;
        let rows = conn.execute(
            "DELETE FROM project WHERE id = ?1",
            params![project_id.to_string()],
        )?;
        if rows == 0 {
            return Err(SessionError::ProjectNotFound { project_id });
        }
        Ok(())
    })?;

    Ok(())
}

/// Counts the total number of images across all sessions in a project.
///
/// # Purity
///
/// Impure — reads from the database.
pub fn count_images_by_project(conn: &Connection, project_id: Uuid) -> Result<u32, SessionError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM image i
         INNER JOIN session s ON i.session_id = s.id
         WHERE s.project_id = ?1",
        params![project_id.to_string()],
        |row| row.get(0),
    )?;
    Ok(count as u32)
}

fn row_to_project(row: &Row) -> rusqlite::Result<Project> {
    let id: String = row.get(0)?;
    let name: String = row.get(1)?;
    let description: Option<String> = row.get(2)?;
    let created_at: String = row.get(3)?;
    let updated_at: String = row.get(4)?;

    Ok(Project {
        id: Uuid::parse_str(&id).map_err(|e| conversion_err(0, rusqlite::types::Type::Text, e))?,
        name,
        description,
        created_at: created_at
            .parse()
            .map_err(|e| conversion_err(3, rusqlite::types::Type::Text, e))?,
        updated_at: updated_at
            .parse()
            .map_err(|e| conversion_err(4, rusqlite::types::Type::Text, e))?,
    })
}

/// Builds a persistable [`Image`] record from already-extracted metadata.
///
/// Pure, total (never fails).
///
/// # Arguments
///
/// * `image_id` — id for the new record (generated by the caller).
/// * `session_id` — session the image is imported into.
/// * `file_path` — absolute destination path of the copied file.
/// * `metadata` — output of [`crate::modules::module_01::extract_metadata`].
///   Absent tags become `NULL` columns; the record is always built, even
///   when every field is missing (M1-23 "incomplete metadata").
///
/// The copy-on-import provenance columns (`source_path`, `content_hash`,
/// `file_size`, `raw_sibling_id`, `analysis_ready`) start as `None`: the
/// import I/O layer fills them in once the copy exists and the stem pair is
/// known. `blur_score`/`is_blurry` likewise start as `None`: scoring reads
/// pixels from the copy, so it only happens once the copy exists. A record
/// built here is otherwise complete.
pub fn image_from_metadata(
    image_id: Uuid,
    session_id: Uuid,
    file_path: String,
    metadata: &ImageMetadata,
) -> Image {
    Image {
        id: image_id,
        session_id,
        file_path,
        location_lat: metadata.gps_latitude,
        location_lon: metadata.gps_longitude,
        captured_at: metadata.date_time_original,
        gps_altitude_m: metadata.gps_altitude_m,
        width: metadata.width,
        height: metadata.height,
        format: metadata.format.map(|format| format.as_str().to_string()),
        make: metadata.make.clone(),
        camera_model: metadata.camera_model_name.clone(),
        source_path: None,
        content_hash: None,
        file_size: None,
        raw_sibling_id: None,
        analysis_ready: None,
        blur_score: None,
        is_blurry: None,
    }
}

/// Persists one image record built by [`image_from_metadata`].
///
/// # Purity
///
/// Impure — writes a new row.
///
/// # Errors
///
/// [`SessionError::Db`] if the insert fails (e.g. duplicate `file_path`).
pub fn insert_image(conn: &Connection, image: &Image) -> Result<(), SessionError> {
    conn.execute(
        "INSERT INTO image (id, session_id, file_path, location_lat, location_lon, \
            captured_at, gps_altitude_m, width, height, format, make, camera_model, \
            source_path, content_hash, file_size, raw_sibling_id, analysis_ready, \
            blur_score, is_blurry)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
        params![
            image.id.to_string(),
            image.session_id.to_string(),
            image.file_path,
            image.location_lat,
            image.location_lon,
            image.captured_at.map(naive_to_db),
            image.gps_altitude_m,
            image.width.map(i64::from),
            image.height.map(i64::from),
            image.format,
            image.make,
            image.camera_model,
            image.source_path,
            image.content_hash,
            image.file_size.and_then(|size| i64::try_from(size).ok()),
            image.raw_sibling_id.map(|id| id.to_string()),
            image.analysis_ready.map(i64::from),
            image.blur_score,
            image.is_blurry.map(i64::from),
        ],
    )?;
    Ok(())
}

/// Fetches a single persisted image by id, including its stored metadata.
///
/// # Purity
///
/// Impure — reads from the database.
///
/// # Errors
///
/// [`SessionError::ImageNotFound`] if no image with that id exists.
/// [`SessionError::Db`] on any other database error.
pub fn get_image(conn: &Connection, image_id: Uuid) -> Result<Image, SessionError> {
    conn.query_row(
        &format!("SELECT {IMAGE_COLUMNS} FROM image WHERE id = ?1"),
        params![image_id.to_string()],
        row_to_image,
    )
    .optional()?
    .ok_or(SessionError::ImageNotFound { image_id })
}

/// Lists every persisted image belonging to a session, in file-path order.
///
/// # Purity
///
/// Impure — reads from the database.
pub fn get_images_by_session(
    conn: &Connection,
    session_id: Uuid,
) -> Result<Vec<Image>, SessionError> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {IMAGE_COLUMNS} FROM image WHERE session_id = ?1 ORDER BY file_path"
    ))?;
    let images = stmt
        .query_map(params![session_id.to_string()], row_to_image)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(images)
}

/// Clears `raw_sibling_id` on a JPEG/DNG partner, demotes a DNG that no
/// longer has a JPEG pair, and clears an unscored DNG's copied `is_blurry` flag.
///
/// A lone DNG is not analysis-ready (Module 1 cannot hand raw pixels to a
/// plugin); a surviving JPEG stays as it was. If the DNG copied its blur flag
/// from its partner, unlinking it clears that flag so an unscored DNG does not
/// carry a stale blur warning forever. One statement so all effects share the
/// caller's transaction.
fn unlink_sibling(conn: &Connection, sibling_id: Uuid) -> Result<(), SessionError> {
    conn.execute(
        "UPDATE image SET raw_sibling_id = NULL, \
            analysis_ready = CASE WHEN format = 'dng' THEN 0 ELSE analysis_ready END, \
            is_blurry = CASE WHEN blur_score IS NULL THEN NULL ELSE is_blurry END \
         WHERE id = ?1",
        params![sibling_id.to_string()],
    )?;
    Ok(())
}

/// Deletes one image row, unlinking its raw sibling when it had one.
///
/// The caller removes the file itself; this only touches the database. Runs
/// in one transaction, so a crash cannot leave the sibling pointing at a row
/// that no longer exists.
///
/// # Purity
///
/// Impure — deletes a row.
///
/// # Errors
///
/// [`SessionError::ImageNotFound`] if the id is unknown, [`SessionError::Db`]
/// on any database error.
pub fn delete_image_row(conn: &Connection, image_id: Uuid) -> Result<(), SessionError> {
    let image = get_image(conn, image_id)?;
    with_immediate_transaction(conn, |conn| {
        conn.execute(
            "DELETE FROM image WHERE id = ?1",
            params![image_id.to_string()],
        )?;
        if let Some(sibling_id) = image.raw_sibling_id {
            unlink_sibling(conn, sibling_id)?;
        }
        Ok(())
    })
}

/// Moves one image row to another session and points it at its relocated file.
///
/// The caller has already verified the sessions share a project and has
/// already moved the file to `new_file_path`. The moved image loses its pair
/// link and a DNG partner left behind is demoted; moving one half of a pair
/// never silently keeps a link across sessions. A moved DNG is demoted too:
/// it leaves its JPEG behind, so it is exactly as analysable as any other
/// lone DNG.
///
/// # Purity
///
/// Impure — updates a row.
///
/// # Errors
///
/// [`SessionError::ImageNotFound`] if the id is unknown, [`SessionError::Db`]
/// on any database error.
pub fn move_image_row(
    conn: &Connection,
    image_id: Uuid,
    target_session_id: Uuid,
    new_file_path: &str,
) -> Result<(), SessionError> {
    let image = get_image(conn, image_id)?;
    with_immediate_transaction(conn, |conn| {
        conn.execute(
            "UPDATE image SET session_id = ?2, file_path = ?3, raw_sibling_id = NULL, \
                analysis_ready = CASE WHEN format = 'dng' THEN 0 ELSE analysis_ready END, \
                is_blurry = CASE WHEN blur_score IS NULL THEN NULL ELSE is_blurry END \
             WHERE id = ?1",
            params![
                image_id.to_string(),
                target_session_id.to_string(),
                new_file_path
            ],
        )?;
        if let Some(sibling_id) = image.raw_sibling_id {
            unlink_sibling(conn, sibling_id)?;
        }
        Ok(())
    })
}

/// Re-derives `is_blurry` for every scored image in a session from the
/// session's current score distribution, in one transaction.
///
/// `classify` receives one image's `blur_score` and the leave-one-out
/// baseline stats (mean/stddev/count) of every *other* scored image in the
/// session, and decides whether that image is blurry. Running this once
/// over the whole session, rather than classifying each image incrementally
/// as it is inserted, makes the result depend only on the session's final
/// score set — not on import order or how many separate import runs it took
/// to assemble it.
///
/// Every DNG is left out of the JPEG baseline, scored or not: a DNG's
/// embedded preview is processed differently by camera firmware than a
/// final JPEG, so its Laplacian-variance scores sit on a different scale
/// and would be compared against the wrong yardstick. A DNG with a
/// same-stem JPEG (the same capture as its JPEG, so scoring it as an
/// independent sample would also double-count that capture) mirrors its
/// JPEG's flag instead. A standalone DNG (no same-stem JPEG) has no JPEG
/// to mirror, so it is simply left unflagged — `blur_score` is still
/// computed and stored for it, but `is_blurry` stays `None`.
///
/// Each row's leave-one-out baseline comes from
/// [`super::blur_detection::leave_one_out_stats`] — an O(n) pure helper, not
/// a fresh "every other score" collection per row (which is the O(n²)
/// approach this used to take, held under this function's write lock on
/// every import). See that function's docs for the numerical-stability
/// trade-off its sum-of-squares formula makes.
///
/// Call this after *any* change to a session's set of scored images, not
/// only an import: `is_blurry` is documented as reflecting the session's
/// current score set, and that invariant only holds as long as every
/// mutation re-derives it. Today [`super::import_io::run_import`] is the
/// only caller — a later path that deletes images or moves/copies them
/// between sessions must call this for every session it touches, or those
/// sessions' flags go stale.
///
/// # Purity
///
/// Impure — reads and updates the `image` table.
pub fn reclassify_session_blur(
    conn: &Connection,
    session_id: Uuid,
    classify: impl Fn(f64, &SessionBlurStats) -> bool,
) -> Result<(), SessionError> {
    with_immediate_transaction(conn, |conn| {
        let scored: Vec<(String, f64)> = {
            let mut stmt = conn.prepare(
                "SELECT id, blur_score FROM image
                 WHERE session_id = ?1 AND blur_score IS NOT NULL
                   AND (format IS NULL OR format != 'dng')",
            )?;
            let rows = stmt.query_map(params![session_id.to_string()], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?;
            rows.collect::<Result<_, _>>()?
        };
        let scores: Vec<f64> = scored.iter().map(|(_, score)| *score).collect();
        let stats = super::blur_detection::leave_one_out_stats(&scores);
        let mut update = conn.prepare("UPDATE image SET is_blurry = ?1 WHERE id = ?2")?;
        for ((id, score), stats) in scored.iter().zip(stats.iter()) {
            update.execute(params![i64::from(classify(*score, stats)), id])?;
        }
        // A paired DNG is the same capture as its JPEG: mirror its flag.
        conn.execute(
            "UPDATE image SET is_blurry =
                 (SELECT j.is_blurry FROM image j WHERE j.id = image.raw_sibling_id)
             WHERE session_id = ?1 AND format = 'dng' AND raw_sibling_id IS NOT NULL",
            params![session_id.to_string()],
        )?;
        Ok(())
    })
}

/// Lists the content hashes of every image already stored in a project.
///
/// This is the duplicate scope for an import: an image is a duplicate when
/// its hash matches one of these, even if it lives in a different session.
/// Rows without a hash (migrated from before copy-on-import) are skipped —
/// they cannot prove a duplicate.
///
/// # Purity
///
/// Impure — reads from the database.
///
/// # Returns
///
/// `(image_id, content_hash)` pairs ordered by image id, so the first entry
/// for a repeated hash is stable across calls.
pub fn known_hashes_for_project(
    conn: &Connection,
    project_id: Uuid,
) -> Result<Vec<(String, crate::models::image::ContentHash)>, SessionError> {
    let mut stmt = conn.prepare(
        "SELECT i.id, i.content_hash FROM image i
         JOIN session s ON s.id = i.session_id
         WHERE s.project_id = ?1 AND i.content_hash IS NOT NULL AND i.content_hash <> ''
         ORDER BY i.id",
    )?;
    let hashes = stmt
        .query_map(params![project_id.to_string()], |row| {
            let id: String = row.get(0)?;
            let hash: String = row.get(1)?;
            Ok((id, crate::models::image::ContentHash(hash)))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(hashes)
}

/// Derives a session's `(date_start, date_end)` as the min/max `captured_at`
/// across a set of images.
///
/// Images without a capture time (missing `DateTimeOriginal`) are skipped.
///
/// # Purity
///
/// Pure, total (never fails).
///
/// # Returns
///
/// `None` when `images` is empty or none of them has a capture time.
fn compute_date_range(images: &[Image]) -> Option<(NaiveDateTime, NaiveDateTime)> {
    let mut iter = images.iter().filter_map(|img| img.captured_at);
    let first = iter.next()?;
    let (min, max) = iter.fold((first, first), |(min, max), t| (min.min(t), max.max(t)));
    Some((min, max))
}

/// Generates a default session name from a timestamp, e.g. `"Session -
/// 2026-09-18 07:12"`. User-overwritable per project decision log.
///
/// # Purity
///
/// Pure, total (never fails).
fn default_session_name(at: NaiveDateTime) -> String {
    format!("Session - {}", at.format("%Y-%m-%d %H:%M"))
}

/// Errors with [`SessionError::NotFound`] when an `UPDATE`/`DELETE` touched no row.
fn require_changed(rows: usize, session_id: Uuid) -> Result<(), SessionError> {
    if rows == 0 {
        return Err(SessionError::NotFound { session_id });
    }
    Ok(())
}

fn uuid_col(row: &Row, idx: usize) -> rusqlite::Result<Uuid> {
    let raw: String = row.get(idx)?;
    Uuid::parse_str(&raw).map_err(|e| conversion_err(idx, rusqlite::types::Type::Text, e))
}

fn row_to_session(row: &Row) -> rusqlite::Result<Session> {
    let name: String = row.get(2)?;
    let date_start: String = row.get(3)?;
    let date_end: String = row.get(4)?;
    let status: String = row.get(5)?;
    let created_at: String = row.get(6)?;
    let updated_at: String = row.get(7)?;

    Ok(Session {
        id: uuid_col(row, 0)?,
        project_id: uuid_col(row, 1)?,
        name,
        date_start: naive_from_db(&date_start)
            .map_err(|e| conversion_err(3, rusqlite::types::Type::Text, e))?,
        date_end: naive_from_db(&date_end)
            .map_err(|e| conversion_err(4, rusqlite::types::Type::Text, e))?,
        status: SessionStatus::from_db(&status),
        created_at: created_at
            .parse()
            .map_err(|e| conversion_err(6, rusqlite::types::Type::Text, e))?,
        updated_at: updated_at
            .parse()
            .map_err(|e| conversion_err(7, rusqlite::types::Type::Text, e))?,
    })
}

fn row_to_image(row: &Row) -> rusqlite::Result<Image> {
    let file_path: String = row.get(2)?;
    let location_lat: Option<f64> = row.get(3)?;
    let location_lon: Option<f64> = row.get(4)?;
    let captured_at: Option<String> = row.get(5)?;
    let gps_altitude_m: Option<f64> = row.get(6)?;
    let width: Option<i64> = row.get(7)?;
    let height: Option<i64> = row.get(8)?;
    let format: Option<String> = row.get(9)?;
    let make: Option<String> = row.get(10)?;
    let camera_model: Option<String> = row.get(11)?;
    let source_path: Option<String> = row.get(12)?;
    let content_hash: Option<String> = row.get(13)?;
    let file_size: Option<i64> = row.get(14)?;
    let raw_sibling_id: Option<String> = row.get(15)?;
    let analysis_ready: Option<i64> = row.get(16)?;
    let blur_score: Option<f64> = row.get(17)?;
    let is_blurry: Option<i64> = row.get(18)?;

    let captured_at = captured_at
        .map(|s| naive_from_db(&s).map_err(|e| conversion_err(5, rusqlite::types::Type::Text, e)))
        .transpose()?;
    // Width/height are written by `insert_image` from `u32`, so any stored
    // value outside `u32` range is corrupt data, not a missing value.
    let width = width
        .map(|v| u32::try_from(v).map_err(|e| conversion_err(7, rusqlite::types::Type::Integer, e)))
        .transpose()?;
    let height = height
        .map(|v| u32::try_from(v).map_err(|e| conversion_err(8, rusqlite::types::Type::Integer, e)))
        .transpose()?;
    // Written from `u64`; a negative or non-representable value is corruption.
    let file_size = file_size
        .map(|v| {
            u64::try_from(v).map_err(|e| conversion_err(14, rusqlite::types::Type::Integer, e))
        })
        .transpose()?;
    let raw_sibling_id = raw_sibling_id
        .map(|raw| {
            Uuid::parse_str(&raw).map_err(|e| conversion_err(15, rusqlite::types::Type::Text, e))
        })
        .transpose()?;
    // Stored as 0/1; anything else is not a boolean this code wrote.
    let analysis_ready = analysis_ready
        .map(|v| match v {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(conversion_err(
                16,
                rusqlite::types::Type::Integer,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("analysis_ready must be 0 or 1, found {other}"),
                ),
            )),
        })
        .transpose()?;
    // Stored as 0/1; anything else is not a boolean this code wrote.
    let is_blurry = is_blurry
        .map(|v| match v {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(conversion_err(
                18,
                rusqlite::types::Type::Integer,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("is_blurry must be 0 or 1, found {other}"),
                ),
            )),
        })
        .transpose()?;

    Ok(Image {
        id: uuid_col(row, 0)?,
        session_id: uuid_col(row, 1)?,
        file_path,
        location_lat,
        location_lon,
        captured_at,
        gps_altitude_m,
        width,
        height,
        format,
        make,
        camera_model,
        source_path,
        content_hash,
        file_size,
        raw_sibling_id,
        analysis_ready,
        blur_score,
        is_blurry,
    })
}

/// Canonical text encoding for timezone-naive timestamps in SQLite
/// (`date_start`, `date_end`, `captured_at`).
///
/// `NaiveDateTime::to_string` emits a space separator that `str::parse`
/// cannot read back, so timestamps must go through this helper instead.
fn naive_to_db(dt: NaiveDateTime) -> String {
    dt.format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// Parses a timestamp column written by [`naive_to_db`]. Also accepts the
/// legacy space-separated form (with optional fractional seconds) written
/// before M1-24, so upgraded databases keep reading.
fn naive_from_db(s: &str) -> Result<NaiveDateTime, chrono::ParseError> {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S%.f"))
}

/// Wraps a parse error (UUID or `NaiveDateTime`) as a proper
/// `rusqlite::Error::FromSqlConversionFailure` for a given column, so
/// callers get a real `rusqlite::Error` instead of a made-up variant.
fn conversion_err(
    col: usize,
    ty: rusqlite::types::Type,
    err: impl std::error::Error + Send + Sync + 'static,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(col, ty, Box::new(err))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::image::ImageFormat;

    /// Minimal image row for insert tests; `content_hash` is what matters.
    fn image_row(session_id: Uuid, file_path: &str, content_hash: Option<&str>) -> Image {
        Image {
            id: Uuid::new_v4(),
            session_id,
            file_path: file_path.to_string(),
            location_lat: None,
            location_lon: None,
            captured_at: None,
            gps_altitude_m: None,
            width: None,
            height: None,
            format: Some("jpeg".to_string()),
            make: None,
            camera_model: None,
            source_path: None,
            content_hash: content_hash.map(str::to_string),
            file_size: None,
            raw_sibling_id: None,
            analysis_ready: Some(true),
            blur_score: None,
            is_blurry: None,
        }
    }

    fn img(captured_at: &str) -> Image {
        Image {
            id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            file_path: "test.jpg".to_string(),
            location_lat: Some(0.0),
            location_lon: Some(0.0),
            captured_at: Some(captured_at.parse().unwrap()),
            gps_altitude_m: None,
            width: None,
            height: None,
            format: None,
            make: None,
            camera_model: None,
            source_path: None,
            content_hash: None,
            file_size: None,
            raw_sibling_id: None,
            analysis_ready: None,
            blur_score: None,
            is_blurry: None,
        }
    }

    #[test]
    fn compute_date_range_empty_is_none() {
        assert_eq!(compute_date_range(&[]), None);
    }

    #[test]
    fn compute_date_range_picks_min_and_max() {
        let images = vec![
            img("2026-09-20T10:00:00"),
            img("2026-09-18T07:15:00"),
            img("2026-09-25T09:40:00"),
        ];
        let (start, end) = compute_date_range(&images).unwrap();
        assert_eq!(
            start,
            "2026-09-18T07:15:00".parse::<NaiveDateTime>().unwrap()
        );
        assert_eq!(end, "2026-09-25T09:40:00".parse::<NaiveDateTime>().unwrap());
    }

    #[test]
    fn default_session_name_formats_timestamp() {
        let at: NaiveDateTime = "2026-09-18T07:12:00".parse().unwrap();
        assert_eq!(default_session_name(at), "Session - 2026-09-18 07:12");
    }

    fn full_metadata() -> ImageMetadata {
        ImageMetadata {
            gps_latitude: Some(-7.5),
            gps_longitude: Some(110.0),
            gps_altitude_m: Some(120.0),
            date_time_original: "2026-09-18T07:12:00".parse().ok(),
            width: Some(4000),
            height: Some(3000),
            format: Some(ImageFormat::Jpeg),
            make: Some("DJI".to_string()),
            camera_model_name: Some("Mavic 3".to_string()),
            ..ImageMetadata::default()
        }
    }

    fn memory_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        init_schema(&conn).expect("init schema");
        conn
    }

    fn test_project(conn: &Connection) -> Project {
        create_project(conn, "Test Project".to_string(), None).expect("create test project")
    }

    #[test]
    fn image_from_metadata_maps_all_fields() {
        let image_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();
        let image = image_from_metadata(
            image_id,
            session_id,
            "/photos/a.jpg".to_string(),
            &full_metadata(),
        );
        assert_eq!(image.id, image_id);
        assert_eq!(image.session_id, session_id);
        assert_eq!(image.location_lat, Some(-7.5));
        assert_eq!(image.location_lon, Some(110.0));
        assert_eq!(
            image.captured_at,
            Some("2026-09-18T07:12:00".parse::<NaiveDateTime>().unwrap())
        );
        assert_eq!(image.gps_altitude_m, Some(120.0));
        assert_eq!(image.width, Some(4000));
        assert_eq!(image.height, Some(3000));
        assert_eq!(image.format.as_deref(), Some("jpeg"));
        assert_eq!(image.make.as_deref(), Some("DJI"));
        assert_eq!(image.camera_model.as_deref(), Some("Mavic 3"));
    }

    #[test]
    fn image_from_metadata_empty_is_all_none() {
        let image = image_from_metadata(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "/photos/b.dng".to_string(),
            &ImageMetadata::default(),
        );
        assert_eq!(image.location_lat, None);
        assert_eq!(image.captured_at, None);
        assert_eq!(image.width, None);
        assert_eq!(image.format, None);
    }

    #[test]
    fn insert_and_get_image_round_trip() {
        let conn = memory_db();
        let project = test_project(&conn);
        let session = create_session(&conn, project.id, None).expect("session");
        let image = image_from_metadata(
            Uuid::new_v4(),
            session.id,
            "/photos/a.jpg".to_string(),
            &full_metadata(),
        );
        insert_image(&conn, &image).expect("insert");
        assert_eq!(get_image(&conn, image.id).expect("get"), image);
        let listed = get_images_by_session(&conn, session.id).expect("list");
        assert_eq!(listed, vec![image]);
    }

    /// Builds a session holding one mild outlier (above the hard floor, but
    /// a clear relative outlier against a tight sharp cluster) plus five
    /// sharp images, inserted in the given order, then reclassifies it.
    /// Returns the outlier's `is_blurry` after reclassification.
    fn reclassified_outlier_flag(
        conn: &Connection,
        project_id: Uuid,
        outlier_first: bool,
    ) -> Option<bool> {
        let session = create_session(conn, project_id, None).expect("session");
        let suffix = if outlier_first { "a" } else { "b" };
        let mut outlier = image_row(
            session.id,
            &format!("/photos/{suffix}/outlier.jpg"),
            Some(&format!("outlier-{suffix}")),
        );
        outlier.blur_score = Some(30.0);
        let sharp_scores = [95.0, 100.0, 105.0, 98.0, 102.0];
        let sharp: Vec<Image> = sharp_scores
            .iter()
            .enumerate()
            .map(|(n, score)| {
                let mut image = image_row(
                    session.id,
                    &format!("/photos/{suffix}/sharp{n}.jpg"),
                    Some(&format!("sharp-{suffix}-{n}")),
                );
                image.blur_score = Some(*score);
                image
            })
            .collect();

        if outlier_first {
            insert_image(conn, &outlier).expect("insert outlier");
            for image in &sharp {
                insert_image(conn, image).expect("insert sharp");
            }
        } else {
            for image in &sharp {
                insert_image(conn, image).expect("insert sharp");
            }
            insert_image(conn, &outlier).expect("insert outlier");
        }

        reclassify_session_blur(
            conn,
            session.id,
            crate::modules::module_01::blur_detection::classify_with_stats,
        )
        .expect("reclassify");
        get_image(conn, outlier.id).expect("get outlier").is_blurry
    }

    #[test]
    fn reclassify_session_blur_does_not_depend_on_insert_order() {
        let conn = memory_db();
        let project = test_project(&conn);

        let outlier_imported_first = reclassified_outlier_flag(&conn, project.id, true);
        let outlier_imported_last = reclassified_outlier_flag(&conn, project.id, false);

        assert_eq!(
            outlier_imported_first, outlier_imported_last,
            "the same final score set must classify the same way regardless \
             of the order rows were inserted in"
        );
        assert_eq!(
            outlier_imported_first,
            Some(true),
            "a clear relative outlier against a tight sharp cluster should be flagged blurry"
        );
    }

    #[test]
    fn reclassify_session_blur_mirrors_a_dng_from_its_jpeg() {
        let conn = memory_db();
        let project = test_project(&conn);
        let session = create_session(&conn, project.id, None).expect("session");

        let dng_id = Uuid::new_v4();
        let mut jpeg = image_row(session.id, "/photos/pair.jpg", Some("pair-jpg"));
        jpeg.blur_score = Some(30.0);
        jpeg.raw_sibling_id = Some(dng_id);
        let jpeg_id = jpeg.id;

        let mut dng = image_row(session.id, "/photos/pair.dng", Some("pair-dng"));
        dng.id = dng_id;
        dng.format = Some("dng".to_string());
        dng.raw_sibling_id = Some(jpeg_id);
        // A DNG's preview can score differently from its JPEG sibling — the
        // reclassification pass must still mirror the JPEG's flag rather
        // than treating this as an independent sample.
        dng.blur_score = Some(999.0);

        let sharp_scores = [95.0, 100.0, 105.0, 98.0, 102.0];
        for (n, score) in sharp_scores.iter().enumerate() {
            let mut image = image_row(
                session.id,
                &format!("/photos/sharp{n}.jpg"),
                Some(&format!("sharp{n}")),
            );
            image.blur_score = Some(*score);
            insert_image(&conn, &image).expect("insert sharp");
        }
        insert_image(&conn, &jpeg).expect("insert jpeg");
        insert_image(&conn, &dng).expect("insert dng");

        reclassify_session_blur(
            &conn,
            session.id,
            crate::modules::module_01::blur_detection::classify_with_stats,
        )
        .expect("reclassify");

        let jpeg_flag = get_image(&conn, jpeg_id).expect("get jpeg").is_blurry;
        let dng_flag = get_image(&conn, dng_id).expect("get dng").is_blurry;
        assert_eq!(
            jpeg_flag,
            Some(true),
            "the jpeg is a clear relative outlier"
        );
        assert_eq!(
            dng_flag, jpeg_flag,
            "the paired dng must mirror its jpeg's flag, not its own score"
        );
    }

    #[test]
    fn reclassify_session_blur_leaves_a_standalone_dng_unflagged() {
        let conn = memory_db();
        let project = test_project(&conn);
        let session = create_session(&conn, project.id, None).expect("session");

        // No same-stem JPEG: raw_sibling_id is None.
        let mut dng = image_row(session.id, "/photos/lone.dng", Some("lone-dng"));
        dng.format = Some("dng".to_string());
        dng.blur_score = Some(5.0); // would be a clear outlier if mixed into the jpeg baseline
        let dng_id = dng.id;
        insert_image(&conn, &dng).expect("insert standalone dng");

        let sharp_scores = [95.0, 100.0, 105.0, 98.0, 102.0];
        for (n, score) in sharp_scores.iter().enumerate() {
            let mut image = image_row(
                session.id,
                &format!("/photos/sharp{n}.jpg"),
                Some(&format!("sharp{n}")),
            );
            image.blur_score = Some(*score);
            insert_image(&conn, &image).expect("insert sharp");
        }

        reclassify_session_blur(
            &conn,
            session.id,
            crate::modules::module_01::blur_detection::classify_with_stats,
        )
        .expect("reclassify");

        assert_eq!(
            get_image(&conn, dng_id).expect("get dng").is_blurry,
            None,
            "a standalone dng has no jpeg to mirror, so it stays unflagged"
        );
        let sharp_images = get_images_by_session(&conn, session.id).expect("list");
        for sharp in sharp_images.iter().filter(|img| img.id != dng_id) {
            assert_eq!(
                sharp.is_blurry,
                Some(false),
                "the jpeg baseline must not be skewed by the dng's score: {sharp:?}"
            );
        }
    }

    #[test]
    fn incomplete_image_persists_with_nulls() {
        let conn = memory_db();
        let project = test_project(&conn);
        let session = create_session(&conn, project.id, None).expect("session");
        let image = image_from_metadata(
            Uuid::new_v4(),
            session.id,
            "/photos/b.jpg".to_string(),
            &ImageMetadata::default(),
        );
        insert_image(&conn, &image).expect("insert incomplete");
        let stored = get_image(&conn, image.id).expect("get");
        assert_eq!(stored.captured_at, None);
        assert_eq!(stored.location_lat, None);

        let missing = get_image(&conn, Uuid::new_v4());
        assert!(matches!(missing, Err(SessionError::ImageNotFound { .. })));
    }

    #[test]
    fn migrate_upgrades_pre_m24_table_preserving_rows() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute(
            "CREATE TABLE session (
                id          TEXT PRIMARY KEY,
                project_id  TEXT NOT NULL,
                name        TEXT NOT NULL,
                date_start  TEXT,
                date_end    TEXT,
                status      TEXT NOT NULL DEFAULT 'active',
                created_at  TEXT NOT NULL,
                updated_at  TEXT NOT NULL
            )",
            [],
        )
        .expect("old session table");
        conn.execute(
            "CREATE TABLE image (
                id            TEXT PRIMARY KEY,
                session_id    TEXT NOT NULL REFERENCES session(id),
                file_path     TEXT NOT NULL UNIQUE,
                location_lat  REAL NOT NULL,
                location_lon  REAL NOT NULL,
                captured_at   TEXT NOT NULL
            )",
            [],
        )
        .expect("old image table");
        let session_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO session (id, project_id, name, date_start, date_end, status, created_at, updated_at)
             VALUES (?1, ?2, 'S', '2026-09-18 07:12:00', '2026-09-18 07:12:00', 'active', '2026-09-18T07:12:00+00:00', '2026-09-18T07:12:00+00:00')",
            params![session_id, Uuid::new_v4().to_string()],
        )
        .expect("old session row");
        conn.execute(
            "INSERT INTO image (id, session_id, file_path, location_lat, location_lon, captured_at)
             VALUES (?1, ?2, '/photos/old.jpg', -7.5, 110.0, '2026-09-18 07:12:00')",
            params![Uuid::new_v4().to_string(), session_id],
        )
        .expect("old image row");

        init_schema(&conn).expect("migrate");

        // The rebuilt table gets the duplicate-detection index back.
        let index_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                params![IMAGE_CONTENT_HASH_INDEX],
                |row| row.get(0),
            )
            .expect("index lookup");
        assert_eq!(index_count, 1);

        let old: Image = conn
            .query_row(
                &format!("SELECT {IMAGE_COLUMNS} FROM image WHERE file_path = '/photos/old.jpg'"),
                [],
                row_to_image,
            )
            .expect("old row readable");
        assert_eq!(old.location_lat, Some(-7.5));
        assert_eq!(old.width, None);
        // Copy-on-import provenance is unknown for a migrated row: it keeps
        // pointing at the original path and is not rewritten.
        assert_eq!(old.file_path, "/photos/old.jpg");
        assert_eq!(old.source_path, None);
        assert_eq!(old.content_hash, None);
        assert_eq!(old.file_size, None);
        assert_eq!(old.raw_sibling_id, None);
        assert_eq!(old.analysis_ready, None);
        assert_eq!(old.blur_score, None);
        assert_eq!(old.is_blurry, None);

        // Previously impossible on the old schema: persisting an image
        // without GPS or capture time.
        let session_uuid = Uuid::parse_str(&session_id).expect("session uuid");
        let incomplete = image_from_metadata(
            Uuid::new_v4(),
            session_uuid,
            "/photos/new.jpg".to_string(),
            &ImageMetadata::default(),
        );
        insert_image(&conn, &incomplete).expect("insert after migrate");

        // Second run is a no-op.
        init_schema(&conn).expect("idempotent");
        assert_eq!(
            get_images_by_session(&conn, session_uuid)
                .expect("list")
                .len(),
            2
        );
    }

    #[test]
    fn migrate_adds_blur_columns_to_an_existing_table_preserving_rows() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute(
            "CREATE TABLE session (
                id          TEXT PRIMARY KEY,
                project_id  TEXT NOT NULL,
                name        TEXT NOT NULL,
                date_start  TEXT,
                date_end    TEXT,
                status      TEXT NOT NULL DEFAULT 'active',
                created_at  TEXT NOT NULL,
                updated_at  TEXT NOT NULL
            )",
            [],
        )
        .expect("session table");
        // The full pre-blur-detection (M1-24) image schema: every other
        // canonical column is present, only blur_score/is_blurry are
        // missing — the actual upgrade this PR's users hit, as opposed to
        // the much older pre-M1-24 schema `migrate_upgrades_pre_m24_table_
        // preserving_rows` covers.
        conn.execute(
            "CREATE TABLE image (
                id            TEXT PRIMARY KEY,
                session_id    TEXT NOT NULL REFERENCES session(id),
                file_path     TEXT NOT NULL UNIQUE,
                location_lat  REAL,
                location_lon  REAL,
                captured_at   TEXT,
                gps_altitude_m REAL,
                width         INTEGER,
                height        INTEGER,
                format        TEXT,
                make          TEXT,
                camera_model  TEXT,
                source_path   TEXT,
                content_hash  TEXT,
                file_size     INTEGER,
                raw_sibling_id TEXT,
                analysis_ready INTEGER
            )",
            [],
        )
        .expect("pre-blur image table");
        let session_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO session (id, project_id, name, date_start, date_end, status, created_at, updated_at)
             VALUES (?1, ?2, 'S', '2026-09-18 07:12:00', '2026-09-18 07:12:00', 'active', '2026-09-18T07:12:00+00:00', '2026-09-18T07:12:00+00:00')",
            params![session_id, Uuid::new_v4().to_string()],
        )
        .expect("session row");
        conn.execute(
            "INSERT INTO image (id, session_id, file_path, location_lat, location_lon, captured_at,
                gps_altitude_m, width, height, format, make, camera_model, source_path,
                content_hash, file_size, raw_sibling_id, analysis_ready)
             VALUES (?1, ?2, '/library/old.jpg', -7.5, 110.0, '2026-09-18 07:12:00', 50.0, 4000,
                3000, 'jpeg', 'DJI', 'Mavic 3', '/originals/old.jpg', 'deadbeef', 1234, NULL, 1)",
            params![Uuid::new_v4().to_string(), session_id],
        )
        .expect("pre-blur image row");

        init_schema(&conn).expect("migrate");

        let names: Vec<String> = table_info(&conn, "image")
            .expect("table info")
            .into_iter()
            .map(|c| c.name)
            .collect();
        assert!(names.contains(&"blur_score".to_string()));
        assert!(names.contains(&"is_blurry".to_string()));

        let row: Image = conn
            .query_row(
                &format!("SELECT {IMAGE_COLUMNS} FROM image WHERE file_path = '/library/old.jpg'"),
                [],
                row_to_image,
            )
            .expect("row readable after migration");
        // Every pre-existing column survives the rebuild untouched...
        assert_eq!(row.content_hash.as_deref(), Some("deadbeef"));
        assert_eq!(row.width, Some(4000));
        assert_eq!(row.analysis_ready, Some(true));
        // ...and the two new columns are NULL, not an error or a default
        // that would misrepresent an unscored row as deliberately sharp.
        assert_eq!(row.blur_score, None);
        assert_eq!(row.is_blurry, None);
    }

    /// Creates the `session` table plus one row, so `image` rows have a
    /// session to reference. Returns the session id.
    fn seeded_session(conn: &Connection) -> String {
        conn.execute(
            "CREATE TABLE session (
                id          TEXT PRIMARY KEY,
                project_id  TEXT NOT NULL,
                name        TEXT NOT NULL,
                date_start  TEXT,
                date_end    TEXT,
                status      TEXT NOT NULL DEFAULT 'active',
                created_at  TEXT NOT NULL,
                updated_at  TEXT NOT NULL
            )",
            [],
        )
        .expect("session table");
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO session (id, project_id, name, date_start, date_end, status, created_at, updated_at)
             VALUES (?1, ?2, 'S', '2026-09-18 07:12:00', '2026-09-18 07:12:00', 'active', '2026-09-18T07:12:00+00:00', '2026-09-18T07:12:00+00:00')",
            params![id, Uuid::new_v4().to_string()],
        )
        .expect("session row");
        id
    }

    #[test]
    fn rebuild_keeps_an_expression_default_on_a_custom_column() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        let session_id = seeded_session(&conn);
        // NOT NULL location columns force a rebuild.
        conn.execute(
            "CREATE TABLE image (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                file_path TEXT NOT NULL UNIQUE,
                location_lat REAL NOT NULL,
                location_lon REAL NOT NULL,
                captured_at TEXT NOT NULL,
                gps_altitude_m REAL,
                width INTEGER,
                height INTEGER,
                format TEXT,
                make TEXT,
                camera_model TEXT,
                imported_at TEXT DEFAULT (datetime('now')),
                score INTEGER NOT NULL DEFAULT 7
            )",
            [],
        )
        .expect("legacy table");
        conn.execute(
            "INSERT INTO image (id, session_id, file_path, location_lat, location_lon, captured_at)
             VALUES ('img-1', ?1, '/a.jpg', 1.0, 2.0, '2026-09-18 07:12:00')",
            params![session_id],
        )
        .expect("row");

        init_schema(&conn).expect("rebuild must not fail on the defaults");

        conn.execute(
            "INSERT INTO image (id, session_id, file_path) VALUES ('img-2', ?1, '/b.jpg')",
            params![session_id],
        )
        .expect("insert relying on defaults");
        let (imported_at, score): (Option<String>, i64) = conn
            .query_row(
                "SELECT imported_at, score FROM image WHERE id = 'img-2'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("defaults applied");
        assert!(imported_at.is_some(), "expression default must survive");
        assert_eq!(score, 7);
    }

    #[test]
    fn rebuild_refuses_a_custom_unique_column_without_losing_data() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        let session_id = seeded_session(&conn);
        conn.execute(
            "CREATE TABLE image (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                file_path TEXT NOT NULL UNIQUE,
                location_lat REAL NOT NULL,
                location_lon REAL NOT NULL,
                captured_at TEXT NOT NULL,
                gps_altitude_m REAL,
                width INTEGER,
                height INTEGER,
                format TEXT,
                make TEXT,
                camera_model TEXT,
                serial TEXT UNIQUE
            )",
            [],
        )
        .expect("legacy table");
        conn.execute(
            "INSERT INTO image (id, session_id, file_path, location_lat, location_lon, captured_at, serial)
             VALUES ('img-1', ?1, '/a.jpg', 1.0, 2.0, '2026-09-18 07:12:00', 'SN1')",
            params![session_id],
        )
        .expect("row");

        let err = init_schema(&conn).expect_err("unique custom column cannot be rebuilt");
        assert!(err.to_string().contains("serial"), "{err}");
        let serial: String = conn
            .query_row("SELECT serial FROM image", [], |row| row.get(0))
            .expect("original table untouched");
        assert_eq!(serial, "SN1");
    }

    #[test]
    fn init_schema_keeps_custom_columns_and_data() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        let session_id = seeded_session(&conn);
        conn.execute(
            "CREATE TABLE image (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                file_path TEXT NOT NULL UNIQUE,
                location_lat REAL,
                location_lon REAL,
                captured_at TEXT,
                gps_altitude_m REAL,
                width INTEGER,
                height INTEGER,
                format TEXT,
                make TEXT,
                camera_model TEXT,
                user_notes TEXT
            )",
            [],
        )
        .expect("current image table with a custom column");
        conn.execute(
            "INSERT INTO image (id, session_id, file_path, user_notes)
             VALUES ('img-1', ?1, '/photos/a.jpg', 'IMPORTANT NOTE')",
            params![session_id],
        )
        .expect("row with a note");

        // A custom column is not a migration: the table is rebuilt nowhere.
        init_schema(&conn).expect("init");
        let columns = table_info(&conn, "image").expect("introspect");
        assert!(columns.iter().any(|c| c.name == "user_notes"));
        let note: String = conn
            .query_row("SELECT user_notes FROM image", [], |row| row.get(0))
            .expect("note survives");
        assert_eq!(note, "IMPORTANT NOTE");

        init_schema(&conn).expect("second init");
        let note: String = conn
            .query_row("SELECT user_notes FROM image", [], |row| row.get(0))
            .expect("note still there");
        assert_eq!(note, "IMPORTANT NOTE");
    }

    #[test]
    fn init_schema_recovers_rows_stranded_in_image_new() {
        // State left behind by a killed rebuild: the only copy of the row is
        // in image_new, and no image table exists.
        let conn = Connection::open_in_memory().expect("in-memory db");
        let session_id = seeded_session(&conn);
        conn.execute(
            &format!("CREATE TABLE image_new ({IMAGE_TABLE_COLUMNS_DDL})"),
            [],
        )
        .expect("image_new");
        conn.execute(
            "INSERT INTO image_new (id, session_id, file_path)
             VALUES ('img-1', ?1, '/photos/old.jpg')",
            params![session_id],
        )
        .expect("row only in image_new");
        assert!(!table_exists(&conn, "image").expect("no image table"));

        init_schema(&conn).expect("recover");
        assert!(!table_exists(&conn, "image_new").expect("leftover dropped"));
        let path: String = conn
            .query_row("SELECT file_path FROM image", [], |row| row.get(0))
            .expect("row recovered");
        assert_eq!(path, "/photos/old.jpg");

        init_schema(&conn).expect("second init");
        let path: String = conn
            .query_row("SELECT file_path FROM image", [], |row| row.get(0))
            .expect("row still there");
        assert_eq!(path, "/photos/old.jpg");
    }

    #[test]
    fn empty_image_table_with_rows_in_image_new_keeps_the_rows() {
        // The bug as shipped: init_schema created an empty image table, so
        // the row in image_new was stranded and lost on the next start.
        let conn = Connection::open_in_memory().expect("in-memory db");
        let session_id = seeded_session(&conn);
        conn.execute(
            &format!("CREATE TABLE image ({IMAGE_TABLE_COLUMNS_DDL})"),
            [],
        )
        .expect("empty image table");
        conn.execute(
            &format!("CREATE TABLE image_new ({IMAGE_TABLE_COLUMNS_DDL})"),
            [],
        )
        .expect("image_new");
        conn.execute(
            "INSERT INTO image_new (id, session_id, file_path)
             VALUES ('img-1', ?1, '/photos/old.jpg')",
            params![session_id],
        )
        .expect("row only in image_new");

        init_schema(&conn).expect("recover");
        assert!(!table_exists(&conn, "image_new").expect("leftover dropped"));
        let path: String = conn
            .query_row("SELECT file_path FROM image", [], |row| row.get(0))
            .expect("row recovered");
        assert_eq!(path, "/photos/old.jpg");
    }

    #[test]
    fn image_with_rows_beats_leftover_image_new() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        let session_id = seeded_session(&conn);
        conn.execute(
            &format!("CREATE TABLE image ({IMAGE_TABLE_COLUMNS_DDL})"),
            [],
        )
        .expect("image");
        conn.execute(
            "INSERT INTO image (id, session_id, file_path) VALUES ('img-1', ?1, '/photos/keep.jpg')",
            params![session_id],
        )
        .expect("real row");
        conn.execute(
            &format!("CREATE TABLE image_new ({IMAGE_TABLE_COLUMNS_DDL})"),
            [],
        )
        .expect("image_new");
        conn.execute(
            "INSERT INTO image_new (id, session_id, file_path) VALUES ('img-2', ?1, '/photos/copy.jpg')",
            params![session_id],
        )
        .expect("leftover row");

        init_schema(&conn).expect("init");
        assert!(!table_exists(&conn, "image_new").expect("leftover dropped"));
        let paths: Vec<String> = conn
            .prepare("SELECT file_path FROM image ORDER BY file_path")
            .and_then(|mut stmt| {
                stmt.query_map([], |row| row.get::<_, String>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .expect("list");
        assert_eq!(paths, vec!["/photos/keep.jpg".to_string()]);
    }

    #[test]
    fn migrate_copies_custom_columns_when_rebuilding() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        let session_id = seeded_session(&conn);
        conn.execute(
            "CREATE TABLE image (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                file_path TEXT NOT NULL UNIQUE,
                location_lat REAL NOT NULL,
                location_lon REAL NOT NULL,
                captured_at TEXT NOT NULL,
                user_notes TEXT
            )",
            [],
        )
        .expect("legacy image table with a custom column");
        conn.execute(
            "INSERT INTO image (id, session_id, file_path, location_lat, location_lon, captured_at, user_notes)
             VALUES ('img-1', ?1, '/photos/old.jpg', -7.5, 110.0, '2026-09-18 07:12:00', 'IMPORTANT NOTE')",
            params![session_id],
        )
        .expect("legacy row with a note");

        init_schema(&conn).expect("migrate");

        let columns = table_info(&conn, "image").expect("introspect");
        assert!(columns.iter().any(|c| c.name == "user_notes"));
        let (lat, note): (Option<f64>, String) = conn
            .query_row("SELECT location_lat, user_notes FROM image", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .expect("row readable");
        assert_eq!(lat, Some(-7.5));
        assert_eq!(note, "IMPORTANT NOTE");
    }

    /// The `image` table exactly as the last commit on the main branch
    /// defines it: twelve columns, no metadata/hash pair columns. The team
    /// pinned this as the migration fixture everyone's checkout hits.
    const MAIN_BRANCH_IMAGE_DDL: &str = "CREATE TABLE image (
        id TEXT PRIMARY KEY,
        session_id TEXT NOT NULL,
        file_path TEXT NOT NULL UNIQUE,
        location_lat REAL,
        location_lon REAL,
        captured_at TEXT,
        gps_altitude_m REAL,
        width INTEGER,
        height INTEGER,
        format TEXT,
        make TEXT,
        camera_model TEXT
    )";

    #[test]
    fn migrates_main_schema_preserving_rows_and_values() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        let session_id = seeded_session(&conn);
        conn.execute(MAIN_BRANCH_IMAGE_DDL, [])
            .expect("main-branch table");
        conn.execute(
            "INSERT INTO image (id, session_id, file_path, location_lat, location_lon,
                                captured_at, gps_altitude_m, width, height, format, make, camera_model)
             VALUES ('img-1', ?1, '/photos/flight.jpg', -7.52, 110.11,
                     '2026-09-18 07:12:00', 512.0, 4032, 3000, 'jpeg', 'DJI', 'FC7304')",
            params![session_id],
        )
        .expect("row of real image data");

        init_schema(&conn).expect("migrate");

        let columns = table_info(&conn, "image").expect("introspect");
        for canonical in IMAGE_COLUMN_NAMES {
            assert!(
                columns.iter().any(|c| c.name == *canonical),
                "column {canonical} present after migration"
            );
        }
        let row: (String, Option<f64>, Option<f64>, Option<f64>, i64, i64) = conn
            .query_row(
                "SELECT file_path, location_lat, location_lon, gps_altitude_m, width, height
                 FROM image WHERE id = 'img-1'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .expect("row readable");
        assert_eq!(row.0, "/photos/flight.jpg");
        assert_eq!(row.1, Some(-7.52));
        assert_eq!(row.2, Some(110.11));
        assert_eq!(row.3, Some(512.0));
        assert_eq!(row.4, 4032);
        assert_eq!(row.5, 3000);
        // Migrated rows have no hash yet; the backfill is a separate follow-up.
        assert!(
            known_hashes_for_project(&conn, project_of(&conn).parse().expect("uuid"))
                .expect("hash query")
                .is_empty()
        );
    }

    fn project_of(conn: &Connection) -> String {
        conn.query_row("SELECT project_id FROM session", [], |row| row.get(0))
            .expect("one seeded session")
    }

    #[test]
    fn migration_keeps_user_indexes() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        let session_id = seeded_session(&conn);
        conn.execute(MAIN_BRANCH_IMAGE_DDL, []).expect("table");
        conn.execute(
            "INSERT INTO image (id, session_id, file_path) VALUES ('img-1', ?1, '/a.jpg')",
            params![session_id],
        )
        .expect("row");
        conn.execute(
            "CREATE INDEX idx_image_session_fast ON image (session_id, captured_at)",
            [],
        )
        .expect("user index");

        init_schema(&conn).expect("migrate");

        let index_sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'index' AND name = 'idx_image_session_fast'",
                [],
                |row| row.get(0),
            )
            .expect("user index recreated");
        assert!(
            index_sql.to_lowercase().contains("session_id"),
            "original definition preserved: {index_sql}"
        );
        // And it is still a usable index over the rebuilt table.
        conn.query_row(
            "EXPLAIN QUERY PLAN SELECT file_path FROM image WHERE session_id = 'x'",
            [],
            |row| row.get::<_, String>(3),
        )
        .map(|plan| {
            assert!(
                plan.to_lowercase().contains("idx_image_session_fast"),
                "the planner uses the recreated index: {plan}"
            )
        })
        .expect("query plan");
    }

    #[test]
    fn date_range_skips_undated_images() {
        let dated = img("2026-09-18T07:15:00");
        let mut undated = img("2026-09-20T10:00:00");
        undated.captured_at = None;
        // Only undated images: no range to derive.
        assert_eq!(compute_date_range(&[undated.clone()]), None);
        // Mixed: undated images are ignored.
        let (start, end) = compute_date_range(&[undated, dated]).unwrap();
        assert_eq!(start, end);
        assert_eq!(
            start,
            "2026-09-18T07:15:00".parse::<NaiveDateTime>().unwrap()
        );
    }

    #[test]
    fn naive_datetime_db_round_trip_with_legacy_fallback() {
        let dt: NaiveDateTime = "2026-09-18T07:12:00".parse().unwrap();
        let stored = naive_to_db(dt);
        assert_eq!(stored, "2026-09-18T07:12:00");
        assert_eq!(naive_from_db(&stored).expect("round trip"), dt);
        // Legacy space-separated rows (pre-M1-24) keep reading.
        assert_eq!(naive_from_db("2026-09-18 07:12:00").expect("legacy"), dt);
    }

    #[test]
    fn session_create_get_round_trip() {
        let conn = memory_db();
        let project = test_project(&conn);
        let created = create_session(&conn, project.id, Some("Trip".to_string())).expect("create");
        let fetched = get_session(&conn, created.id).expect("get");
        assert_eq!(fetched.id, created.id);
        // Stored at second precision; the in-memory value keeps nanoseconds.
        assert_eq!(
            naive_to_db(fetched.date_start),
            naive_to_db(created.date_start)
        );
    }

    #[test]
    fn recalculate_derives_range_from_stored_images() {
        let conn = memory_db();
        let project = test_project(&conn);
        let session = create_session(&conn, project.id, None).expect("session");
        let dated = |path: &str, dt: &str| {
            let mut meta = full_metadata();
            meta.date_time_original = dt.parse().ok();
            image_from_metadata(Uuid::new_v4(), session.id, path.to_string(), &meta)
        };
        insert_image(&conn, &dated("/photos/a.jpg", "2026-09-20T10:00:00")).expect("a");
        insert_image(&conn, &dated("/photos/b.jpg", "2026-09-18T07:15:00")).expect("b");
        let meta = ImageMetadata {
            width: Some(10),
            height: Some(10),
            ..ImageMetadata::default()
        };
        insert_image(
            &conn,
            &image_from_metadata(
                Uuid::new_v4(),
                session.id,
                "/photos/c.jpg".to_string(),
                &meta,
            ),
        )
        .expect("c undated");
        recalculate_session_date_range(&conn, session.id).expect("recalc");
        let updated = get_session(&conn, session.id).expect("get");
        assert_eq!(
            updated.date_start,
            "2026-09-18T07:15:00".parse::<NaiveDateTime>().unwrap()
        );
        assert_eq!(
            updated.date_end,
            "2026-09-20T10:00:00".parse::<NaiveDateTime>().unwrap()
        );
    }

    #[test]
    fn foreign_keys_are_enforced() {
        // Bundled libsqlite3-sys is built with SQLITE_DEFAULT_FOREIGN_KEYS,
        // so a fresh connection must be able to reject orphaned image rows.
        // Guarding this here stops anyone from "fixing" a cascade failure by
        // turning foreign keys off instead of deleting children first.
        let conn = memory_db();
        let enforced: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("pragma foreign_keys");
        assert_eq!(enforced, 1);

        let live = create_session(&conn, test_project(&conn).id, None).expect("live");
        let image = image_from_metadata(
            Uuid::new_v4(),
            live.id,
            "/photos/a.jpg".to_string(),
            &full_metadata(),
        );
        insert_image(&conn, &image).expect("image");
        assert!(
            insert_image(
                &conn,
                &image_row(Uuid::new_v4(), "/photos/orphan.jpg", None)
            )
            .is_err(),
            "an image row must reference a session the database knows"
        );
    }

    #[test]
    fn delete_session_removes_a_session_with_images() {
        let conn = memory_db();
        let session = create_session(&conn, test_project(&conn).id, None).expect("session");
        for path in ["/lib/a.jpg", "/lib/b.jpg"] {
            insert_image(&conn, &image_row(session.id, path, None)).expect("image");
        }
        delete_session(&conn, session.id).expect("cascade delete");
        assert!(matches!(
            get_session(&conn, session.id),
            Err(SessionError::NotFound { .. })
        ));
        assert_eq!(
            get_images_by_session(&conn, session.id).expect("images"),
            Vec::<Image>::new()
        );
    }

    #[test]
    fn delete_session_of_missing_id_keeps_other_rows() {
        let conn = memory_db();
        let kept = create_session(&conn, test_project(&conn).id, None).expect("kept session");
        for path in ["/lib/keep1.jpg", "/lib/keep2.jpg"] {
            insert_image(&conn, &image_row(kept.id, path, None)).expect("image");
        }

        assert!(matches!(
            delete_session(&conn, Uuid::new_v4()),
            Err(SessionError::NotFound { .. })
        ));
        assert!(get_session(&conn, kept.id).is_ok());
        assert_eq!(
            get_images_by_session(&conn, kept.id).expect("images").len(),
            2
        );
    }

    #[test]
    fn assign_tolerates_a_deleted_previous_session() {
        let conn = memory_db();
        // Foreign keys are off on connections that never enabled them; that
        // is how an image can outlive its session.
        conn.execute("PRAGMA foreign_keys = OFF", [])
            .expect("pragma");
        let project = test_project(&conn);
        let live = create_session(&conn, project.id, None).expect("live");
        // The image still points at a session that no longer exists.
        let image = image_from_metadata(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "/photos/a.jpg".to_string(),
            &full_metadata(),
        );
        insert_image(&conn, &image).expect("image");

        assign_image_to_session(&conn, live.id, image.id).expect("assign succeeds");
        let stored = get_image(&conn, image.id).expect("image");
        assert_eq!(stored.session_id, live.id);
    }

    #[test]
    fn assign_rolls_back_when_the_target_session_is_missing() {
        let conn = memory_db();
        // With foreign keys off the UPDATE itself succeeds, so only the
        // transaction can undo it when the recalculation then fails.
        conn.execute("PRAGMA foreign_keys = OFF", [])
            .expect("pragma");
        let project = test_project(&conn);
        let original = create_session(&conn, project.id, None).expect("original");
        let image = image_from_metadata(
            Uuid::new_v4(),
            original.id,
            "/photos/a.jpg".to_string(),
            &full_metadata(),
        );
        insert_image(&conn, &image).expect("image");

        let err = assign_image_to_session(&conn, Uuid::new_v4(), image.id).unwrap_err();
        assert!(matches!(err, SessionError::NotFound { .. }));
        let stored = get_image(&conn, image.id).expect("image");
        assert_eq!(stored.session_id, original.id, "the move must roll back");
    }

    #[test]
    fn emptied_session_resets_its_range_to_created_at() {
        let conn = memory_db();
        let project = test_project(&conn);
        let source = create_session(&conn, project.id, None).expect("source");
        let target = create_session(&conn, project.id, None).expect("target");
        let mut meta = full_metadata();
        meta.date_time_original = "2026-09-18T07:12:00".parse().ok();
        let image = image_from_metadata(Uuid::new_v4(), source.id, "/a.jpg".to_string(), &meta);
        insert_image(&conn, &image).expect("image");
        recalculate_session_date_range(&conn, source.id).expect("recalc");
        let with_image = get_session(&conn, source.id).expect("get");
        assert_eq!(
            with_image.date_start,
            "2026-09-18T07:12:00".parse::<NaiveDateTime>().unwrap()
        );

        assign_image_to_session(&conn, target.id, image.id).expect("move");

        let emptied = get_session(&conn, source.id).expect("get");
        assert_eq!(
            naive_to_db(emptied.date_start),
            naive_to_db(source.date_start)
        );
        assert_eq!(
            naive_to_db(emptied.date_end),
            naive_to_db(source.date_start)
        );
        let moved = get_session(&conn, target.id).expect("get");
        assert_eq!(
            moved.date_start,
            "2026-09-18T07:12:00".parse::<NaiveDateTime>().unwrap()
        );
    }

    #[test]
    fn project_crud_round_trip() {
        let conn = memory_db();
        let created = create_project(&conn, "Survey Alpha".to_string(), Some("Desc".to_string()))
            .expect("create");
        assert_eq!(created.name, "Survey Alpha");
        assert_eq!(created.description.as_deref(), Some("Desc"));

        let fetched = get_project(&conn, created.id).expect("get");
        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.name, "Survey Alpha");
        assert_eq!(fetched.description.as_deref(), Some("Desc"));

        let listed = list_projects(&conn).expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);

        let updated = update_project(
            &conn,
            created.id,
            "Survey Beta".to_string(),
            Some("New Desc".to_string()),
        )
        .expect("update");
        assert_eq!(updated.name, "Survey Beta");
        assert_eq!(updated.description.as_deref(), Some("New Desc"));

        delete_project(&conn, created.id).expect("delete");
        let after_delete = get_project(&conn, created.id);
        assert!(matches!(
            after_delete,
            Err(SessionError::ProjectNotFound { .. })
        ));
    }

    #[test]
    fn project_not_found_on_get_update_delete() {
        let conn = memory_db();
        let missing = Uuid::new_v4();

        assert!(matches!(
            get_project(&conn, missing),
            Err(SessionError::ProjectNotFound { project_id }) if project_id == missing
        ));
        assert!(matches!(
            update_project(&conn, missing, "Name".to_string(), None),
            Err(SessionError::ProjectNotFound { project_id }) if project_id == missing
        ));
        assert!(matches!(
            delete_project(&conn, missing),
            Err(SessionError::ProjectNotFound { project_id }) if project_id == missing
        ));
    }

    #[test]
    fn create_session_rejects_missing_project() {
        let conn = memory_db();
        let missing = Uuid::new_v4();
        let err = create_session(&conn, missing, None).unwrap_err();
        assert!(matches!(
            err,
            SessionError::ProjectNotFound { project_id } if project_id == missing
        ));
    }

    #[test]
    fn delete_project_atomically_cascades_sessions_and_images() {
        let conn = memory_db();
        let project = test_project(&conn);
        let s1 = create_session(&conn, project.id, None).expect("s1");
        let s2 = create_session(&conn, project.id, None).expect("s2");

        let img1 = image_from_metadata(
            Uuid::new_v4(),
            s1.id,
            "/a.jpg".to_string(),
            &full_metadata(),
        );
        let img2 = image_from_metadata(
            Uuid::new_v4(),
            s2.id,
            "/b.jpg".to_string(),
            &full_metadata(),
        );
        insert_image(&conn, &img1).expect("img1");
        insert_image(&conn, &img2).expect("img2");

        delete_project(&conn, project.id).expect("delete project");

        assert!(matches!(
            get_project(&conn, project.id),
            Err(SessionError::ProjectNotFound { .. })
        ));
        assert!(matches!(
            get_session(&conn, s1.id),
            Err(SessionError::NotFound { .. })
        ));
        assert!(matches!(
            get_session(&conn, s2.id),
            Err(SessionError::NotFound { .. })
        ));
        assert!(matches!(
            get_image(&conn, img1.id),
            Err(SessionError::ImageNotFound { .. })
        ));
        assert!(matches!(
            get_image(&conn, img2.id),
            Err(SessionError::ImageNotFound { .. })
        ));
    }

    #[test]
    fn count_images_by_project_aggregates_across_sessions() {
        let conn = memory_db();
        let p1 = test_project(&conn);
        let p2 = test_project(&conn);

        let s1 = create_session(&conn, p1.id, None).expect("s1");
        let s2 = create_session(&conn, p1.id, None).expect("s2");
        let s3 = create_session(&conn, p2.id, None).expect("s3");

        let img1 = image_from_metadata(
            Uuid::new_v4(),
            s1.id,
            "/1.jpg".to_string(),
            &full_metadata(),
        );
        let img2 = image_from_metadata(
            Uuid::new_v4(),
            s1.id,
            "/2.jpg".to_string(),
            &full_metadata(),
        );
        let img3 = image_from_metadata(
            Uuid::new_v4(),
            s2.id,
            "/3.jpg".to_string(),
            &full_metadata(),
        );
        let img4 = image_from_metadata(
            Uuid::new_v4(),
            s3.id,
            "/4.jpg".to_string(),
            &full_metadata(),
        );

        insert_image(&conn, &img1).expect("img1");
        insert_image(&conn, &img2).expect("img2");
        insert_image(&conn, &img3).expect("img3");
        insert_image(&conn, &img4).expect("img4");

        assert_eq!(count_images_by_project(&conn, p1.id).expect("count p1"), 3);
        assert_eq!(count_images_by_project(&conn, p2.id).expect("count p2"), 1);
    }

    #[test]
    fn moving_a_row_unlinks_the_pair_and_demotes_a_left_behind_dng() {
        let conn = memory_db();
        let project = test_project(&conn);
        let source = create_session(&conn, project.id, None).expect("source session");
        let target = create_session(&conn, project.id, None).expect("target session");

        let jpeg_id = Uuid::new_v4();
        let dng_id = Uuid::new_v4();
        let mut jpeg = image_from_metadata(
            jpeg_id,
            source.id,
            "/lib/a.jpg".to_string(),
            &full_metadata(),
        );
        jpeg.format = Some("jpeg".to_string());
        jpeg.raw_sibling_id = Some(dng_id);
        jpeg.analysis_ready = Some(true);
        let mut dng = image_from_metadata(
            dng_id,
            source.id,
            "/lib/a.dng".to_string(),
            &full_metadata(),
        );
        dng.format = Some("dng".to_string());
        dng.raw_sibling_id = Some(jpeg_id);
        dng.analysis_ready = Some(true);
        insert_image(&conn, &jpeg).expect("jpeg row");
        insert_image(&conn, &dng).expect("dng row");

        move_image_row(&conn, jpeg_id, target.id, "/lib/target/a.jpg").expect("move");

        let moved = get_image(&conn, jpeg_id).expect("moved row");
        assert_eq!(moved.session_id, target.id);
        assert_eq!(moved.file_path, "/lib/target/a.jpg");
        assert_eq!(moved.raw_sibling_id, None, "a moved image leaves its pair");
        let partner = get_image(&conn, dng_id).expect("partner row");
        assert_eq!(partner.raw_sibling_id, None);
        assert_eq!(
            partner.analysis_ready,
            Some(false),
            "a dng whose jpeg moved away is not analysable"
        );
    }

    #[test]
    fn moving_a_dng_demotes_it_and_keeps_the_jpeg_analysable() {
        let conn = memory_db();
        let project = test_project(&conn);
        let source = create_session(&conn, project.id, None).expect("source session");
        let target = create_session(&conn, project.id, None).expect("target session");

        let jpeg_id = Uuid::new_v4();
        let dng_id = Uuid::new_v4();
        let mut jpeg = image_from_metadata(
            jpeg_id,
            source.id,
            "/lib/a.jpg".to_string(),
            &full_metadata(),
        );
        jpeg.format = Some("jpeg".to_string());
        jpeg.raw_sibling_id = Some(dng_id);
        jpeg.analysis_ready = Some(true);
        let mut dng = image_from_metadata(
            dng_id,
            source.id,
            "/lib/a.dng".to_string(),
            &full_metadata(),
        );
        dng.format = Some("dng".to_string());
        dng.raw_sibling_id = Some(jpeg_id);
        dng.analysis_ready = Some(true);
        insert_image(&conn, &jpeg).expect("jpeg row");
        insert_image(&conn, &dng).expect("dng row");

        move_image_row(&conn, dng_id, target.id, "/lib/target/a.dng").expect("move");

        let moved = get_image(&conn, dng_id).expect("moved row");
        assert_eq!(moved.session_id, target.id);
        assert_eq!(moved.raw_sibling_id, None, "a moved image leaves its pair");
        assert_eq!(
            moved.analysis_ready,
            Some(false),
            "a moved dng is a lone dng and not analysable"
        );
        let partner = get_image(&conn, jpeg_id).expect("partner row");
        assert_eq!(partner.raw_sibling_id, None);
        assert_eq!(
            partner.analysis_ready,
            Some(true),
            "the jpeg left behind stays analysable"
        );
    }

    #[test]
    fn deleting_a_row_keeps_a_surviving_jpeg_analysable() {
        let conn = memory_db();
        let project = test_project(&conn);
        let session = create_session(&conn, project.id, None).expect("session");

        let jpeg_id = Uuid::new_v4();
        let dng_id = Uuid::new_v4();
        let mut jpeg = image_from_metadata(
            jpeg_id,
            session.id,
            "/lib/a.jpg".to_string(),
            &full_metadata(),
        );
        jpeg.format = Some("jpeg".to_string());
        jpeg.raw_sibling_id = Some(dng_id);
        jpeg.analysis_ready = Some(true);
        let mut dng = image_from_metadata(
            dng_id,
            session.id,
            "/lib/a.dng".to_string(),
            &full_metadata(),
        );
        dng.format = Some("dng".to_string());
        dng.raw_sibling_id = Some(jpeg_id);
        dng.analysis_ready = Some(true);
        insert_image(&conn, &jpeg).expect("jpeg row");
        insert_image(&conn, &dng).expect("dng row");

        delete_image_row(&conn, dng_id).expect("delete dng");

        assert!(matches!(
            get_image(&conn, dng_id),
            Err(SessionError::ImageNotFound { .. })
        ));
        let jpeg = get_image(&conn, jpeg_id).expect("jpeg survives");
        assert_eq!(jpeg.raw_sibling_id, None);
        assert_eq!(
            jpeg.analysis_ready,
            Some(true),
            "a lone jpeg stays analysable"
        );
    }

    #[test]
    fn deleting_a_jpeg_clears_is_blurry_on_an_unscored_dng() {
        let conn = memory_db();
        let project = test_project(&conn);
        let session = create_session(&conn, project.id, None).expect("session");

        let jpeg_id = Uuid::new_v4();
        let dng_id = Uuid::new_v4();
        let mut jpeg = image_from_metadata(
            jpeg_id,
            session.id,
            "/lib/a.jpg".to_string(),
            &full_metadata(),
        );
        jpeg.format = Some("jpeg".to_string());
        jpeg.raw_sibling_id = Some(dng_id);
        jpeg.analysis_ready = Some(true);
        jpeg.blur_score = Some(15.0);
        jpeg.is_blurry = Some(true);

        let mut dng = image_from_metadata(
            dng_id,
            session.id,
            "/lib/a.dng".to_string(),
            &full_metadata(),
        );
        dng.format = Some("dng".to_string());
        dng.raw_sibling_id = Some(jpeg_id);
        dng.analysis_ready = Some(true);
        dng.blur_score = None;
        dng.is_blurry = Some(true);

        insert_image(&conn, &jpeg).expect("jpeg row");
        insert_image(&conn, &dng).expect("dng row");

        delete_image_row(&conn, jpeg_id).expect("delete jpeg");

        let dng = get_image(&conn, dng_id).expect("dng survives");
        assert_eq!(dng.raw_sibling_id, None);
        assert_eq!(dng.analysis_ready, Some(false));
        assert_eq!(
            dng.is_blurry, None,
            "an unscored lone dng must have is_blurry cleared"
        );
    }
}
