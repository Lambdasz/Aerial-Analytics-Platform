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

use chrono::{NaiveDateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row, TransactionBehavior};
use uuid::Uuid;

use super::error::SessionError;
use crate::models::image::{ImageFormat, ImageMetadata};
use crate::models::{Image, Session, SessionStatus};

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
            camera_model  TEXT";

/// Columns of the `image` table in canonical order, for `SELECT`s and for
/// the data-preserving copy in [`migrate_image_schema`].
const IMAGE_COLUMNS: &str = "id, session_id, file_path, location_lat, location_lon, \
    captured_at, gps_altitude_m, width, height, format, make, camera_model";

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
];

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
        Ok(value) => {
            conn.execute("COMMIT", [])?;
            Ok(value)
        }
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
/// metadata columns are `NULL`.
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

    // One transaction: either the whole table moves or nothing does.
    with_immediate_transaction(conn, |conn| {
        conn.execute("DROP TABLE IF EXISTS image_new", [])?;
        conn.execute(&format!("CREATE TABLE image_new ({ddl})"), [])?;
        conn.execute(
            &format!("INSERT INTO image_new ({shared_list}) SELECT {shared_list} FROM image"),
            [],
        )?;
        conn.execute("DROP TABLE image", [])?;
        conn.execute("ALTER TABLE image_new RENAME TO image", [])?;
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
/// [`SessionError::Db`] if the insert fails.
pub fn create_session(
    conn: &Connection,
    project_id: Uuid,
    name: Option<String>,
) -> Result<Session, SessionError> {
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
            status_to_str(session.status),
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
    if rows == 0 {
        return Err(SessionError::NotFound { session_id });
    }
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
        params![
            status_to_str(status),
            now.to_rfc3339(),
            session_id.to_string()
        ],
    )?;
    if rows == 0 {
        return Err(SessionError::NotFound { session_id });
    }
    get_session(conn, session_id)
}

/// Associates an already-imported image with a session (manual assignment
/// at upload time — see project decision log: images are never dangling).
///
/// # Purity
///
/// Impure. Also triggers [`recalculate_session_date_range`] for the target
/// session, since adding an image can shift `date_start`/`date_end`.
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
    let previous_session_id: Option<String> = conn
        .query_row(
            "SELECT session_id FROM image WHERE id = ?1",
            params![image_id.to_string()],
            |row| row.get(0),
        )
        .optional()?;
    let Some(previous_session_id) = previous_session_id else {
        return Err(SessionError::ImageNotFound { image_id });
    };

    conn.execute(
        "UPDATE image SET session_id = ?1 WHERE id = ?2",
        params![session_id.to_string(), image_id.to_string()],
    )?;

    recalculate_session_date_range(conn, session_id)?;

    if previous_session_id != session_id.to_string() {
        if let Ok(previous_session_id) = Uuid::parse_str(&previous_session_id) {
            recalculate_session_date_range(conn, previous_session_id)?;
        }
    }
    Ok(())
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
/// zero images assigned is left with its previous date range unchanged
/// (not an error — see project decision log: `image_ids` may be empty on a
/// freshly created session).
pub fn recalculate_session_date_range(
    conn: &Connection,
    session_id: Uuid,
) -> Result<(), SessionError> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM session WHERE id = ?1",
            params![session_id.to_string()],
            |row| row.get(0),
        )
        .optional()?;
    if exists.is_none() {
        return Err(SessionError::NotFound { session_id });
    }

    let mut stmt = conn.prepare(&format!(
        "SELECT {IMAGE_COLUMNS} FROM image WHERE session_id = ?1"
    ))?;
    let images = stmt
        .query_map(params![session_id.to_string()], row_to_image)?
        .collect::<Result<Vec<_>, _>>()?;

    let Some((start, end)) = compute_date_range(&images) else {
        return Ok(());
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
    if rows == 0 {
        return Err(SessionError::NotFound { session_id });
    }
    Ok(())
}

/// Deletes a session.
///
/// # Status
///
/// **Open item** — behaviour when the session still has images assigned to
/// it has not yet been decided by the team (block deletion vs. cascade
/// vs. unassign). Currently deletes unconditionally; revisit once decided.
///
/// # Purity
///
/// Impure.
pub fn delete_session(conn: &Connection, session_id: Uuid) -> Result<(), SessionError> {
    let rows = conn.execute(
        "DELETE FROM session WHERE id = ?1",
        params![session_id.to_string()],
    )?;
    if rows == 0 {
        return Err(SessionError::NotFound { session_id });
    }
    Ok(())
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
        format: metadata.format.map(|format| match format {
            ImageFormat::Jpeg => "jpeg".to_string(),
            ImageFormat::Dng => "dng".to_string(),
        }),
        make: metadata.make.clone(),
        camera_model: metadata.camera_model_name.clone(),
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
            captured_at, gps_altitude_m, width, height, format, make, camera_model)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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

fn status_to_str(status: SessionStatus) -> &'static str {
    match status {
        SessionStatus::Active => "active",
        SessionStatus::Archived => "archived",
    }
}

fn row_to_session(row: &Row) -> rusqlite::Result<Session> {
    let id: String = row.get(0)?;
    let project_id: String = row.get(1)?;
    let name: String = row.get(2)?;
    let date_start: String = row.get(3)?;
    let date_end: String = row.get(4)?;
    let status: String = row.get(5)?;
    let created_at: String = row.get(6)?;
    let updated_at: String = row.get(7)?;

    Ok(Session {
        id: Uuid::parse_str(&id).map_err(|e| conversion_err(0, rusqlite::types::Type::Text, e))?,
        project_id: Uuid::parse_str(&project_id)
            .map_err(|e| conversion_err(1, rusqlite::types::Type::Text, e))?,
        name,
        date_start: naive_from_db(&date_start)
            .map_err(|e| conversion_err(3, rusqlite::types::Type::Text, e))?,
        date_end: naive_from_db(&date_end)
            .map_err(|e| conversion_err(4, rusqlite::types::Type::Text, e))?,
        status: match status.as_str() {
            "archived" => SessionStatus::Archived,
            _ => SessionStatus::Active,
        },
        created_at: created_at
            .parse()
            .map_err(|e| conversion_err(6, rusqlite::types::Type::Text, e))?,
        updated_at: updated_at
            .parse()
            .map_err(|e| conversion_err(7, rusqlite::types::Type::Text, e))?,
    })
}

fn row_to_image(row: &Row) -> rusqlite::Result<Image> {
    let id: String = row.get(0)?;
    let session_id: String = row.get(1)?;
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

    Ok(Image {
        id: Uuid::parse_str(&id).map_err(|e| conversion_err(0, rusqlite::types::Type::Text, e))?,
        session_id: Uuid::parse_str(&session_id)
            .map_err(|e| conversion_err(1, rusqlite::types::Type::Text, e))?,
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
        let session = create_session(&conn, Uuid::new_v4(), None).expect("session");
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

    #[test]
    fn incomplete_image_persists_with_nulls() {
        let conn = memory_db();
        let session = create_session(&conn, Uuid::new_v4(), None).expect("session");
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

        let old: Image = conn
            .query_row(
                &format!("SELECT {IMAGE_COLUMNS} FROM image WHERE file_path = '/photos/old.jpg'"),
                [],
                row_to_image,
            )
            .expect("old row readable");
        assert_eq!(old.location_lat, Some(-7.5));
        assert_eq!(old.width, None);

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
        let created =
            create_session(&conn, Uuid::new_v4(), Some("Trip".to_string())).expect("create");
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
        let session = create_session(&conn, Uuid::new_v4(), None).expect("session");
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
}
