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
 * Pembungkus kartu container umum untuk seluruh varian Leaflet popup di Modul 03.
 * Mengikuti sistem desain Modern Soft Card (iOS 18 / Apple HIG Minimalist).
 */
export function wrapPopupCard(
  contentHtml: string,
  options: { minWidth?: number; maxWidth?: number } = {},
): string {
  const minWidth = options.minWidth ?? 280;
  const maxWidth = options.maxWidth ?? 330;
  return `
    <div style="
      background: #ffffff;
      border-radius: 16px;
      padding: 16px 18px;
      min-width: ${minWidth}px;
      max-width: ${maxWidth}px;
      box-shadow: 0 4px 20px -2px rgba(0, 0, 0, 0.08), 0 1px 4px rgba(0, 0, 0, 0.03);
      font-family: var(--font-sf-pro-text, -apple-system, BlinkMacSystemFont, sans-serif);
      border: none;
    ">
      ${contentHtml}
    </div>
  `;
}

/**
 * Helper untuk merender baris header popup dengan badge pill dan indikator status / tag kanan.
 */
export function renderPopupHeader(
  badgeText: string,
  dotColor: string,
  rightElementHtml: string = "",
): string {
  return `
    <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px; padding-right: 22px;">
      <span style="display: inline-flex; align-items: center; gap: 5px; background: #f1f5f9; color: #475569; font-size: 10px; font-weight: 600; padding: 3px 9px; border-radius: 999px; text-transform: uppercase; letter-spacing: 0.5px;">
        <span style="width: 6px; height: 6px; border-radius: 50%; background: ${dotColor};"></span>
        ${badgeText}
      </span>
      ${rightElementHtml}
    </div>
  `;
}

/**
 * Helper untuk merender judul utama dan subtitle opsional (seperti ID atau kode unik).
 */
export function renderPopupTitle(titleHtml: string, subtitleHtml: string = ""): string {
  const marginBottom = subtitleHtml ? "4px" : "10px";
  return `
    <div style="font-size: 15px; font-weight: 700; color: #0f172a; margin-bottom: ${marginBottom}; letter-spacing: -0.2px; line-height: 1.35; word-break: break-word;">
      ${titleHtml}
    </div>
    ${subtitleHtml}
  `;
}

/**
 * Helper untuk garis pembatas halus (divider) antar seksi di dalam popup.
 */
export function renderPopupDivider(marginBottom: number = 8): string {
  return `<div style="margin-bottom: ${marginBottom}px; border-bottom: 1px solid #f1f5f9;"></div>`;
}

/**
 * Helper untuk merender tabel atribut dua kolom.
 */
export function renderPopupTable(rowsHtml: string): string {
  return `
    <table style="width: 100%; border-collapse: collapse; font-size: 12px;">
      <tbody>
        ${rowsHtml}
      </tbody>
    </table>
  `;
}

/**
 * Helper untuk merender satu baris tabel atribut (label + nilai).
 */
export function renderPopupTableRow(
  label: string,
  valueHtml: string,
  isLast: boolean = false,
  fontMonospace: boolean = false,
): string {
  const borderBottom = isLast ? "" : "border-bottom: 1px solid #f1f5f9;";
  const monoStyle = fontMonospace ? "font-family: monospace;" : "";
  return `
    <tr style="${borderBottom}">
      <td style="color: #64748b; font-weight: 500; padding: 6px 0; font-size: 12px;">${label}</td>
      <td style="color: #0f172a; font-weight: 600; text-align: right; padding: 6px 0; font-size: 12px; ${monoStyle}">${valueHtml}</td>
    </tr>
  `;
}

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
      return renderPopupTableRow(escapeHtml(field.label), escapeHtml(formattedVal));
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

  const header = renderPopupHeader(
    "Hasil Analisis Spasial",
    safeColor,
    `<span style="font-size: 11px; color: #94a3b8; font-weight: 500;">${safeSource}</span>`,
  );
  const title = renderPopupTitle(safeLayerName);
  const divider = renderPopupDivider(8);
  const table = renderPopupTable(rows);

  return wrapPopupCard(`${header}${title}${imageSection}${divider}${table}`, {
    minWidth: 270,
    maxWidth: 320,
  });
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

  const safeName = escapeHtml(name);
  const safeAoiId = escapeHtml(aoi_id);
  const safeTypeName = escapeHtml(typeName);

  const header = renderPopupHeader(
    "Area of Interest (AOI)",
    "#0f172a",
    `<span style="background: #f1f5f9; color: #475569; font-size: 10px; font-weight: 600; padding: 2px 8px; border-radius: 6px;">${safeTypeName}</span>`,
  );
  const title = renderPopupTitle(
    safeName,
    `<div style="font-size: 11px; color: #94a3b8; font-family: monospace; margin-bottom: 12px;">${safeAoiId}</div>`,
  );
  const divider = renderPopupDivider(10);
  const rows = `
    ${renderPopupTableRow("Luas Area", `${areaFormatted} m²${haFormatted}`)}
    ${renderPopupTableRow("Keliling", `${perimeterFormatted} m`)}
    ${renderPopupTableRow("Disimpan", dateFormatted, true)}
  `;
  const table = renderPopupTable(rows);

  return wrapPopupCard(`${header}${title}${divider}${table}`, {
    minWidth: 280,
    maxWidth: 330,
  });
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
      ${renderPopupTableRow("Latitude", latStr, false, true)}
      ${renderPopupTableRow("Longitude", lonStr, false, true)}
    `;
  } else if (geomType === "LineString") {
    const coords = annotation.geometry.coordinates;
    const lengthMeters = calculateLineLengthMeters(coords);
    const lengthStr =
      lengthMeters >= 1000
        ? `${(lengthMeters / 1000).toFixed(2)} km`
        : `${lengthMeters.toLocaleString("id-ID", { maximumFractionDigits: 1 })} m`;
    detailRows = `
      ${renderPopupTableRow("Jumlah Titik", `${coords.length} titik`)}
      ${renderPopupTableRow("Panjang Garis", lengthStr, false, true)}
    `;
  } else if (geomType === "Polygon") {
    const ring = (annotation.geometry.coordinates[0] || []) as [number, number][];
    const areaSqm = calculateGeodesicAreaMeters(ring);
    const areaStr =
      areaSqm >= 10000
        ? `${(areaSqm / 10000).toFixed(2)} ha`
        : `${areaSqm.toLocaleString("id-ID", { maximumFractionDigits: 1 })} m²`;
    detailRows = `
      ${renderPopupTableRow("Jumlah Sudut", `${ring.length} titik`)}
      ${renderPopupTableRow("Estimasi Luas", areaStr, false, true)}
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

  const safeId = escapeHtml(annotation.id || annotation.annotation_id || "");
  const safeText = escapeHtml(annotation.text || "Tanpa keterangan");

  const header = renderPopupHeader(
    "Anotasi Spasial",
    "#0f172a",
    `<span style="background: #f1f5f9; color: #475569; font-size: 10px; font-weight: 600; padding: 2px 8px; border-radius: 6px;">${geomLabel}</span>`,
  );
  const title = renderPopupTitle(
    safeText,
    `<div style="font-size: 11px; color: #94a3b8; font-family: monospace; margin-bottom: 12px;">${safeId}</div>`,
  );
  const divider = renderPopupDivider(10);
  const rows = `
    ${detailRows}
    ${renderPopupTableRow("Dibuat", dateFormatted, true)}
  `;
  const table = renderPopupTable(rows);

  return wrapPopupCard(`${header}${title}${divider}${table}`, {
    minWidth: 280,
    maxWidth: 320,
  });
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

  const header = renderPopupHeader(
    "Orthomosaic Drone",
    isEnabled ? "#0f172a" : "#94a3b8",
    `<span style="background: #f1f5f9; color: #475569; font-size: 10px; font-weight: 600; padding: 2px 8px; border-radius: 6px;">${
      isEnabled ? "Aktif" : "Nonaktif"
    }</span>`,
  );
  const title = renderPopupTitle(safeFilename);
  const rows = `
    ${renderPopupTableRow("Ukuran Citra", dimensionsText)}
    ${renderPopupTableRow("Format File", `${formatText} (${sizeText})`)}
    ${renderPopupTableRow(
      "Status Memori",
      isEnabled ? "Gambar Dimuat (Canvas)" : "Hemat Memori (Outline)",
      true,
    )}
  `;
  const table = renderPopupTable(rows);

  const resolutionWarning =
    img.display_dimensions && Math.max(img.display_dimensions[0], img.display_dimensions[1]) < 2048
      ? `
        <div style="margin-top: 10px; font-size: 11px; color: #b45309; background: #fffbeb; border: 1px solid #fef3c7; padding: 6px 10px; border-radius: 8px; line-height: 1.4;">
          ⚠️ <strong>Resolusi Display Dibatasi (&lt; 2.048 px):</strong><br/>
          Ukuran display dioptimalkan ke ${img.display_dimensions[0]}×${img.display_dimensions[1]} px untuk menghemat memori. Detail pada zoom tinggi mungkin berkurang.
        </div>
      `
      : "";

  const toggleControl = `
    <div style="margin-top: 14px; padding-top: 12px; border-top: 1px solid #f1f5f9; display: flex; align-items: center; justify-content: space-between; gap: 8px;">
      <div style="display: flex; flex-direction: column;">
        <span style="font-size: 12px; font-weight: 700; color: #0f172a;">Tampilkan Citra</span>
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
          justify-content: center;
          background: ${isEnabled ? "#0f172a" : "#f1f5f9"};
          color: ${isEnabled ? "#ffffff" : "#64748b"};
          border: none;
          padding: 5px 14px;
          border-radius: 999px;
          font-size: 11px;
          font-weight: 600;
          cursor: pointer;
          pointer-events: auto !important;
          user-select: none;
          box-shadow: none;
          transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
        "
      >
        <span style="pointer-events: none;">${isEnabled ? "ON" : "OFF"}</span>
      </button>
    </div>
  `;

  const memoryModeNote = !isEnabled
    ? `
      <div style="margin-top: 10px; font-size: 11px; color: #475569; background: #f8fafc; padding: 8px 12px; border-radius: 10px; line-height: 1.4;">
        💡 <strong>Mode Hemat Memori Aktif:</strong><br/>
        Gambar tidak dirender ke canvas. Arahkan kursor mouse ke area peta ini untuk melihat outline batas citra.
      </div>
    `
    : "";

  return wrapPopupCard(
    `${header}${title}${imageSection}${table}${resolutionWarning}${toggleControl}${memoryModeNote}`,
    { minWidth: 280, maxWidth: 330 },
  );
}
