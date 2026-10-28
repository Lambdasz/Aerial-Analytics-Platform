/**
 * Mirrors `src-tauri/src/models/image.rs`'s `ImageMetadata` struct. Every field is optional because
 * EXIF/XMP tags are frequently absent depending on camera/drone make and shooting conditions.
 */
export interface ImageMetadata {
  // Geolocation
  gpsLatitude?: number | null;
  gpsLongitude?: number | null;
  gpsAltitudeM?: number | null;
  absoluteAltitudeM?: number | null;

  // Timestamp
  dateTimeOriginal?: string | null;

  // Dimensions
  width?: number | null;
  height?: number | null;

  // Imagery / exposure
  format?: "Jpeg" | "Dng" | null;
  make?: string | null;
  cameraModelName?: string | null;
  exposureTimeS?: number | null;
  fNumber?: number | null;
  iso?: number | null;
  focalLengthMm?: number | null;
  focalLength35mm?: number | null;
  flash?: number | string | null;
  whiteBalance?: number | string | null;
  meteringMode?: number | string | null;
  exposureMode?: number | string | null;
  digitalZoomRatio?: number | null;
  colorSpace?: number | string | null;
  orientation?: number | string | null;

  // Flight telemetry (XMP DJI)
  relativeAltitudeM?: number | null;
  gimbalRollDegree?: number | null;
  gimbalYawDegree?: number | null;
  gimbalPitchDegree?: number | null;
  flightRollDegree?: number | null;
  flightYawDegree?: number | null;
  flightPitchDegree?: number | null;
  flightXSpeed?: number | null;
  flightYSpeed?: number | null;
  flightZSpeed?: number | null;
}

/** Mirrors the Rust `REQUIRED_METADATA_FIELDS` constant. */
export const REQUIRED_METADATA_FIELDS: (keyof ImageMetadata)[] = [
  "gpsLatitude",
  "gpsLongitude",
  "gpsAltitudeM",
  "dateTimeOriginal",
  "width",
  "height",
];

const DJI_TELEMETRY_FIELDS: (keyof ImageMetadata)[] = [
  "relativeAltitudeM",
  "gimbalRollDegree",
  "gimbalYawDegree",
  "gimbalPitchDegree",
  "flightRollDegree",
  "flightYawDegree",
  "flightPitchDegree",
  "flightXSpeed",
  "flightYSpeed",
  "flightZSpeed",
];

export function hasAnyDjiTelemetry(meta: ImageMetadata): boolean {
  return DJI_TELEMETRY_FIELDS.some((field) => meta[field] !== null && meta[field] !== undefined);
}

/** Indonesian display labels for every tracked field. */
export const METADATA_FIELD_LABELS: Record<keyof ImageMetadata, string> = {
  gpsLatitude: "Lintang GPS",
  gpsLongitude: "Bujur GPS",
  gpsAltitudeM: "Altitude GPS",
  absoluteAltitudeM: "Altitude absolut",
  dateTimeOriginal: "Waktu pengambilan",
  width: "Lebar",
  height: "Tinggi",
  format: "Format berkas",
  make: "Merek kamera",
  cameraModelName: "Model kamera",
  exposureTimeS: "Waktu eksposur",
  fNumber: "Bukaan (f-number)",
  iso: "ISO",
  focalLengthMm: "Panjang fokus",
  focalLength35mm: "Panjang fokus (35mm)",
  flash: "Flash",
  whiteBalance: "White balance",
  meteringMode: "Mode metering",
  exposureMode: "Mode eksposur",
  digitalZoomRatio: "Rasio zoom digital",
  colorSpace: "Color space",
  orientation: "Orientasi",
  relativeAltitudeM: "Altitude relatif",
  gimbalRollDegree: "Gimbal roll",
  gimbalYawDegree: "Gimbal yaw",
  gimbalPitchDegree: "Gimbal pitch",
  flightRollDegree: "Roll pesawat",
  flightYawDegree: "Yaw pesawat",
  flightPitchDegree: "Pitch pesawat",
  flightXSpeed: "Kecepatan X",
  flightYSpeed: "Kecepatan Y",
  flightZSpeed: "Kecepatan Z",
};

const ALL_TRACKED_FIELDS: (keyof ImageMetadata)[] = Object.keys(
  METADATA_FIELD_LABELS,
) as (keyof ImageMetadata)[];

/** Fields the image row actually stores. Exposure and XMP are not among them. */
export const STORED_METADATA_FIELDS: (keyof ImageMetadata)[] = [
  "gpsLatitude",
  "gpsLongitude",
  "gpsAltitudeM",
  "dateTimeOriginal",
  "width",
  "height",
  "format",
  "make",
  "cameraModelName",
];

export interface MetadataCompleteness {
  present: (keyof ImageMetadata)[];
  missing: (keyof ImageMetadata)[];
  missingRequired: (keyof ImageMetadata)[];
}

/**
 * Pure, mirrors Rust's `describe_completeness`/`field_presence`.
 *
 * `fields` defaults to every tracked tag. Pass {@link STORED_METADATA_FIELDS}
 * when only the image row is available, so unstored exposure and XMP tags are
 * not counted as missing.
 */
export function describeCompleteness(
  meta: ImageMetadata,
  fields: readonly (keyof ImageMetadata)[] = ALL_TRACKED_FIELDS,
): MetadataCompleteness {
  const present: (keyof ImageMetadata)[] = [];
  const missing: (keyof ImageMetadata)[] = [];

  for (const field of fields) {
    const value = meta[field];
    if (value === null || value === undefined) {
      missing.push(field);
    } else {
      present.push(field);
    }
  }

  const missingRequired = REQUIRED_METADATA_FIELDS.filter((field) => missing.includes(field));

  return { present, missing, missingRequired };
}

/** Maps the lowercase `format` string the database stores onto the EXIF-style enum. */
export function imageFormatFromDb(format: string | null): ImageMetadata["format"] {
  if (format === "jpeg") return "Jpeg";
  if (format === "dng") return "Dng";
  return null;
}
