import { useMemo, type FC, type ReactNode } from "react";
import { Button, Callout, Dialog, Intent, Tag } from "@blueprintjs/core";
import {
  describeCompleteness,
  hasAnyDjiTelemetry,
  imageFormatFromDb,
  METADATA_FIELD_LABELS,
  REQUIRED_METADATA_FIELDS,
  type ImageMetadata,
} from "../imageMetadata";
import { formatSingleDate } from "../sessionStats";
import type { StoredImage } from "../sessionCommands";
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
 * them live would need an `extract_metadata` Tauri command over `file_path`.
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

export const MetadataExtractorPanel: FC<MetadataExtractorPanelProps> = ({ image, onClose }) => {
  const metadata: ImageMetadata | null = useMemo(
    () => (image ? metadataFromStoredImage(image) : null),
    [image],
  );

  const completeness = useMemo(
    () => (metadata ? describeCompleteness(metadata) : null),
    [metadata],
  );

  if (!image || !metadata || !completeness) return null;

  const isComplete = completeness.missingRequired.length === 0;
  const djiAvailable = hasAnyDjiTelemetry(metadata);

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
            {completeness.present.length} dari{" "}
            {completeness.present.length + completeness.missing.length} field metadata tersedia.
          </p>
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
            {fact("Orientasi", metadata.orientation)}
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
            {fact("Flash", metadata.flash)}
            {fact("White balance", metadata.whiteBalance)}
            {fact("Mode metering", metadata.meteringMode)}
            {fact("Mode eksposur", metadata.exposureMode)}
            {fact("Rasio zoom digital", metadata.digitalZoomRatio)}
            {fact("Color space", metadata.colorSpace)}
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
              Tidak ada data telemetri XMP DJI (bukan drone DJI, atau tag tidak tersedia).
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
