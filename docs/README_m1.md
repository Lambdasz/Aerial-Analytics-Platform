# Module 1 — Image Metadata Extraction & RGB Import

Source: `src-tauri/src/modules/module_01/`

This module is Module 1's back-end for two responsibilities:

- **Image import planning** (`image_importer.rs`) — classifies and validates
  a batch of picked files, rejects duplicates, and produces an `ImportReport`,
  without touching the disk or the database.
- **Copy-on-import I/O** (`import_io.rs`) — stages the picked files (one read
  each, hashed while copying), renames the accepted ones into the app-owned
  library under the first free name, reads metadata from the copy, and
  records the row(s). This is what a Tauri command calls.
- **Metadata extraction** (`metadata_extractor.rs`) — reads EXIF and XMP DJI
  tags from a single JPEG/DNG file into an `ImageMetadata` value.
- **DNG preview** (`dng_preview.rs`) — returns the JPEG a DNG already stores,
  so a browser can display a DNG without a raw decoder.

It sits below the organisational hierarchy defined in
`src-tauri/src/models/session.rs`:

```text
Project
└── Session
    └── Image  (the persisted record this module's output eventually becomes)
```

## Status

Implemented. Import planning and copy-on-import I/O, the extraction pipeline
(format sniff, JPEG and DNG/TIFF parsers, XMP DJI scan, merge helpers), the
DNG preview reader and the schema migration are all live and covered by unit
tests. Two commands expose this module to the UI:
`commands::import::import_images` and `commands::import::get_analysis_input`.

Status now (what an importing UI can rely on):

- yes — picked images are copied into the app-owned library before they are
  recorded; the original folder may be moved or deleted afterwards;
- yes — `.jpg`/`.jpeg` and `.dng` are imported; each stored row's `file_path`
  is the absolute path of the copy;
- yes — a DNG with a same-stem JPEG is stored as a second row linked through
  `raw_sibling_id`, and the plugin input is the JPEG;
- no — a DNG with no JPEG is imported but marked
  `analysis_ready = 0` and listed in `not_analysis_ready`; Module 1 does not
  decode raw pixels;
- yes — a destination name that is already taken is imported under the
  first free ` (n)` suffix (`DJI_0003 (2).JPG`); nothing is ever overwritten;
- no — duplicates are reported, never resolved automatically (a second
  import of the same bytes is skipped and flagged).

DNG dimensions come from the largest image not flagged reduced-resolution
among IFD0 and its SubIFDs (tag 330), so a preview in IFD0 is not mistaken for
the sensor size.

## Visibility

Only [`MetadataError`](#metadataerror) is `pub` — usable from outside this
crate. Everything else described below is `pub(crate)`: it can be imported
and called from any other module in this crate (`commands/`,
`plugin_manager/`, etc.), but is not part of the crate's external API.

```rust
use crate::modules::module_01::{
    describe_completeness, extract_metadata, import_images, read_dng, to_plugin_metadata,
    MetadataCompleteness, MetadataError,
};
```

## API reference

### `import_images`

```rust
pub(crate) fn import_images(
    request: &ImportRequest,
    known: &[(String, ContentHash)],
) -> Result<ImportReport, ImportError> {
    // ...
}
```

Entry point for RGB image import. Composes per-file format classification and
duplicate detection into one `ImportReport`. Does not copy files or touch the
disk or the database — `request` is already populated with file
headers/hashes/ids by I/O before this runs, and the caller applies the
returned `ImportReport` afterwards.

Destination names are deliberately **not** decided here. The I/O layer picks
the first free ` (n)` name when it persists each staged copy, so two files
with the same basename both import (`DJI_0001.JPG`, `DJI_0001 (2).JPG`), and a
race between two imports is resolved by retrying the next suffix rather than
failing. A JPEG/DNG stem pair always receives one shared suffix, so the
sibling link survives the rename.

- **Errors**: `ImportError::NoImagesFound` if `request` has no candidates.
  Per-candidate failures (bad format, magic mismatch, invalid file name, missing
  content hash) are
  reported in the result's `rejected` list, not as an `Err`.
- **The caller validates the session.** This function has no session list, so
  it never returns `ImportError::SessionNotFound`. The command that owns the
  database checks an `Existing` id via `get_session` before calling in.
- Intra-batch duplicates are flagged against the first file that was accepted
  for import. Duplicates are never copied.

### `run_import`

```rust
pub(crate) fn run_import(
    db: &Mutex<Connection>,
    library_root: &Path,
    session_id: Uuid,
    sources: &[String],
    from_folder: bool,
    on_progress: impl FnMut(usize, usize, &str),
) -> Result<ImportReport, ImportFailure> {
    // ...
}
```

The copy-on-import pipeline behind the `import_images` command. It scans
`sources` (files, or non-recursive folders when `from_folder` is set), reads
each source **once** — hashing it while writing a temporary copy in the
session folder — runs the pure planner, renames every approved candidate to a
free destination name, reads metadata from the copy, and inserts the row(s)
in one transaction with the session date-range recalculation. `on_progress`
receives `(done, total, destination_path)` after each file.

Storage layout, below the root resolved once at startup:

```text
<library_root>/projects/<project_id>/sessions/<session_id>/images/<file_name>
```

`library_root` is `<app-data-dir>/library`, or `AAP_LIBRARY_ROOT` when set.
The database stores the canonical absolute path of the copy.

Per-file behaviour:

- picked paths are checked **before** any read: anything that is not a
  regular file, or is not `.jpg`/`.jpeg`/`.dng`, is rejected unopened, and a
  non-matching header is rejected after the first bytes, so a video or a FIFO
  never stalls or exhausts the import;
- each source is read once, while a temporary copy is hashed; the plan never
  re-reads it;
- an existing destination is never overwritten (`DestinationExists`); the
  persist step retries the next suffix, and a JPEG/DNG pair moves together to
  keep their stems equal;
- if the insert fails, the copied file(s) are deleted, so no orphan remains,
  and the failure is reported as `RecordFailed` (a database problem) rather
  than `CopyFailed`;
- every content hash is re-checked inside the insert transaction, so two
  imports running at once cannot store the same bytes twice;
- one rejected file never aborts the rest of the batch.

Its helpers `remove_session_files` and `cleanup_orphaned_copies` handle the
disk side of deletion: `delete_session` (which the command refuses unless
`confirm=true`) removes the session's copied files after deleting its rows,
and a background sweep started after launch removes files older than an hour
that no row references (the crash window between staging and commit). The
sweep runs off the startup path so a large library does not delay launch.

### `get_analysis_input` / `analysis_input`

```rust
pub(crate) fn analysis_input(
    conn: &Connection,
    image_id: Uuid,
) -> Result<PluginImageMetadata, ImportFailure> {
    // ...
}
```

Projects a stored image onto the plugin pre-flight shape without reading the
file again. A JPEG is its own input. A DNG returns its `raw_sibling_id` row
(always a JPEG by construction); a DNG without a sibling is rejected with
`ImportError::NotAnalysisReady`. Rows migrated from before copy-on-import have
`analysis_ready = NULL`; readiness for them is decided from `format` (JPEG
ready, DNG not), so an old JPEG stays analysable.

### Schema (image table)

Copy-on-import adds five nullable columns; existing rows are copied unchanged
and keep their `file_path`. `CREATE INDEX idx_image_content_hash` supports
project-scope duplicate detection.

| Column           | Meaning                                              |
| ---------------- | ---------------------------------------------------- |
| `source_path`    | Path the image was imported from (provenance).       |
| `content_hash`   | SHA-256 of the file, lowercase hex.                  |
| `file_size`      | Size of the copy in bytes.                           |
| `raw_sibling_id` | The other half of a JPG/DNG stem pair (symmetric).   |
| `analysis_ready` | `1` analysable, `0` DNG with no JPEG, `NULL` legacy. |

### `extract_metadata`

```rust
pub(crate) fn extract_metadata(path: &Path) -> Result<ImageMetadata, MetadataError> {
    // ...
}
```

Reads a single JPEG/DNG file from disk and extracts its full metadata. It
touches the filesystem (so does `read_dng`, which delegates to it); it's
defined as the composition:

```text
read file (JPEG: up to the SOS header; DNG: whole file)
  -> detect_format(header, extension)
  -> read_dimensions(bytes, format)
  -> read_exif(bytes) + from_exif(&exif)
  -> xmp_packet(bytes, format) -> parse_xmp_dji(packet)
  -> merge_tags(exif, xmp, dimensions, format)
```

A tag missing from the file is not an error — the corresponding
`ImageMetadata` field is simply `None`. An EXIF block that is absent or too
broken to salvage also composes into default tags instead of failing the
extraction (`read_exif` runs kamadak-exif with `continue_on_error`, so a
partially broken block keeps what could be parsed).

- **Errors**: `MetadataError::Io` (file unreadable), `UnsupportedFormat` /
  `MagicMismatch` (not a supported image), `MalformedExif` (an unparseable
  dimension header).

### `read_dng`

```rust
pub(crate) fn read_dng(path: &Path) -> Result<ImageMetadata, MetadataError> {
    // ...
}
```

Reads one DNG from disk and returns its metadata. This is how another module
reads a DNG: the TIFF/IFD parsing stays inside this module, so no caller
needs to know about byte order, IFD chains, or SubIFD offsets.

The file is **sniffed before it is parsed**. A file that is not a DNG is
rejected as the wrong format, not as a broken one: a JPEG handed to this
function reports `MetadataError::UnsupportedFormat`, not the
`MalformedExif` that parsing it would otherwise produce. The one exception
is a `.dng` whose header is not TIFF at all, which reports
`MetadataError::MagicMismatch` — the extension promised a DNG and the bytes
did not deliver one. Absent tags are `None`, as everywhere else in this
module.

A plain TIFF named `.dng` is accepted, because the format sniff cannot tell
a DNG from a TIFF and this module treats them the same. No DNG version check
is performed. A DNG is read in full, because its IFD offsets can point
anywhere in the file.

- **Errors**: `MetadataError::UnsupportedFormat` (not a DNG), `MagicMismatch`
  (a `.dng` without TIFF magic), `Io` (unreadable). A present EXIF segment
  that is broken does not fail the read: whatever kamadak recovers composes
  into the tags and the rest stay `None`.

### `read_dng_preview`

```rust
pub(crate) fn read_dng_preview(path: &Path) -> Result<Vec<u8>, MetadataError> {
    // ...
}
```

Returns the JPEG a DNG already stores. A browser cannot put a `.dng` in an
`<img>` tag, and the JPEG embedded in the file is the same preview the file
manager shows. This returns those bytes **without decoding raw pixels** — there
is no Bayer demosaic, no color matrix, and no full-resolution decode.

The search order inside the file is:

1. the DNG `PreviewImageStart`/`PreviewImageLength` pair (tags 50981/50982) on
   IFD0,
2. `JPEGInterchangeFormat`/`Length` (513/514) on IFD1,
3. the same pair on IFD0,
4. the reduced-resolution SubIFDs (tag 330).

Every candidate that is not a standalone JPEG — a raw strip or a tiled
payload — is ignored. The largest usable candidate wins, capped at 8 MiB. The
file itself is capped at 80 MiB, because a DNG is read whole: its IFD offsets
can point anywhere in the file.

A DNG that stores only sensor data has no preview. That is reported as
`MetadataError::PreviewUnavailable` (code `PREVIEW_UNAVAILABLE`), which is not
a failure to read the file — the caller should say the preview is unavailable
rather than report a broken image.

- **Errors**: `MetadataError::UnsupportedFormat` (not a DNG), `MagicMismatch`
  (a `.dng` without TIFF magic), `Io` (unreadable or over the size cap),
  `PreviewUnavailable` (no embedded JPEG).

## Note on the XMP DJI scan

`parse_xmp_dji` reads only the XMP packet (`<x:xmpmeta>` / `<?xpacket>`), so a
JPEG comment or other stray bytes containing `drone-dji:` are never mistaken
for telemetry. Both serializations are accepted inside the packet:

- attribute form — `drone-dji:Name="value"` or `'value'`
- element form — `<drone-dji:Name>value</drone-dji:Name>`

An occurrence that is neither (stray text, `xmlns:drone-dji=…`, an unquoted or
truncated value) is skipped, and a non-numeric value (`"n/a"`, empty) leaves
that one field `None`. Flight telemetry is optional: a quirk there never
discards the EXIF, GPS, or dimensions already read, so XMP never fails an
extraction. `MetadataError::MalformedXmp` remains in the error type but is no
longer produced by this path.

### `describe_completeness`

```rust
pub(crate) fn describe_completeness(metadata: &ImageMetadata) -> MetadataCompleteness {
    // ...
}
```

Pure, total. Walks a previously-extracted `ImageMetadata` and buckets its
fields into present vs. missing. **Contract: Module 1.3 → Module 1.5** (feeds
Image Quality Checking, which flags images with missing metadata).

### `missing_required_fields` / `REQUIRED_METADATA_FIELDS`

```rust
pub(crate) const REQUIRED_METADATA_FIELDS: &[&str] = &[
    "gps_latitude", "gps_longitude", "gps_altitude_m",
    "date_time_original", "width", "height",
];

pub(crate) fn missing_required_fields(metadata: &ImageMetadata) -> Vec<String> {
    // ...
}
```

Pure, total. Returns the subset of `REQUIRED_METADATA_FIELDS` absent from
the metadata (empty = fully described for import). Covers the M1-9
acceptance criteria (GPS, date, altitude, resolution); XMP flight telemetry
is excluded since non-DJI drones never carry it.

### `flag_incomplete_metadata`

```rust
pub(crate) fn flag_incomplete_metadata(
    imported: &[ImportCandidate],
    metadata: &HashMap<String, ImageMetadata>,
) -> Vec<IncompleteMetadataFlag> {
    // ...
}
```

Pure, total. Second phase of the import pipeline, after `import_images`:

```text
import_images(request, known)
  -> I/O copies files + extracts metadata per imported id (impure)
  -> flag_incomplete_metadata(imported_candidates, metadata)
```

One `IncompleteMetadataFlag { candidate_id, source_path, missing }` per
imported candidate lacking a required field, in import order. Candidates with
no metadata entry were never extracted (the I/O layer skipped them), so
completeness cannot be judged and nothing is emitted. When extraction was
attempted but failed (corrupt/unreadable), the I/O layer records
`ImageMetadata::default()` instead, which flags every required field as
missing. Flagged images stay imported; the flag never moves an id into
`rejected` or `duplicates`.

### `to_plugin_metadata`

```rust
pub(crate) fn to_plugin_metadata(
    metadata: &ImageMetadata,
    path: &Path,
) -> crate::plugin_manager::payload::ImageMetadata {
    // ...
}
```

Pure, total. Projects `ImageMetadata` into the minimal shape
`plugin_manager::payload::preflight_check` needs before running a plugin
(sets `has_gps` when both latitude and longitude are present; renders
`format` as a lowercase string, `"jpeg"`/`"dng"`). **Contract: Module 1.3 →
Module 2** (plugin pre-flight validation).

### `MetadataCompleteness`

```rust
pub(crate) struct MetadataCompleteness {
    pub present: Vec<String>,
    pub missing: Vec<String>,
}
```

Return type of `describe_completeness`: field names that were populated vs.
left `None`.

### `MetadataError`

```rust
pub enum MetadataError {
    Io(std::io::Error),
    UnsupportedFormat { path: String, extension: String },
    MagicMismatch { path: String },
    MalformedExif(String),
    MalformedXmp(String),
    NotImplemented { path: String },
    PreviewUnavailable { path: String },
}
```

The one fully-public type in this module — the domain error for every
metadata-extraction operation. Each variant has a `code()` method
(`"IO_ERROR"`, `"UNSUPPORTED_FORMAT"`, `"MAGIC_MISMATCH"`,
`"MALFORMED_EXIF"`, `"MALFORMED_XMP"`, `"NOT_IMPLEMENTED"`,
`"PREVIEW_UNAVAILABLE"`) for programmatic matching on the frontend, and
converts via `From<MetadataError> for CommandError` so it can be returned
directly from a `#[tauri::command]` and reach the frontend over IPC.

## Note on `ImportError` copy-on-import variants

Four variants belong to the copy step:

- `CopyFailed { path, reason }` (code `COPY_FAILED`) — an I/O failure while
  scanning, copying, or reading the copy's metadata. The file leaves no row
  and no half-written copy.
- `RecordFailed { path, reason }` (`RECORD_FAILED`) — the copy succeeded but
  its row could not be written (or the destination path is already owned by
  another row). The copy is deleted; the UI should treat this as a database
  problem, not a bad image.
- `DestinationExists { path }` (`DESTINATION_EXISTS`) — a file appeared at the
  destination after planning. The import never overwrites.
- `NotAnalysisReady { image_id }` (`IMAGE_NOT_READY`) — `get_analysis_input`
  was asked for a DNG with no usable JPEG sibling (none was imported, or the
  sibling row is gone).

`ImportError` implements `Display` (human-readable) and `code()`
(programmatic), and converts to `CommandError` through
`ImportFailure` (see `import_io::ImportFailure`), which also carries
`SessionError` for database/storage failures.

## Note on `ImportError::UnsupportedFormat`

`image_importer::classify_file` (an internal helper behind `import_images`)
also raises `ImportError::UnsupportedFormat` as a defensive fallback if the
format detector fails in a way its own contract says can't happen (an
`Io`/`MalformedExif`/`MalformedXmp`/`PreviewUnavailable` `MetadataError` from
`detect_format`). In that case the reported `extension` isn't necessarily the
real cause of the failure — see the doc comments on `classify_file` and
`ImportError::UnsupportedFormat` for detail.

## Note on `ImportError::InvalidFileName`

A file whose name yields no destination basename (empty, `.`, `..`, or a
trailing separator) is rejected with `ImportError::InvalidFileName { path }`,
not `UnsupportedFormat`. The two failures need different UI messages ("fix
the file name" vs. "convert to JPG/DNG"), so they are separate variants even
though both land in `ImportReport.rejected`.

## Note on `ImportError::MissingContentHash`

A candidate whose `content_hash` is empty was never hashed (for example its
read failed). It is rejected with `ImportError::MissingContentHash { path }`
instead of being compared: an empty string would otherwise make unrelated
files duplicates of one another. Persisted `known` entries with an empty hash
are ignored for the same reason.
