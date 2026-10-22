import type { SpatialResult } from "../types/map";

/**
 * Membangun markup string HTML tabel atribut untuk popup hasil analisis spasial.
 *
 * Fungsi murni deterministik tanpa dependensi React maupun Leaflet DOM.
 * Catatan: Nilai properti belum disanitasi/di-escape (lihat catatan temuan XSS di implementasi-frontend.md).
 */
export function buildPopupHtml(result: SpatialResult, layerName: string): string {
  const formatKey = (k: string) => k.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());

  const rows = Object.entries(result.properties)
    .map(
      ([key, value]) => `
      <tr>
        <td style="color: var(--color-slate); font-weight: 600; padding: 6px 0;">${formatKey(key)}</td>
        <td style="color: var(--color-ink); font-weight: 500; text-align: right; font-family: var(--font-sf-pro-text); padding: 6px 0;">${String(value)}</td>
      </tr>
    `,
    )
    .join("");

  return `
    <div style="
      background: var(--color-gallery-white);
      border-radius: 16px;
      padding: 16px;
      min-width: 260px;
      box-shadow: 0 8px 32px rgba(0, 0, 0, 0.12), var(--shadow-subtle);
      font-family: var(--font-sf-pro-text);
      border: 1px solid var(--color-hairline-silver);
    ">
      <div style="font-size: 15px; font-weight: 700; color: var(--color-ink); margin-bottom: 12px; letter-spacing: -0.2px; line-height: 1.3;">
        ${layerName}
      </div>
      
      <div style="margin: 12px 0; border-bottom: 1px solid var(--color-hairline-silver);"></div>
      
      <table style="width: 100%; border-collapse: collapse; font-size: 13px;">
        <tbody>
          ${rows}
        </tbody>
      </table>
    </div>
  `;
}
