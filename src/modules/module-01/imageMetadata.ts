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
  flash?: string | null;
  whiteBalance?: string | null;
  meteringMode?: string | null;
  exposureMode?: string | null;
  digitalZoomRatio?: number | null;
  colorSpace?: string | null;
  orientation?: string | null;

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

export interface MetadataCompleteness {
  present: (keyof ImageMetadata)[];
  missing: (keyof ImageMetadata)[];
  missingRequired: (keyof ImageMetadata)[];
}

/** Pure, mirrors Rust's `describe_completeness`/`field_presence`. Independent of mock vs real origin. */
export function describeCompleteness(meta: ImageMetadata): MetadataCompleteness {
  const present: (keyof ImageMetadata)[] = [];
  const missing: (keyof ImageMetadata)[] = [];

  for (const field of ALL_TRACKED_FIELDS) {
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

/**
 * Deterministic mock metadata keyed off the image row id, so repeated panel opens for the same row
 * always show the same (illustrative) values. Some fields are deliberately left absent so the
 * completeness badge has something real to react to.
 *
 * TODO: replace with a real `extract_metadata` Tauri command call once the backend is wired.
 */
export function buildMockImageMetadata(row: { id: string; takenIso: string }): ImageMetadata {
  // Simple positive hash of the row id, used only to vary mock values deterministically.
  let seed = 0;
  for (let i = 0; i < row.id.length; i++) {
    seed = (seed * 31 + row.id.charCodeAt(i)) >>> 0;
  }
  const isDjiDrone = seed % 2 === 0;
  const hasGps = seed % 5 !== 0;

  return {
    gpsLatitude: hasGps ? -6.9 + (seed % 100) / 1000 : null,
    gpsLongitude: hasGps ? 107.6 + (seed % 100) / 1000 : null,
    gpsAltitudeM: hasGps ? 120 + (seed % 40) : null,
    absoluteAltitudeM: isDjiDrone ? 118 + (seed % 40) : null,
    dateTimeOriginal: row.takenIso,
    width: 4000,
    height: 3000,
    format: "Jpeg",
    make: isDjiDrone ? "DJI" : "Hasselblad",
    cameraModelName: isDjiDrone ? "FC7303" : "L1D-20c",
    exposureTimeS: 1 / 1000,
    fNumber: 2.8,
    iso: 100,
    focalLengthMm: 8.8,
    focalLength35mm: 24,
    flash: "No Flash",
    whiteBalance: "Auto",
    meteringMode: "Center-weighted average",
    exposureMode: "Auto",
    digitalZoomRatio: 1,
    colorSpace: "sRGB",
    orientation: "Horizontal (normal)",
    relativeAltitudeM: isDjiDrone ? 80 + (seed % 20) : null,
    gimbalRollDegree: isDjiDrone ? 0 : null,
    gimbalYawDegree: isDjiDrone ? (seed % 360) - 180 : null,
    gimbalPitchDegree: isDjiDrone ? -90 : null,
    flightRollDegree: isDjiDrone ? 0 : null,
    flightYawDegree: isDjiDrone ? (seed % 360) - 180 : null,
    flightPitchDegree: isDjiDrone ? 0 : null,
    flightXSpeed: isDjiDrone ? 2.1 : null,
    flightYSpeed: isDjiDrone ? 0.4 : null,
    flightZSpeed: isDjiDrone ? 0 : null,
  };
}
