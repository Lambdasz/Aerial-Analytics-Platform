import { convertFileSrc, isTauri } from "@tauri-apps/api/core";
import type { FieldDef, FieldKind } from "../types/spatialResult";
import { isReservedKey } from "./fieldValidation";

let cachedStorageRoot: string | null = null;

/**
 * Menyimpan path root storage dari backend untuk resolusi convertFileSrc di desktop.
 */
export function setCachedStorageRoot(root: string | null): void {
  cachedStorageRoot = root;
}

export function getCachedStorageRoot(): string | null {
  return cachedStorageRoot;
}

/**
 * Kamus standar mapping key teknis bawaan plugin ke label, unit, dan kind.
 */
export const STANDARD_FIELD_DICTIONARY: Record<
  string,
  { label: string; unit: string | null; kind: FieldKind }
> = {
  tree_id: { label: "ID Pohon", unit: null, kind: "text" },
  confidence: { label: "Keyakinan", unit: null, kind: "number" },
  height_est_m: { label: "Tinggi Estimasi", unit: "m", kind: "number" },
  crown_diameter_m: { label: "Diameter Tajuk", unit: "m", kind: "number" },
  health_status: { label: "Kondisi", unit: null, kind: "text" },
  detector_run_id: { label: "ID Proses Deteksi", unit: null, kind: "text" },
  land_cover_class: { label: "Kelas Tutupan Lahan", unit: null, kind: "text" },
  area_sqm: { label: "Luas", unit: "m²", kind: "number" },
  path_id: { label: "ID Jalur", unit: null, kind: "text" },
  path_type: { label: "Jenis Jalan", unit: null, kind: "text" },
  type: { label: "Jenis", unit: null, kind: "text" },
  length_m: { label: "Panjang", unit: "m", kind: "number" },
  surface_condition: { label: "Kondisi Permukaan", unit: null, kind: "text" },
  vegetation_index_type: { label: "Jenis Indeks Vegetasi", unit: null, kind: "text" },
  mean_greenness_score: { label: "Skor Kehijauan Rata-rata", unit: null, kind: "number" },
  plot_id: { label: "ID Petak", unit: null, kind: "text" },
  damage_level: { label: "Tingkat Kerusakan", unit: null, kind: "text" },
  damage_percent: { label: "Persentase Kerusakan", unit: "%", kind: "number" },
  affected_area_sqm: { label: "Luas Terdampak", unit: "m²", kind: "number" },
  inspected: { label: "Sudah Diperiksa", unit: null, kind: "boolean" },
  catatan: { label: "Catatan", unit: null, kind: "text" },
};

/**
 * Mengubah string snake_case menjadi label Title Case (misal: "mean_score" -> "Mean Score").
 */
export function humanizeKey(key: string): string {
  return key
    .replace(/_/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase())
    .trim();
}

/**
 * Menurunkan kind dari tipe nilai runtime.
 */
export function inferFieldKind(value: unknown): FieldKind {
  if (typeof value === "number") return "number";
  if (typeof value === "boolean") return "boolean";
  return "text";
}

/**
 * Menyelesaikan daftar definisi field yang aktif.
 *
 * Jika `fieldsDef` sudah tersimpan:
 * 1. Gunakan field yang ada.
 * 2. Tambahkan key baru bawaan plugin yang belum tercatat di akhir daftar.
 *
 * Jika `fieldsDef` belum ada:
 * 1. Telusuri seluruh properties dari semua fitur secara urut kemunculan pertama.
 * 2. Cocokkan dengan kamus standar atau buat fallback otomatis.
 */
export function resolveDisplayFields(
  savedFields: FieldDef[] | null | undefined,
  featuresProperties: Record<string, unknown>[] = [],
): FieldDef[] {
  // Kumpulkan seluruh key dari data aktual (abaikan reserved keys)
  const encounteredKeys: string[] = [];
  const keyTypeMap = new Map<string, FieldKind>();

  for (const props of featuresProperties) {
    if (!props || typeof props !== "object") continue;
    for (const [k, v] of Object.entries(props)) {
      if (isReservedKey(k)) continue;
      if (!encounteredKeys.includes(k)) {
        encounteredKeys.push(k);
        keyTypeMap.set(k, inferFieldKind(v));
      }
    }
  }

  if (savedFields && savedFields.length > 0) {
    const result: FieldDef[] = [...savedFields];
    const existingKeySet = new Set(result.map((f) => f.key.toLowerCase()));

    // Tambahkan key baru yang belum terdaftar di posisi akhir
    for (const key of encounteredKeys) {
      if (!existingKeySet.has(key.toLowerCase())) {
        const dict = STANDARD_FIELD_DICTIONARY[key.toLowerCase()];
        result.push({
          key,
          label: dict ? dict.label : humanizeKey(key),
          unit: dict ? dict.unit : null,
          visible: true,
          custom: false,
          kind: dict ? dict.kind : keyTypeMap.get(key) || "text",
        });
        existingKeySet.add(key.toLowerCase());
      }
    }
    return result;
  }

  // Jika belum ada fields tersimpan, bentuk dari seluruh kemunculan key
  return encounteredKeys.map((key) => {
    const dict = STANDARD_FIELD_DICTIONARY[key.toLowerCase()];
    return {
      key,
      label: dict ? dict.label : humanizeKey(key),
      unit: dict ? dict.unit : null,
      visible: true,
      custom: false,
      kind: dict ? dict.kind : keyTypeMap.get(key) || "text",
    };
  });
}

/**
 * Format nilai properti untuk tampilan popup atau tabel dengan unit dan locale ID.
 */
export function formatFieldValue(val: unknown, unit?: string | null, kind?: FieldKind): string {
  if (val === null || val === undefined) return "-";

  let formatted = "";
  if (typeof val === "boolean" || kind === "boolean") {
    formatted = val ? "Ya" : "Tidak";
  } else if (typeof val === "number" || kind === "number") {
    const num = Number(val);
    if (Number.isFinite(num)) {
      formatted = num.toLocaleString("id-ID", { maximumFractionDigits: 4 });
    } else if (typeof val === "number") {
      formatted = val.toString();
    } else {
      formatted = "";
    }
  } else if (typeof val === "string") {
    formatted = val.trim();
  } else {
    formatted = "";
  }

  if (unit && unit.trim()) {
    return `${formatted} ${unit.trim()}`;
  }
  return formatted;
}

/**
 * Menghasilkan nama representasi fitur untuk pemilih dropdown berdasarkan prioritas field.
 */
export function getFeatureDisplayName(
  feat: { properties?: Record<string, unknown> },
  index: number,
  drafts?: Record<string, unknown>,
): string {
  const props = { ...(feat.properties || {}), ...(drafts || {}) };
  if (props.tree_id && typeof props.tree_id === "string") {
    return `Pohon: ${props.tree_id}`;
  }
  if (props.path_id && typeof props.path_id === "string") {
    return `Jalur: ${props.path_id}`;
  }
  if (props.plot_id && typeof props.plot_id === "string") {
    return `Petak: ${props.plot_id}`;
  }
  if (props.land_cover_class && typeof props.land_cover_class === "string") {
    return `Tutupan: ${props.land_cover_class}`;
  }
  if (props.vegetation_index_type && typeof props.vegetation_index_type === "string") {
    return `Vegetasi: ${props.vegetation_index_type}`;
  }
  return `Fitur #${index + 1}`;
}

/**
 * Menstandarisasi URL/path citra untuk tampilan browser maupun Tauri desktop.
 * Mencegah path traversal ("..") dan menangani assetProtocol secara aman.
 */
export function resolveImageSrc(
  rawUrl: string | null | undefined,
  storageRoot?: string | null,
): string | null {
  if (!rawUrl) return null;
  const trimmed = rawUrl.trim().replace(/\\/g, "/");
  if (!trimmed) return null;

  // 1. URL Protokol Eksternal / Data URI / Asset URI dicek LEBIH DULU
  if (/^(https?:\/\/|data:|asset:)/i.test(trimmed)) {
    return trimmed;
  }

  // 2. Tolak keras path traversal ("..")
  if (trimmed.includes("..")) {
    console.warn("Peringatan Keamanan: Path traversal terdeteksi pada image URL:", trimmed);
    return null;
  }

  // 3. Tangani berkas penyimpanan /map_storage/...
  const storageIdx = trimmed.indexOf("map_storage/");
  if (storageIdx !== -1) {
    const relAfterStorage = trimmed.substring(storageIdx + "map_storage/".length);
    if (isTauri()) {
      const activeRoot = storageRoot || cachedStorageRoot;
      if (activeRoot) {
        const cleanRoot = activeRoot.replace(/\\/g, "/").replace(/\/$/, "");
        return convertFileSrc(`${cleanRoot}/${relAfterStorage}`);
      }
      return convertFileSrc(trimmed);
    }
    return `/${trimmed.substring(storageIdx)}`;
  }

  // 4. Aset publik lokal (/sample_drone.*)
  if (trimmed.startsWith("/")) {
    return trimmed;
  }
  return `/${trimmed}`;
}

/**
 * Resolusi thumbnail untuk kartu sidebar:
 * Prioritas 1: Foto layer (display_preference.image_url)
 * Prioritas 2: Foto fitur pertama (spatial_results[0].image_url atau properties.image_url)
 * Prioritas 3: null (Placeholder No Image)
 */
export function resolveCardThumbnail(
  layerImageUrl: string | null | undefined,
  firstFeatureImageUrl: string | null | undefined,
  storageRoot?: string | null,
): string | null {
  if (layerImageUrl && layerImageUrl.trim()) {
    return resolveImageSrc(layerImageUrl, storageRoot);
  }
  if (firstFeatureImageUrl && firstFeatureImageUrl.trim()) {
    return resolveImageSrc(firstFeatureImageUrl, storageRoot);
  }
  return null;
}

/**
 * Resolusi foto untuk popup fitur peta:
 * Prioritas 1: Foto spesifik fitur tersebut (feature.image_url atau properties.image_url)
 * Prioritas 2: Foto layer (display_preference.image_url)
 * Prioritas 3: null (Placeholder No Image)
 */
export function resolveFeaturePhoto(
  featureImageUrl: string | null | undefined,
  layerImageUrl: string | null | undefined,
  storageRoot?: string | null,
): string | null {
  if (featureImageUrl && featureImageUrl.trim()) {
    return resolveImageSrc(featureImageUrl, storageRoot);
  }
  if (layerImageUrl && layerImageUrl.trim()) {
    return resolveImageSrc(layerImageUrl, storageRoot);
  }
  return null;
}
