import type { SpatialResult } from "../types/map";
import type { AoiFeature } from "../types/aoi";
import type { AnnotationFeature } from "../types/layer";
import type { StoredImageMetadata } from "../types/storage";
import { calculateLineLengthMeters, calculateGeodesicAreaMeters } from "./geoJson";

import type { FieldDef } from "../types/spatialResult";
import { resolveDisplayFields, formatFieldValue, resolveFeaturePhoto } from "./fieldResolution";
import { escapeHtml, isValidHexColor } from "./fieldValidation";
import { resolveOrthoDisplayUrl } from "./leaflet/spatialLayerBuilder";

/**
 * Membangun markup string HTML tabel atribut untuk popup hasil analisis spasial.
 *
 * Menerapkan resolusi field murni, pemformatan locale Indonesia, dan mitigasi XSS menyeluruh.
 */
export function buildPopupHtml(
  result: SpatialResult,
  layerName: string,
  layerImageUrl?: string | null,
  fieldsDef?: FieldDef[] | null,
  displayColor?: string,
): string {
  const props = result.properties || {};
  const rawImageUrl =
    result.image_url ??
    props.image_url ??
    props.photo_url ??
    props.thumbnail_url ??
    props.image ??
    layerImageUrl;

  const imageUrl = resolveFeaturePhoto(
    typeof rawImageUrl === "string" ? rawImageUrl : null,
    layerImageUrl,
  );

  // Resolusi field yang aktif dan dapat ditampilkan
  const resolvedFields = resolveDisplayFields(fieldsDef, [props]).filter(
    (f) => f.visible !== false,
  );

  const rows = resolvedFields
    .map((field) => {
      const val = props[field.key];
      if (val === undefined || val === null || val === "") return "";
      const formattedVal = formatFieldValue(val, field.unit, field.kind);
      return `
      <tr style="border-bottom: 1px solid rgba(0, 0, 0, 0.05);">
        <td style="color: var(--color-slate, #64748b); font-weight: 600; padding: 7px 0; font-size: 12px;">${escapeHtml(field.label)}</td>
        <td style="color: var(--color-ink, #0f172a); font-weight: 600; text-align: right; font-family: var(--font-sf-pro-text, sans-serif); padding: 7px 0; font-size: 13px;">${escapeHtml(formattedVal)}</td>
      </tr>
    `;
    })
    .filter(Boolean)
    .join("");

  const safeColor = displayColor && isValidHexColor(displayColor) ? displayColor : "#34c759";
  const safeLayerName = escapeHtml(layerName);
  const safeSource = escapeHtml(result.source || "");

  const imageSection = imageUrl
    ? `
      <div style="
        position: relative;
        margin-bottom: 12px;
        border-radius: 12px;
        overflow: hidden;
        background: #0f172a;
        box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
        border: 1px solid rgba(0, 0, 0, 0.08);
      ">
        <img
          src="${escapeHtml(imageUrl)}"
          alt="Pratinjau Hasil"
          style="
            width: 100%;
            height: 150px;
            object-fit: cover;
            display: block;
          "
          onerror="this.onerror=null;this.parentElement.style.display='none';"
        />
        <div style="
          position: absolute;
          bottom: 6px;
          right: 6px;
          background: rgba(15, 23, 42, 0.75);
          backdrop-filter: blur(4px);
          color: #ffffff;
          font-size: 9px;
          font-weight: 600;
          padding: 2px 7px;
          border-radius: 4px;
          text-transform: uppercase;
          letter-spacing: 0.5px;
        ">
          Foto Udara
        </div>
      </div>
    `
    : `
      <div style="
        position: relative;
        margin-bottom: 12px;
        border-radius: 12px;
        overflow: hidden;
        background: rgba(143, 153, 168, 0.08);
        border: 1px dashed rgba(143, 153, 168, 0.3);
        height: 72px;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        color: #8a9ba8;
        font-size: 11px;
        font-weight: 500;
      ">
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" style="margin-bottom: 4px; opacity: 0.6;">
          <rect width="18" height="18" x="3" y="3" rx="2" ry="2"/>
          <circle cx="9" cy="9" r="2"/>
          <path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"/>
        </svg>
        <span>No Image</span>
      </div>
    `;

  return `
    <div style="
      background: var(--color-gallery-white, #ffffff);
      border-radius: 16px;
      padding: 16px 18px;
      min-width: 270px;
      max-width: 320px;
      box-shadow: 0 10px 30px rgba(0, 0, 0, 0.12);
      font-family: var(--font-sf-pro-text, -apple-system, BlinkMacSystemFont, sans-serif);
      border: 1px solid var(--color-hairline-silver, #e2e8f0);
      border-left: 5px solid ${safeColor};
    ">
      <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px; padding-right: 22px;">
        <span style="display: inline-flex; align-items: center; gap: 5px; background: rgba(52, 199, 89, 0.12); color: #166534; font-size: 10px; font-weight: 700; padding: 3px 9px; border-radius: 999px; text-transform: uppercase; letter-spacing: 0.6px;">
          <span style="width: 6px; height: 6px; border-radius: 50%; background: ${safeColor};"></span>
          Hasil Analisis Spasial
        </span>
        <span style="font-size: 11px; color: #94a3b8; font-weight: 500;">${safeSource}</span>
      </div>

      <div style="font-size: 15px; font-weight: 700; color: var(--color-ink, #0f172a); margin-bottom: 10px; letter-spacing: -0.2px; line-height: 1.3;">
        ${safeLayerName}
      </div>

      ${imageSection}
      
      <div style="margin-bottom: 8px; border-bottom: 1px solid var(--color-hairline-silver, #e2e8f0);"></div>
      
      <table style="width: 100%; border-collapse: collapse;">
        <tbody>
          ${rows}
        </tbody>
      </table>
    </div>
  `;
}

export const buildSpatialResultPopupHtml = buildPopupHtml;

/**
 * Membangun markup string HTML untuk popup Area of Interest (AOI).
 *
 * Memiliki aksen visual biru (Apple Blue) dan badge AREA OF INTEREST (AOI)
 * sehingga secara visual terbedakan dengan jelas dari popup hasil analitik spasial.
 */
export function buildAoiPopupHtml(aoi: AoiFeature): string {
  const { aoi_id, name, calculated_area_sq_m, perimeter_m, created_at, aoi_type, tpe } =
    aoi.properties;

  const typeName = aoi_type || tpe || "Umum";
  const areaFormatted = calculated_area_sq_m.toLocaleString("id-ID", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
  const perimeterFormatted = perimeter_m.toLocaleString("id-ID", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });

  const haFormatted =
    calculated_area_sq_m >= 1000
      ? ` <span style="font-size: 11px; color: #64748b; font-weight: 400;">(${(calculated_area_sq_m / 10000).toFixed(4)} ha)</span>`
      : "";

  let dateFormatted = created_at;
  try {
    const d = new Date(created_at);
    if (!isNaN(d.getTime())) {
      dateFormatted = d.toLocaleString("id-ID", {
        year: "numeric",
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      });
    }
  } catch {
    // fallback
  }

  return `
    <div style="
      background: var(--color-gallery-white, #ffffff);
      border-radius: 16px;
      padding: 16px 18px;
      min-width: 280px;
      box-shadow: 0 10px 30px rgba(0, 0, 0, 0.12);
      font-family: var(--font-sf-pro-text, -apple-system, BlinkMacSystemFont, sans-serif);
      border: 1px solid var(--color-hairline-silver, #e2e8f0);
      border-left: 5px solid #007aff;
    ">
      <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px; padding-right: 22px;">
        <span style="display: inline-flex; align-items: center; gap: 5px; background: rgba(0, 122, 255, 0.12); color: #0056b3; font-size: 10px; font-weight: 700; padding: 3px 9px; border-radius: 999px; text-transform: uppercase; letter-spacing: 0.6px;">
          <span style="width: 6px; height: 6px; border-radius: 50%; background: #007aff;"></span>
          Area of Interest (AOI)
        </span>
        <span style="background: rgba(88, 86, 214, 0.1); color: #5856d6; font-size: 11px; font-weight: 600; padding: 2px 8px; border-radius: 6px;">
          ${typeName}
        </span>
      </div>

      <div style="font-size: 16px; font-weight: 700; color: var(--color-ink, #0f172a); margin-bottom: 4px; letter-spacing: -0.2px; line-height: 1.3;">
        ${name}
      </div>

      <div style="font-size: 11px; color: #64748b; font-family: monospace; margin-bottom: 12px;">
        ${aoi_id}
      </div>
      
      <div style="margin-bottom: 10px; border-bottom: 1px solid var(--color-hairline-silver, #e2e8f0);"></div>
      
      <table style="width: 100%; border-collapse: collapse; font-size: 12px;">
        <tbody>
          <tr style="border-bottom: 1px solid rgba(0, 0, 0, 0.05);">
            <td style="color: var(--color-slate, #64748b); font-weight: 600; padding: 6px 0;">Luas Area</td>
            <td style="color: var(--color-ink, #0f172a); font-weight: 600; text-align: right; padding: 6px 0;">
              ${areaFormatted} m²${haFormatted}
            </td>
          </tr>
          <tr style="border-bottom: 1px solid rgba(0, 0, 0, 0.05);">
            <td style="color: var(--color-slate, #64748b); font-weight: 600; padding: 6px 0;">Keliling</td>
            <td style="color: var(--color-ink, #0f172a); font-weight: 600; text-align: right; padding: 6px 0;">
              ${perimeterFormatted} m
            </td>
          </tr>
          <tr>
            <td style="color: var(--color-slate, #64748b); font-weight: 600; padding: 6px 0;">Disimpan</td>
            <td style="color: #64748b; font-weight: 500; text-align: right; padding: 6px 0; font-size: 11px;">
              ${dateFormatted}
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  `;
}

/**
 * Membangun markup string HTML untuk popup anotasi spasial (Spatial Annotation).
 *
 * Memiliki aksen visual oranye/amber (Amber/Orange) dan badge ANOTASI SPASIAL
 * yang konsisten dengan gaya kartu Apple popup AOI dan Hasil Analisis Spasial.
 */
export function buildAnnotationPopupHtml(annotation: AnnotationFeature): string {
  const geomType = annotation.geometry.type;
  const geomLabel = geomType === "Point" ? "Titik" : geomType === "Polygon" ? "Poligon" : "Garis";

  let detailRows = "";

  if (geomType === "Point") {
    const [lon, lat] = annotation.geometry.coordinates;
    const latStr = typeof lat === "number" ? lat.toFixed(6) : String(lat);
    const lonStr = typeof lon === "number" ? lon.toFixed(6) : String(lon);
    detailRows = `
      <tr style="border-bottom: 1px solid rgba(0, 0, 0, 0.05);">
        <td style="color: var(--color-slate, #64748b); font-weight: 600; padding: 6px 0;">Latitude</td>
        <td style="color: var(--color-ink, #0f172a); font-weight: 600; text-align: right; padding: 6px 0; font-family: monospace;">${latStr}</td>
      </tr>
      <tr style="border-bottom: 1px solid rgba(0, 0, 0, 0.05);">
        <td style="color: var(--color-slate, #64748b); font-weight: 600; padding: 6px 0;">Longitude</td>
        <td style="color: var(--color-ink, #0f172a); font-weight: 600; text-align: right; padding: 6px 0; font-family: monospace;">${lonStr}</td>
      </tr>
    `;
  } else if (geomType === "LineString") {
    const coords = annotation.geometry.coordinates;
    const lengthMeters = calculateLineLengthMeters(coords);
    const lengthStr =
      lengthMeters >= 1000
        ? `${(lengthMeters / 1000).toFixed(2)} km`
        : `${lengthMeters.toLocaleString("id-ID", { maximumFractionDigits: 1 })} m`;
    detailRows = `
      <tr style="border-bottom: 1px solid rgba(0, 0, 0, 0.05);">
        <td style="color: var(--color-slate, #64748b); font-weight: 600; padding: 6px 0;">Jumlah Titik</td>
        <td style="color: var(--color-ink, #0f172a); font-weight: 600; text-align: right; padding: 6px 0;">${coords.length} titik</td>
      </tr>
      <tr style="border-bottom: 1px solid rgba(0, 0, 0, 0.05);">
        <td style="color: var(--color-slate, #64748b); font-weight: 600; padding: 6px 0;">Panjang Garis</td>
        <td style="color: var(--color-ink, #0f172a); font-weight: 600; text-align: right; padding: 6px 0; font-family: monospace;">${lengthStr}</td>
      </tr>
    `;
  } else if (geomType === "Polygon") {
    const ring = (annotation.geometry.coordinates[0] || []) as [number, number][];
    const areaSqm = calculateGeodesicAreaMeters(ring);
    const areaStr =
      areaSqm >= 10000
        ? `${(areaSqm / 10000).toFixed(2)} ha`
        : `${areaSqm.toLocaleString("id-ID", { maximumFractionDigits: 1 })} m²`;
    detailRows = `
      <tr style="border-bottom: 1px solid rgba(0, 0, 0, 0.05);">
        <td style="color: var(--color-slate, #64748b); font-weight: 600; padding: 6px 0;">Jumlah Sudut</td>
        <td style="color: var(--color-ink, #0f172a); font-weight: 600; text-align: right; padding: 6px 0;">${ring.length} titik</td>
      </tr>
      <tr style="border-bottom: 1px solid rgba(0, 0, 0, 0.05);">
        <td style="color: var(--color-slate, #64748b); font-weight: 600; padding: 6px 0;">Estimasi Luas</td>
        <td style="color: var(--color-ink, #0f172a); font-weight: 600; text-align: right; padding: 6px 0; font-family: monospace;">${areaStr}</td>
      </tr>
    `;
  }

  let dateFormatted = annotation.created_at;
  try {
    const d = new Date(annotation.created_at);
    if (!isNaN(d.getTime())) {
      dateFormatted = d.toLocaleString("id-ID", {
        year: "numeric",
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      });
    }
  } catch {
    // fallback
  }

  const safeText = (annotation.text || "Tanpa keterangan")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");

  return `
    <div style="
      background: var(--color-gallery-white, #ffffff);
      border-radius: 16px;
      padding: 16px 18px;
      min-width: 280px;
      max-width: 320px;
      box-shadow: 0 10px 30px rgba(0, 0, 0, 0.12);
      font-family: var(--font-sf-pro-text, -apple-system, BlinkMacSystemFont, sans-serif);
      border: 1px solid var(--color-hairline-silver, #e2e8f0);
      border-left: 5px solid #f59e0b;
    ">
      <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px; padding-right: 22px;">
        <span style="display: inline-flex; align-items: center; gap: 5px; background: rgba(245, 158, 11, 0.12); color: #b45309; font-size: 10px; font-weight: 700; padding: 3px 9px; border-radius: 999px; text-transform: uppercase; letter-spacing: 0.6px;">
          <span style="width: 6px; height: 6px; border-radius: 50%; background: #f59e0b;"></span>
          Anotasi Spasial
        </span>
        <span style="background: rgba(245, 158, 11, 0.12); color: #b45309; font-size: 11px; font-weight: 600; padding: 2px 8px; border-radius: 6px;">
          ${geomLabel}
        </span>
      </div>

      <div style="font-size: 16px; font-weight: 700; color: var(--color-ink, #0f172a); margin-bottom: 4px; letter-spacing: -0.2px; line-height: 1.35; word-break: break-word;">
        ${safeText}
      </div>

      <div style="font-size: 11px; color: #64748b; font-family: monospace; margin-bottom: 12px;">
        ${annotation.id || annotation.annotation_id}
      </div>
      
      <div style="margin-bottom: 10px; border-bottom: 1px solid var(--color-hairline-silver, #e2e8f0);"></div>
      
      <table style="width: 100%; border-collapse: collapse; font-size: 12px;">
        <tbody>
          ${detailRows}
          <tr>
            <td style="color: var(--color-slate, #64748b); font-weight: 600; padding: 6px 0;">Dibuat</td>
            <td style="color: #64748b; font-weight: 500; text-align: right; padding: 6px 0; font-size: 11px;">
              ${dateFormatted}
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  `;
}

/**
 * Membangun markup string HTML Apple-style untuk popup citra orthomosaic drone.
 *
 * Mendukung status toggle ON / OFF (Mode Hemat Memori / Hover Outline).
 */
export function buildOrthoPopupHtml(img: StoredImageMetadata, isEnabled: boolean): string {
  const imgKey = img.id || img.filename;
  const safeFilename = escapeHtml(img.filename || "Citra Orthomosaic");
  const dimensionsText = img.dimensions ? `${img.dimensions[0]} × ${img.dimensions[1]} px` : "-";
  const formatText = img.format
    ? String(img.format).toUpperCase()
    : (img.extension || "JPG").toUpperCase();

  const sizeText =
    img.size_bytes >= 1024 * 1024
      ? `${(img.size_bytes / (1024 * 1024)).toFixed(2)} MB`
      : `${(img.size_bytes / 1024).toFixed(0)} KB`;

  const displayUrl = resolveOrthoDisplayUrl(img);
  const imageSection = displayUrl
    ? `
      <div style="
        position: relative;
        margin-bottom: 12px;
        border-radius: 12px;
        overflow: hidden;
        background: #0f172a;
        box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
        border: 1px solid rgba(0, 0, 0, 0.08);
      ">
        <img
          src="${escapeHtml(displayUrl)}"
          alt="${safeFilename}"
          style="
            width: 100%;
            height: 130px;
            object-fit: cover;
            display: block;
          "
          onerror="this.onerror=null;this.parentElement.style.display='none';"
        />
        <div style="
          position: absolute;
          bottom: 6px;
          right: 6px;
          background: rgba(15, 23, 42, 0.75);
          backdrop-filter: blur(4px);
          color: #ffffff;
          font-size: 9px;
          font-weight: 600;
          padding: 2px 7px;
          border-radius: 4px;
          text-transform: uppercase;
          letter-spacing: 0.5px;
        ">
          ${formatText}
        </div>
      </div>
    `
    : "";

  return `
    <div style="
      background: var(--color-gallery-white, #ffffff);
      border-radius: 16px;
      padding: 16px 18px;
      min-width: 280px;
      max-width: 330px;
      box-shadow: 0 10px 30px rgba(0, 0, 0, 0.12);
      font-family: var(--font-sf-pro-text, -apple-system, BlinkMacSystemFont, sans-serif);
      border: 1px solid var(--color-hairline-silver, #e2e8f0);
      border-left: 5px solid ${isEnabled ? "#34c759" : "#007aff"};
    ">
      <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px; padding-right: 22px;">
        <span style="display: inline-flex; align-items: center; gap: 5px; background: ${
          isEnabled ? "rgba(52, 199, 89, 0.12)" : "rgba(0, 122, 255, 0.12)"
        }; color: ${
          isEnabled ? "#15803d" : "#0056b3"
        }; font-size: 10px; font-weight: 700; padding: 3px 9px; border-radius: 999px; text-transform: uppercase; letter-spacing: 0.6px;">
          <span style="width: 6px; height: 6px; border-radius: 50%; background: ${
            isEnabled ? "#34c759" : "#007aff"
          };"></span>
          Orthomosaic Drone
        </span>
        <span style="background: ${
          isEnabled ? "rgba(16, 185, 129, 0.1)" : "rgba(100, 116, 139, 0.12)"
        }; color: ${
          isEnabled ? "#059669" : "#475569"
        }; font-size: 11px; font-weight: 600; padding: 2px 8px; border-radius: 6px;">
          ${isEnabled ? "ON • Ditampilkan" : "OFF • Hemat Memori"}
        </span>
      </div>

      <div style="font-size: 15px; font-weight: 700; color: var(--color-ink, #0f172a); margin-bottom: 10px; letter-spacing: -0.2px; line-height: 1.35; word-break: break-word;">
        ${safeFilename}
      </div>

      ${imageSection}

      <table style="width: 100%; border-collapse: collapse; font-size: 12px;">
        <tbody>
          <tr style="border-bottom: 1px solid rgba(0, 0, 0, 0.05);">
            <td style="color: var(--color-slate, #64748b); font-weight: 600; padding: 6px 0;">Ukuran Citra</td>
            <td style="color: var(--color-ink, #0f172a); font-weight: 600; text-align: right; padding: 6px 0;">
              ${dimensionsText}
            </td>
          </tr>
          <tr style="border-bottom: 1px solid rgba(0, 0, 0, 0.05);">
            <td style="color: var(--color-slate, #64748b); font-weight: 600; padding: 6px 0;">Format File</td>
            <td style="color: var(--color-ink, #0f172a); font-weight: 600; text-align: right; padding: 6px 0;">
              ${formatText} (${sizeText})
            </td>
          </tr>
          <tr>
            <td style="color: var(--color-slate, #64748b); font-weight: 600; padding: 6px 0;">Status Memori</td>
            <td style="color: ${
              isEnabled ? "#059669" : "#2563eb"
            }; font-weight: 600; text-align: right; padding: 6px 0; font-size: 11px;">
              ${isEnabled ? "Gambar Dimuat (Canvas)" : "Hemat Memori (Hover Outline)"}
            </td>
          </tr>
        </tbody>
      </table>

      ${
        img.display_dimensions &&
        Math.max(img.display_dimensions[0], img.display_dimensions[1]) < 2048
          ? `
        <div style="margin-top: 10px; font-size: 11px; color: #b45309; background: #fef3c7; border: 1px solid #fde68a; padding: 6px 10px; border-radius: 6px; line-height: 1.4;">
          ⚠️ <strong>Resolusi Display Dibatasi (&lt; 2.048 px):</strong><br/>
          Ukuran display dioptimalkan ke ${img.display_dimensions[0]}×${img.display_dimensions[1]} px untuk menghemat memori. Detail pada zoom tinggi mungkin berkurang.
        </div>
      `
          : ""
      }

      <!-- KONTROL TOGGLE ON/OFF -->
      <div style="margin-top: 14px; padding-top: 12px; border-top: 1px solid var(--color-hairline-silver, #e2e8f0); display: flex; align-items: center; justify-content: space-between; gap: 8px;">
        <div style="display: flex; flex-direction: column;">
          <span style="font-size: 12px; font-weight: 700; color: var(--color-ink, #0f172a);">Tampilkan Citra</span>
          <span style="font-size: 10px; color: #64748b;">
            ${isEnabled ? "Matikan untuk hemat memori" : "Hidupkan untuk render citra"}
          </span>
        </div>
        <button
          type="button"
          data-action="toggle-ortho"
          data-id="${escapeHtml(imgKey)}"
          style="
            display: inline-flex;
            align-items: center;
            gap: 6px;
            background: ${isEnabled ? "#007aff" : "#f1f5f9"};
            color: ${isEnabled ? "#ffffff" : "#334155"};
            border: 1px solid ${isEnabled ? "#0062cc" : "#cbd5e1"};
            padding: 6px 14px;
            border-radius: 999px;
            font-size: 12px;
            font-weight: 700;
            cursor: pointer;
            pointer-events: auto !important;
            user-select: none;
            box-shadow: ${isEnabled ? "0 2px 6px rgba(0, 122, 255, 0.3)" : "none"};
            transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
          "
        >
          <span style="width: 7px; height: 7px; border-radius: 50%; pointer-events: none; background: ${
            isEnabled ? "#34c759" : "#94a3b8"
          };"></span>
          <span style="pointer-events: none;">${isEnabled ? "ON (Tampil)" : "OFF (Hemat)"}</span>
        </button>
      </div>

      ${
        !isEnabled
          ? `
        <div style="margin-top: 10px; font-size: 10px; color: #0369a1; background: rgba(0, 122, 255, 0.08); padding: 6px 10px; border-radius: 8px; border: 1px dashed rgba(0, 122, 255, 0.35); line-height: 1.4;">
          💡 <strong>Mode Hemat Memori Aktif:</strong><br/>
          Gambar tidak dirender ke canvas. Arahkan kursor mouse ke area peta ini untuk melihat outline garis biru.
        </div>
      `
          : ""
      }
    </div>
  `;
}
