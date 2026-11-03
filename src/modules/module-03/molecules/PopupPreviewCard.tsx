import React from "react";
import { Icon } from "@blueprintjs/core";
import type { SpatialResult } from "../types/map";
import type { FieldDef } from "../types/spatialResult";
import { formatFieldValue, resolveFeaturePhoto } from "../lib/fieldResolution";
import { isValidHexColor } from "../lib/fieldValidation";

export interface PopupPreviewCardProps {
  layerName: string;
  displayColor: string;
  layerImageUrl?: string | null;
  feature: SpatialResult | null;
  fields: FieldDef[];
  draftValues?: Record<string, unknown>;
}

export const PopupPreviewCard: React.FC<PopupPreviewCardProps> = ({
  layerName,
  displayColor,
  layerImageUrl,
  feature,
  fields,
  draftValues = {},
}) => {
  const safeColor = isValidHexColor(displayColor) ? displayColor : "#34c759";
  const props: Record<string, unknown> = {
    ...(feature?.properties || {}),
    ...draftValues,
  };

  const rawImg =
    (props.image_url as string | undefined) ??
    feature?.image_url ??
    (props.photo_url as string | undefined) ??
    layerImageUrl;

  const resolvedPhoto = resolveFeaturePhoto(rawImg, layerImageUrl);
  const visibleFields = fields.filter((f) => f.visible !== false);

  return (
    <div
      style={{
        background: "#ffffff",
        borderRadius: "16px",
        padding: "16px 18px",
        boxShadow: "0 10px 30px rgba(0, 0, 0, 0.08)",
        border: "1px solid #e2e8f0",
        borderLeft: `5px solid ${safeColor}`,
        fontFamily: "var(--font-sf-pro-text, -apple-system, BlinkMacSystemFont, sans-serif)",
        position: "relative",
      }}
    >
      {/* Header */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: "8px",
        }}
      >
        <span
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: "5px",
            background: "rgba(52, 199, 89, 0.12)",
            color: "#166534",
            fontSize: "10px",
            fontWeight: 700,
            padding: "3px 9px",
            borderRadius: "999px",
            textTransform: "uppercase",
            letterSpacing: "0.6px",
          }}
        >
          <span
            style={{
              width: "6px",
              height: "6px",
              borderRadius: "50%",
              background: safeColor,
            }}
          />
          Pratinjau Popup Peta
        </span>
        <span style={{ fontSize: "11px", color: "#94a3b8", fontWeight: 500 }}>
          {feature?.source || "manual"}
        </span>
      </div>

      <div
        style={{
          fontSize: "15px",
          fontWeight: 700,
          color: "var(--color-ink, #0f172a)",
          marginBottom: "10px",
          letterSpacing: "-0.2px",
          lineHeight: 1.3,
        }}
      >
        {layerName || "Nama Layer"}
      </div>

      {/* Foto Pratinjau */}
      {resolvedPhoto ? (
        <div
          style={{
            position: "relative",
            marginBottom: "12px",
            borderRadius: "12px",
            overflow: "hidden",
            background: "#0f172a",
            boxShadow: "0 4px 12px rgba(0, 0, 0, 0.1)",
            border: "1px solid rgba(0, 0, 0, 0.08)",
          }}
        >
          <img
            src={resolvedPhoto}
            alt="Pratinjau"
            style={{
              width: "100%",
              height: "140px",
              objectFit: "cover",
              display: "block",
            }}
            onError={(e) => {
              (e.currentTarget.parentElement as HTMLElement).style.display = "none";
            }}
          />
          <div
            style={{
              position: "absolute",
              bottom: "6px",
              right: "6px",
              background: "rgba(15, 23, 42, 0.75)",
              backdropFilter: "blur(4px)",
              color: "#ffffff",
              fontSize: "9px",
              fontWeight: 600,
              padding: "2px 7px",
              borderRadius: "4px",
              textTransform: "uppercase",
              letterSpacing: "0.5px",
            }}
          >
            Foto Udara
          </div>
        </div>
      ) : (
        <div
          style={{
            position: "relative",
            marginBottom: "12px",
            borderRadius: "12px",
            overflow: "hidden",
            background: "rgba(143, 153, 168, 0.08)",
            border: "1px dashed rgba(143, 153, 168, 0.3)",
            height: "64px",
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            color: "#8a9ba8",
            fontSize: "11px",
            fontWeight: 500,
          }}
        >
          <Icon icon="media" size={18} style={{ marginBottom: "2px", opacity: 0.6 }} />
          <span>No Image</span>
        </div>
      )}

      <div style={{ marginBottom: "8px", borderBottom: "1px solid #e2e8f0" }} />

      {/* Tabel Field Properti */}
      <table style={{ width: "100%", borderCollapse: "collapse" }}>
        <tbody>
          {visibleFields.length === 0 ? (
            <tr>
              <td
                colSpan={2}
                style={{
                  textAlign: "center",
                  padding: "10px 0",
                  fontSize: "12px",
                  color: "#94a3b8",
                  fontStyle: "italic",
                }}
              >
                Tidak ada field yang ditampilkan
              </td>
            </tr>
          ) : (
            visibleFields.map((f) => {
              const val = props[f.key];
              const displayVal = formatFieldValue(val, f.unit, f.kind);
              return (
                <tr key={f.key} style={{ borderBottom: "1px solid rgba(0, 0, 0, 0.05)" }}>
                  <td
                    style={{
                      color: "var(--color-slate, #64748b)",
                      fontWeight: 600,
                      padding: "6px 0",
                      fontSize: "12px",
                    }}
                  >
                    {f.label}
                  </td>
                  <td
                    style={{
                      color: "var(--color-ink, #0f172a)",
                      fontWeight: 600,
                      textAlign: "right",
                      fontFamily: "var(--font-sf-pro-text, sans-serif)",
                      padding: "6px 0",
                      fontSize: "13px",
                    }}
                  >
                    {displayVal}
                  </td>
                </tr>
              );
            })
          )}
        </tbody>
      </table>
    </div>
  );
};
