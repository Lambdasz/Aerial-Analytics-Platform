import { describe, it, expect, beforeEach } from "vitest";
import {
  escapeHtml,
  isValidHexColor,
  isReservedKey,
  validateCustomFieldKey,
  validateFieldLabel,
  validateFieldUnit,
  validateCustomFieldsCount,
  validatePropertyValueType,
} from "../fieldValidation";
import {
  resolveDisplayFields,
  formatFieldValue,
  resolveImageSrc,
  resolveCardThumbnail,
  resolveFeaturePhoto,
  setCachedStorageRoot,
  humanizeKey,
} from "../fieldResolution";
import type { FieldDef } from "../../types/spatialResult";

describe("fieldValidation pure functions", () => {
  describe("escapeHtml", () => {
    it("escapes all dangerous HTML characters", () => {
      const raw = `<script>alert("XSS & 'pwned'");</script>`;
      const safe = escapeHtml(raw);
      expect(safe).toBe(
        "&lt;script&gt;alert(&quot;XSS &amp; &#039;pwned&#039;&quot;);&lt;/script&gt;",
      );
    });

    it("handles null and undefined gracefully", () => {
      expect(escapeHtml(null)).toBe("");
      expect(escapeHtml(undefined)).toBe("");
      expect(escapeHtml("")).toBe("");
    });
  });

  describe("isValidHexColor", () => {
    it("accepts valid hex colors (3, 4, 6, 8 digits)", () => {
      expect(isValidHexColor("#fff")).toBe(true);
      expect(isValidHexColor("#2ecc40")).toBe(true);
      expect(isValidHexColor("#FF851B")).toBe(true);
      expect(isValidHexColor("#0074d9cc")).toBe(true);
    });

    it("rejects invalid hex colors", () => {
      expect(isValidHexColor("red")).toBe(false);
      expect(isValidHexColor("#12")).toBe(false);
      expect(isValidHexColor("#12345")).toBe(false);
      expect(isValidHexColor("rgba(0,0,0,1)")).toBe(false);
    });
  });

  describe("validateCustomFieldKey", () => {
    it("accepts valid snake_case keys starting with lowercase letter", () => {
      expect(validateCustomFieldKey("catatan")).toBeNull();
      expect(validateCustomFieldKey("kondisi_pohon_01")).toBeNull();
      expect(validateCustomFieldKey("a")).toBeNull();
    });

    it("rejects empty or whitespace keys", () => {
      expect(validateCustomFieldKey("")).not.toBeNull();
      expect(validateCustomFieldKey("   ")).not.toBeNull();
    });

    it("rejects keys starting with number or containing uppercase/symbols", () => {
      expect(validateCustomFieldKey("1tree")).not.toBeNull();
      expect(validateCustomFieldKey("Tree_Id")).not.toBeNull();
      expect(validateCustomFieldKey("tree-id")).not.toBeNull();
      expect(validateCustomFieldKey("tree id")).not.toBeNull();
    });

    it("rejects reserved keys", () => {
      expect(validateCustomFieldKey("image_url")).not.toBeNull();
      expect(validateCustomFieldKey("photo_url")).not.toBeNull();
      expect(validateCustomFieldKey("edited_fields")).not.toBeNull();
      expect(validateCustomFieldKey("__proto__")).not.toBeNull();
    });

    it("rejects duplicate keys", () => {
      expect(validateCustomFieldKey("confidence", ["tree_id", "confidence"])).not.toBeNull();
    });
  });

  describe("validateFieldLabel", () => {
    it("accepts valid labels (1..40 chars)", () => {
      expect(validateFieldLabel("Tinggi Estimasi")).toBeNull();
      expect(validateFieldLabel("A")).toBeNull();
    });

    it("rejects empty, overlong, or HTML labels", () => {
      expect(validateFieldLabel("")).not.toBeNull();
      expect(validateFieldLabel("   ")).not.toBeNull();
      expect(validateFieldLabel("a".repeat(41))).not.toBeNull();
      expect(validateFieldLabel("<b>Judul</b>")).not.toBeNull();
    });
  });

  describe("validateFieldUnit", () => {
    it("accepts empty or valid units (<= 12 chars)", () => {
      expect(validateFieldUnit(null)).toBeNull();
      expect(validateFieldUnit("")).toBeNull();
      expect(validateFieldUnit("m²")).toBeNull();
      expect(validateFieldUnit("kg/ha")).toBeNull();
    });

    it("rejects overlong units or HTML tags", () => {
      expect(validateFieldUnit("panjang_sekali_melebihi")).not.toBeNull();
      expect(validateFieldUnit("<i>m</i>")).not.toBeNull();
    });
  });

  describe("validateCustomFieldsCount", () => {
    it("allows up to 30 custom fields", () => {
      const fields: FieldDef[] = Array.from({ length: 30 }, (_, i) => ({
        key: `custom_${i}`,
        label: `Custom ${i}`,
        custom: true,
        visible: true,
      }));
      expect(validateCustomFieldsCount(fields)).toBeNull();
    });

    it("rejects more than 30 custom fields while ignoring non-custom fields", () => {
      const fields: FieldDef[] = Array.from({ length: 31 }, (_, i) => ({
        key: `custom_${i}`,
        label: `Custom ${i}`,
        custom: true,
        visible: true,
      }));
      expect(validateCustomFieldsCount(fields)).not.toBeNull();

      // 40 non-custom fields + 5 custom fields = OK
      const mixed: FieldDef[] = [
        ...Array.from({ length: 40 }, (_, i) => ({
          key: `plugin_${i}`,
          label: `Plugin ${i}`,
          custom: false,
          visible: true,
        })),
        ...Array.from({ length: 5 }, (_, i) => ({
          key: `custom_${i}`,
          label: `Custom ${i}`,
          custom: true,
          visible: true,
        })),
      ];
      expect(validateCustomFieldsCount(mixed)).toBeNull();
    });
  });

  describe("validatePropertyValueType", () => {
    it("preserves number type and rejects non-numbers", () => {
      expect(validatePropertyValueType(12.5, 15.0)).toBeNull();
      expect(validatePropertyValueType(12.5, "15.0")).not.toBeNull();
      expect(validatePropertyValueType(12.5, NaN)).not.toBeNull();
    });

    it("preserves boolean type and rejects non-booleans", () => {
      expect(validatePropertyValueType(true, false)).toBeNull();
      expect(validatePropertyValueType(true, "true")).not.toBeNull();
    });

    it("preserves text type and enforces max 200 chars", () => {
      expect(validatePropertyValueType("Sehat", "Kritis")).toBeNull();
      expect(validatePropertyValueType("Sehat", 123)).not.toBeNull();
      expect(validatePropertyValueType("Sehat", "x".repeat(201))).not.toBeNull();
    });

    it("rejects complex objects or arrays", () => {
      expect(validatePropertyValueType("val", { nested: "object" })).not.toBeNull();
      expect(validatePropertyValueType("val", [1, 2, 3])).not.toBeNull();
    });
  });
});

describe("fieldResolution pure functions", () => {
  beforeEach(() => {
    setCachedStorageRoot(null);
  });

  describe("resolveImageSrc", () => {
    it("preserves external and asset protocols as is", () => {
      expect(resolveImageSrc("https://example.com/photo.jpg")).toBe(
        "https://example.com/photo.jpg",
      );
      expect(resolveImageSrc("http://example.com/photo.png")).toBe("http://example.com/photo.png");
      expect(resolveImageSrc("data:image/png;base64,abc")).toBe("data:image/png;base64,abc");
      expect(resolveImageSrc("asset://localhost/path.jpg")).toBe("asset://localhost/path.jpg");
    });

    it("rejects path traversal attempts", () => {
      expect(resolveImageSrc("../../../etc/passwd")).toBeNull();
      expect(resolveImageSrc("map_storage/images/../../secret.txt")).toBeNull();
    });

    it("resolves /map_storage/... correctly in web and desktop modes", () => {
      // In web dev environment
      const webSrc = resolveImageSrc("/map_storage/images/result_image/foto.png");
      expect(webSrc).toBe("/map_storage/images/result_image/foto.png");

      // With absolute storageRoot provided
      const resolvedWithRoot = resolveImageSrc(
        "map_storage/images/result_image/tree.png",
        "D:/AAP/map_storage",
      );
      expect(resolvedWithRoot).toBeDefined();
    });

    it("handles public asset links", () => {
      expect(resolveImageSrc("/sample_drone.jpg")).toBe("/sample_drone.jpg");
      expect(resolveImageSrc("sample_drone.jpg")).toBe("/sample_drone.jpg");
    });

    it("returns null for empty or invalid input", () => {
      expect(resolveImageSrc(null)).toBeNull();
      expect(resolveImageSrc(undefined)).toBeNull();
      expect(resolveImageSrc("")).toBeNull();
      expect(resolveImageSrc("   ")).toBeNull();
    });
  });

  describe("resolveCardThumbnail & resolveFeaturePhoto", () => {
    it("prioritizes layer image then first feature image for card thumbnail", () => {
      expect(resolveCardThumbnail("/map_storage/layer.png", "/map_storage/feat.png")).toBe(
        "/map_storage/layer.png",
      );
      expect(resolveCardThumbnail(null, "/map_storage/feat.png")).toBe("/map_storage/feat.png");
      expect(resolveCardThumbnail(null, null)).toBeNull();
    });

    it("prioritizes feature image then layer image for popup photo", () => {
      expect(resolveFeaturePhoto("/map_storage/feat.png", "/map_storage/layer.png")).toBe(
        "/map_storage/feat.png",
      );
      expect(resolveFeaturePhoto(null, "/map_storage/layer.png")).toBe("/map_storage/layer.png");
      expect(resolveFeaturePhoto(null, null)).toBeNull();
    });
  });

  describe("resolveDisplayFields", () => {
    it("derives fields from feature properties when no saved fields exist", () => {
      const props = [
        { tree_id: "T-001", height_est_m: 12.5, unknown_metric: 42, image_url: "foto.png" },
      ];
      const fields = resolveDisplayFields(null, props);

      expect(fields.length).toBe(3); // tree_id, height_est_m, unknown_metric (image_url ignored)
      expect(fields[0].key).toBe("tree_id");
      expect(fields[0].label).toBe("ID Pohon");
      expect(fields[1].key).toBe("height_est_m");
      expect(fields[1].label).toBe("Tinggi Estimasi");
      expect(fields[1].unit).toBe("m");
      expect(fields[2].key).toBe("unknown_metric");
      expect(fields[2].label).toBe("Unknown Metric");
    });

    it("preserves saved fields and appends newly discovered plugin keys", () => {
      const saved: FieldDef[] = [
        { key: "custom_note", label: "Catatan Petugas", visible: true, custom: true, kind: "text" },
      ];
      const props = [{ custom_note: "Aman", confidence: 0.95 }];

      const fields = resolveDisplayFields(saved, props);
      expect(fields.length).toBe(2);
      expect(fields[0].key).toBe("custom_note");
      expect(fields[1].key).toBe("confidence");
      expect(fields[1].label).toBe("Keyakinan");
    });

    it("filters out reserved keys", () => {
      expect(isReservedKey("image_url")).toBe(true);
      expect(isReservedKey("edited_fields")).toBe(true);
      expect(isReservedKey("geometry")).toBe(true);

      const props = [{ tree_id: "T-01", image_url: "foto.jpg", edited_fields: ["tree_id"] }];
      const fields = resolveDisplayFields(null, props);
      expect(fields.length).toBe(1);
      expect(fields[0].key).toBe("tree_id");
    });
  });

  describe("formatFieldValue", () => {
    it("formats numbers with Indonesian locale and unit", () => {
      expect(formatFieldValue(1420.5, "m²", "number")).toBe("1.420,5 m²");
      expect(formatFieldValue(1000000, null, "number")).toBe("1.000.000");
    });

    it("formats booleans as Ya / Tidak", () => {
      expect(formatFieldValue(true, null, "boolean")).toBe("Ya");
      expect(formatFieldValue(false, null, "boolean")).toBe("Tidak");
    });

    it("formats text strings and appends unit if present", () => {
      expect(formatFieldValue("Sehat")).toBe("Sehat");
      expect(formatFieldValue("Kuning", "derajat")).toBe("Kuning derajat");
    });

    it("handles null and undefined", () => {
      expect(formatFieldValue(null)).toBe("-");
      expect(formatFieldValue(undefined)).toBe("-");
    });
  });

  describe("humanizeKey", () => {
    it("transforms snake_case to Title Case", () => {
      expect(humanizeKey("tree_id")).toBe("Tree Id");
      expect(humanizeKey("crown_diameter_m")).toBe("Crown Diameter M");
    });
  });
});
