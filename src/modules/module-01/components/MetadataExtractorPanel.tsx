import { useEffect, useMemo, useState, type FC, type ReactNode } from "react";
import { Button, Callout, Dialog, Intent, Tag } from "@blueprintjs/core";
import {
  describeCompleteness,
  hasAnyDjiTelemetry,
  imageFormatFromDb,
  METADATA_FIELD_LABELS,
  REQUIRED_METADATA_FIELDS,
  STORED_METADATA_FIELDS,
  type ImageMetadata,
} from "../imageMetadata";
import { formatSingleDate } from "../sessionStats";
import { getImageMetadata, type StoredImage } from "../sessionCommands";
import "./MetadataExtractorPanel.css";

interface MetadataExtractorPanelProps {
  image: StoredImage | null;
  onClose: () => void;
}

function fileNameOf(path: string): string {
  const parts = path.split(/[/\\]/);
  return parts[parts.length - 1] || path;
}

/**
 * Builds the metadata view from the fields import persisted on the image row.
 *
 * That is the stored subset (location, capture time, dimensions, format, camera); exposure
 * and XMP flight-telemetry fields are not persisted, so they show as unavailable. Reading
 * them from the file needs `get_image_metadata`, which reads the stored path.
 */
function metadataFromStoredImage(image: StoredImage): ImageMetadata {
  return {
    gpsLatitude: image.locationLat,
    gpsLongitude: image.locationLon,
    gpsAltitudeM: image.gpsAltitudeM,
    dateTimeOriginal: image.capturedAt,
    width: image.width,
    height: image.height,
    format: imageFormatFromDb(image.format),
    make: image.make,
    cameraModelName: image.cameraModel,
  };
}

const EMPTY = <span className="m1-sd-fact__empty">—</span>;

function fact(label: string, value: ReactNode) {
  return (
    <div className="m1-sd-fact" key={label}>
      <dt>{label}</dt>
      <dd>{value ?? EMPTY}</dd>
    </div>
  );
}

function formatCoordinate(value?: number | null): string | null {
  return value === null || value === undefined ? null : `${value.toFixed(5)}°`;
}

function formatMeters(value?: number | null): string | null {
  return value === null || value === undefined ? null : `${value.toFixed(1)} m`;
}

function formatDegrees(value?: number | null): string | null {
  return value === null || value === undefined ? null : `${value.toFixed(1)}°`;
}

function formatSpeed(value?: number | null): string | null {
  return value === null || value === undefined ? null : `${value.toFixed(1)} m/s`;
}

const WHITE_BALANCE: Record<number, string> = { 0: "Auto", 1: "Manual" };
const EXPOSURE_MODE: Record<number, string> = { 0: "Auto", 1: "Manual", 2: "Auto bracket" };
const METERING_MODE: Record<number, string> = {
  1: "Average",
  2: "Center-weighted average",
  3: "Spot",
  5: "Pattern",
};
const COLOR_SPACE: Record<number, string> = { 1: "sRGB", 65535: "Uncalibrated" };
const ORIENTATION: Record<number, string> = {
  1: "Horizontal (normal)",
  3: "Rotate 180",
  6: "Rotate 90 CW",
  8: "Rotate 270 CW",
};

/** EXIF stores these as codes. A string is already a label (stored-row fallback). */
function formatCode(
  value: number | string | null | undefined,
  labels: Record<number, string>,
): string | null {
  if (value === null || value === undefined) return null;
  if (typeof value === "string") return value;
  return labels[value] ?? String(value);
}

function formatFlash(value: number | string | null | undefined): string | null {
  if (value === null || value === undefined) return null;
  if (typeof value === "string") return value;
  return (value & 1) === 1 ? "Menyala" : "Tidak menyala";
}

export const MetadataExtractorPanel: FC<MetadataExtractorPanelProps> = ({ image, onClose }) => {
  const stored = useMemo(() => (image ? metadataFromStoredImage(image) : null), [image]);
  const [live, setLive] = useState<{ id: string; meta: ImageMetadata } | null>(null);
  const [failedId, setFailedId] = useState<string | null>(null);

  useEffect(() => {
    if (!image) return;
    const id = image.id;
    let alive = true;
    getImageMetadata(id)
      .then((meta) => {
        if (alive) setLive({ id, meta });
      })
      .catch(() => {
        if (alive) setFailedId(id);
      });
    return () => {
      alive = false;
    };
  }, [image]);

  const matched = live !== null && live.id === image?.id ? live : null;
  const currentLive = matched?.meta ?? null;
  const readFailed = failedId === image?.id;
  const reading = Boolean(image) && currentLive === null && !readFailed;
  // A successful read is the file. Until then, only the columns import stored
  // count — exposure and XMP were never on the row, so they must not look missing.
  const metadata = currentLive ?? stored;
  const completeness = useMemo(
    () =>
      metadata
        ? describeCompleteness(metadata, currentLive ? undefined : STORED_METADATA_FIELDS)
        : null,
    [metadata, currentLive],
  );

  if (!image || !metadata || !completeness) return null;

  const isComplete = completeness.missingRequired.length === 0;
  const djiAvailable = hasAnyDjiTelemetry(metadata);
  const counted = completeness.present.length + completeness.missing.length;

  return (
    <Dialog
      isOpen
      onClose={onClose}
      title={`Metadata — ${fileNameOf(image.filePath)}`}
      className="m1-custom-dialog m1-meta-dialog"
      portalClassName="m1-dialog-portal"
    >
      <div className="m1-dialog-content m1-meta-content">
        <Callout
          intent={isComplete ? Intent.SUCCESS : Intent.WARNING}
          icon={isComplete ? "tick-circle" : "warning-sign"}
        >
          <Tag minimal intent={isComplete ? Intent.SUCCESS : Intent.WARNING}>
            {REQUIRED_METADATA_FIELDS.length - completeness.missingRequired.length}/
            {REQUIRED_METADATA_FIELDS.length} field wajib lengkap
          </Tag>
          {!isComplete && (
            <p className="m1-meta-missing">
              Field wajib yang belum lengkap:{" "}
              {completeness.missingRequired.map((field) => METADATA_FIELD_LABELS[field]).join(", ")}
              .
            </p>
          )}
          <p className="m1-meta-note">
            {reading
              ? "Membaca metadata dari berkas…"
              : `${completeness.present.length} dari ${counted} field yang ${currentLive ? "dibaca dari berkas" : "tersimpan saat impor"} tersedia.`}
          </p>
          {readFailed && (
            <p className="m1-meta-note">
              Berkas tidak bisa dibaca. Eksposur dan telemetri tidak tersimpan di baris citra, jadi
              tidak dihitung di sini.
            </p>
          )}
        </Callout>

        <section className="m1-meta-group">
          <h3>Dimensi &amp; Format</h3>
          <dl className="m1-sd-facts m1-tabular">
            {fact(
              "Ukuran",
              metadata.width && metadata.height
                ? `${metadata.width} × ${metadata.height} px`
                : null,
            )}
            {fact("Format berkas", metadata.format)}
            {fact("Orientasi", formatCode(metadata.orientation, ORIENTATION))}
          </dl>
        </section>

        <section className="m1-meta-group">
          <h3>Waktu</h3>
          <dl className="m1-sd-facts m1-tabular">
            {fact(
              "Waktu pengambilan",
              metadata.dateTimeOriginal ? formatSingleDate(metadata.dateTimeOriginal, true) : null,
            )}
          </dl>
        </section>

        <section className="m1-meta-group">
          <h3>Lokasi &amp; GPS</h3>
          <dl className="m1-sd-facts m1-tabular">
            {fact(
              "Koordinat",
              metadata.gpsLatitude !== null &&
                metadata.gpsLatitude !== undefined &&
                metadata.gpsLongitude !== null &&
                metadata.gpsLongitude !== undefined
                ? `${formatCoordinate(metadata.gpsLatitude)}, ${formatCoordinate(metadata.gpsLongitude)}`
                : null,
            )}
            {fact("Altitude GPS", formatMeters(metadata.gpsAltitudeM))}
            {fact("Altitude absolut", formatMeters(metadata.absoluteAltitudeM))}
          </dl>
        </section>

        <section className="m1-meta-group">
          <h3>Kamera &amp; Eksposur</h3>
          <dl className="m1-sd-facts m1-tabular">
            {fact("Merek kamera", metadata.make)}
            {fact("Model kamera", metadata.cameraModelName)}
            {fact(
              "Waktu eksposur",
              metadata.exposureTimeS ? `1/${Math.round(1 / metadata.exposureTimeS)} s` : null,
            )}
            {fact("Bukaan (f-number)", metadata.fNumber ? `f/${metadata.fNumber}` : null)}
            {fact("ISO", metadata.iso)}
            {fact("Panjang fokus", metadata.focalLengthMm ? `${metadata.focalLengthMm} mm` : null)}
            {fact(
              "Panjang fokus (35mm)",
              metadata.focalLength35mm ? `${metadata.focalLength35mm} mm` : null,
            )}
            {fact("Flash", formatFlash(metadata.flash))}
            {fact("White balance", formatCode(metadata.whiteBalance, WHITE_BALANCE))}
            {fact("Mode metering", formatCode(metadata.meteringMode, METERING_MODE))}
            {fact("Mode eksposur", formatCode(metadata.exposureMode, EXPOSURE_MODE))}
            {fact("Rasio zoom digital", metadata.digitalZoomRatio)}
            {fact("Color space", formatCode(metadata.colorSpace, COLOR_SPACE))}
          </dl>
        </section>

        <section className="m1-meta-group">
          <h3>Telemetry Penerbangan (XMP DJI)</h3>
          {djiAvailable ? (
            <dl className="m1-sd-facts m1-tabular">
              {fact("Altitude relatif", formatMeters(metadata.relativeAltitudeM))}
              {fact("Gimbal roll", formatDegrees(metadata.gimbalRollDegree))}
              {fact("Gimbal yaw", formatDegrees(metadata.gimbalYawDegree))}
              {fact("Gimbal pitch", formatDegrees(metadata.gimbalPitchDegree))}
              {fact("Roll pesawat", formatDegrees(metadata.flightRollDegree))}
              {fact("Yaw pesawat", formatDegrees(metadata.flightYawDegree))}
              {fact("Pitch pesawat", formatDegrees(metadata.flightPitchDegree))}
              {fact("Kecepatan X", formatSpeed(metadata.flightXSpeed))}
              {fact("Kecepatan Y", formatSpeed(metadata.flightYSpeed))}
              {fact("Kecepatan Z", formatSpeed(metadata.flightZSpeed))}
            </dl>
          ) : (
            <p className="m1-meta-note">
              {reading
                ? "Membaca telemetri dari berkas…"
                : readFailed
                  ? "Telemetri XMP tidak tersimpan di baris citra, dan berkasnya tidak bisa dibaca."
                  : "Tidak ada tag telemetri XMP DJI pada berkas ini."}
            </p>
          )}
        </section>
      </div>
      <div className="m1-dialog-footer">
        <Button text="Tutup" onClick={onClose} />
      </div>
    </Dialog>
  );
};
