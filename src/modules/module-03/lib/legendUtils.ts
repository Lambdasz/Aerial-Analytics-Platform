import type { LayerPayload } from "../types/map";

/**
 * Mengelompokkan layer hasil spasial berdasarkan sektor nama (format: "Nama - Sektor").
 *
 * Fungsi murni deterministik. Hanya menyertakan layer non-image_overlay.
 */
export function groupLayersBySector(layers: LayerPayload[]): Record<string, LayerPayload[]> {
  const legendItems = layers.filter((layer) => layer.display_preference.type !== "image_overlay");
  const groups: Record<string, typeof legendItems> = {};

  legendItems.forEach((layer) => {
    const parts = layer.layer_name.split(" - ");
    const sector = parts.length > 1 ? parts[1] : "Umum";
    const name = parts[0];
    if (!groups[sector]) groups[sector] = [];
    groups[sector].push({ ...layer, layer_name: name });
  });

  return groups;
}
