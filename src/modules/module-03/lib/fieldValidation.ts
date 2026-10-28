import type { FieldDef, FieldKind } from "../types/spatialResult";

/**
 * Daftar reserved keys yang tidak boleh digunakan sebagai custom field
 * dan tidak ditampilkan sebagai baris data field di popup.
 */
export const RESERVED_KEYS: ReadonlySet<string> = new Set([
  "image_url",
  "photo_url",
  "thumbnail_url",
  "image",
  "edited_fields",
  "source",
  "geometry",
  "properties",
  "__proto__",
  "constructor",
  "prototype",
]);

/**
 * Memeriksa apakah suatu key termasuk reserved key.
 */
export function isReservedKey(key: string): boolean {
  return RESERVED_KEYS.has(key.trim().toLowerCase());
}

/**
 * Sanitasi string untuk mencegah injeksi HTML / XSS pada popup Leaflet.
 */
export function escapeHtml(unsafe: string | null | undefined): string {
  if (unsafe === null || unsafe === undefined) return "";
  return String(unsafe)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

/**
 * Validasi kode warna HEX (tepat 3, 4, 6, atau 8 digit HEX: #RGB, #RGBA, #RRGGBB, #RRGGBBAA).
 */
export function isValidHexColor(color: string): boolean {
  return /^#([0-9a-fA-F]{3}|[0-9a-fA-F]{4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/.test(color.trim());
}

const CUSTOM_KEY_REGEX = /^[a-z][a-z0-9_]{0,39}$/;

/**
 * Validasi format key untuk field custom baru.
 * CATATAN PENTING: Hanya berlaku untuk field custom baru (custom: true).
 * Key bawaan plugin yang sudah ada diterima apa adanya.
 */
export function validateCustomFieldKey(key: string, existingKeys: string[] = []): string | null {
  const trimmed = key.trim();
  if (!trimmed) {
    return "Key field tidak boleh kosong";
  }
  if (!CUSTOM_KEY_REGEX.test(trimmed)) {
    return "Key harus diawali huruf kecil, hanya boleh huruf kecil, angka, dan underscore, serta maksimal 40 karakter";
  }
  if (isReservedKey(trimmed)) {
    return `Key '${trimmed}' adalah kata terlarang (reserved key)`;
  }
  const lowerExisting = existingKeys.map((k) => k.toLowerCase());
  if (lowerExisting.includes(trimmed.toLowerCase())) {
    return `Key '${trimmed}' sudah digunakan pada layer ini`;
  }
  return null;
}

/**
 * Validasi label tampilan field (1..40 karakter setelah di-trim, wajib teks tanpa HTML).
 */
export function validateFieldLabel(label: string): string | null {
  const trimmed = label.trim();
  if (!trimmed) {
    return "Label field tidak boleh kosong";
  }
  if (trimmed.length > 40) {
    return "Label field maksimal 40 karakter";
  }
  if (/<[^>]*>/g.test(trimmed)) {
    return "Label field tidak boleh mengandung tag HTML";
  }
  return null;
}

/**
 * Validasi satuan/unit field (opsional, 0..12 karakter setelah di-trim, teks tanpa HTML).
 */
export function validateFieldUnit(unit?: string | null): string | null {
  if (!unit) return null;
  const trimmed = unit.trim();
  if (trimmed.length > 12) {
    return "Unit field maksimal 12 karakter";
  }
  if (/<[^>]*>/g.test(trimmed)) {
    return "Unit field tidak boleh mengandung tag HTML";
  }
  return null;
}

/**
 * Validasi kuota field custom (maksimal 30 field custom).
 * Field bawaan plugin tidak dihitung dalam kuota ini.
 */
export function validateCustomFieldsCount(fields: FieldDef[]): string | null {
  const customCount = fields.filter((f) => f.custom).length;
  if (customCount > 30) {
    return "Jumlah field custom melebihi batas maksimal (30 field)";
  }
  return null;
}

/**
 * Validasi tipe nilai properti saat diedit manual.
 * Wajib menjaga tipe JSON nilai asli (kecuali field custom baru).
 */
export function validatePropertyValueType(
  existingVal: unknown,
  newVal: unknown,
  kind?: FieldKind,
): string | null {
  // Tolak struktur data kompleks
  if (typeof newVal === "object" && newVal !== null) {
    return "Struktur objek atau array tidak dapat diedit secara manual";
  }

  // Jika nilai baru kosong string untuk number/boolean
  if (kind === "number") {
    if (typeof newVal !== "number" || !Number.isFinite(newVal)) {
      return "Nilai harus berupa angka yang valid";
    }
    return null;
  }

  if (kind === "boolean") {
    if (typeof newVal !== "boolean") {
      return "Nilai harus berupa boolean (true / false)";
    }
    return null;
  }

  if (kind === "text") {
    if (typeof newVal !== "string") {
      return "Nilai harus berupa teks";
    }
    if (newVal.length > 200) {
      return "Nilai teks maksimal 200 karakter";
    }
    return null;
  }

  // Jika kind tidak ditentukan tapi ada nilai asli, pertahankan tipe nilai asli
  if (existingVal !== undefined && existingVal !== null) {
    if (typeof existingVal === "number") {
      if (typeof newVal !== "number" || !Number.isFinite(newVal)) {
        return "Tipe nilai harus berupa angka sesuai data aslinya";
      }
    } else if (typeof existingVal === "boolean") {
      if (typeof newVal !== "boolean") {
        return "Tipe nilai harus berupa boolean sesuai data aslinya";
      }
    } else if (typeof existingVal === "string") {
      if (typeof newVal !== "string") {
        return "Tipe nilai harus berupa teks sesuai data aslinya";
      }
      if (newVal.length > 200) {
        return "Nilai teks maksimal 200 karakter";
      }
    }
  }

  return null;
}
