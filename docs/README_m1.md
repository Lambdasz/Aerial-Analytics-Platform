# Module 1 — Image Metadata Extraction & RGB Import

Source: `src-tauri/src/modules/module_01/`

This module is Module 1's back-end for two responsibilities:

- **Image import** (`image_importer.rs`) — classifies and validates a batch
  of picked files, checks for name collisions and duplicates, and produces an
  `ImportReport`.
- **Metadata extraction** (`metadata_extractor.rs`) — reads EXIF and XMP DJI
  tags from a single JPEG/DNG file into an `ImageMetadata` value.

It sits below the organisational hierarchy defined in
`src-tauri/src/models/session.rs`:

```text
Project
└── Session
    └── Image  (the persisted record this module's output eventually becomes)
```

## Status

Implemented. Import entry points, the extraction pipeline (format sniff, JPEG
and DNG/TIFF parsers, XMP DJI scan, merge helpers), and the schema migration
are all live and covered by unit tests. Nothing in this module has a Tauri
command yet, so the UI reaches none of it directly.

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
    describe_completeness, extract_metadata, import_images, to_plugin_metadata,
    MetadataCompleteness, MetadataError,
};
```

## API reference

### `import_images`

```rust
pub(crate) fn import_images(
    request: &ImportRequest,
    known: &[(String, ContentHash)],
    existing_paths: &[String],
    folding: NameFolding,
) -> Result<ImportReport, ImportError> {
    // ...
}
```

Entry point for RGB image import. Composes per-file format classification,
destination-name-collision detection, and duplicate detection into one
`ImportReport`. Does not copy files or touch the disk or the database —
`request` is already populated with file headers/hashes/ids by I/O before
this runs, and the caller applies the returned `ImportReport` afterwards.
`existing_paths` are the files already in the session folder, whose
basenames are compared against incoming names. `folding` says how the
destination filesystem decides two names are the same file:

- `Exact` — byte-identical names only (most Linux filesystems).
- `Ntfs` — names differing only by case; composed and decomposed accents are
  distinct files.
- `Apfs` — names differing only by case or by canonical (NFC/NFD) form.

None of them folds `ß` to `ss`: no target filesystem does.

- **Errors**: `ImportError::NoImagesFound` if `request` has no candidates.
  Per-candidate failures (bad format, magic mismatch, invalid file name, missing
  content hash) are
  reported in the result's `rejected` list, not as an `Err`.
- **The caller validates the session.** This function has no session list, so
  it never returns `ImportError::SessionNotFound`. The command that owns the
  database checks an `Existing` id via `get_session` before calling in.
- A name conflict holds a candidate out of `imported_image_ids` until the
  frontend resolves it. The backend never renames. Intra-batch duplicates
  are flagged against the first file that was accepted for import. A file
  whose content matches a file this call _held back_ is not a duplicate
  (the held-back file may never be imported); it is listed in
  `pending_duplicates` so it can be imported once the conflict is resolved.

### `extract_metadata`

```rust
pub(crate) fn extract_metadata(path: &Path) -> Result<ImageMetadata, MetadataError> {
    // ...
}
```

Reads a single JPEG/DNG file from disk and extracts its full metadata. This
is the only impure function in the module (the only one that touches the
filesystem); it's defined as the composition:

```text
read file (JPEG: up to the SOS header; DNG: whole file)
  -> detect_format(header, extension)
  -> read_dimensions(bytes, format)
  -> parse_exif(bytes)
  -> xmp_packet(bytes, format) -> parse_xmp_dji(packet)
  -> merge_tags(exif, xmp, dimensions, format)
```

A tag missing from the file is not an error — the corresponding
`ImageMetadata` field is simply `None`.

- **Errors**: `MetadataError::Io` (file unreadable), `UnsupportedFormat` /
  `MagicMismatch` (not a supported image), `MalformedExif` (a present EXIF
  segment couldn't be parsed).

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
import_images(request, known, existing_paths)
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
}
```

The one fully-public type in this module — the domain error for every
metadata-extraction operation. Each variant has a `code()` method
(`"IO_ERROR"`, `"UNSUPPORTED_FORMAT"`, `"MAGIC_MISMATCH"`,
`"MALFORMED_EXIF"`, `"MALFORMED_XMP"`, `"NOT_IMPLEMENTED"`) for programmatic
matching on the frontend, and converts via `From<MetadataError> for
CommandError` so it can be returned directly from a `#[tauri::command]` and
reach the frontend over IPC.

## Note on `ImportError::UnsupportedFormat`

`image_importer::classify_file` (an internal helper behind `import_images`)
also raises `ImportError::UnsupportedFormat` as a defensive fallback if the
format detector fails in a way its own contract says can't happen (an
`Io`/`MalformedExif`/`MalformedXmp` `MetadataError` from `detect_format`). In
that case the reported `extension` isn't necessarily the real cause of the
failure — see the doc comments on `classify_file` and
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
