import React, { useState } from "react";

import { Button, Callout, Card, Spinner } from "@blueprintjs/core";
import { useAOI } from "../hooks/useAOI";

/**
 * Floating toolbar for drawing, validating, and saving an AOI polygon.
 *
 * Must be rendered **inside** a `<MapContainer>` so that `useMap()` can
 * resolve the Leaflet instance. Uses {@link useAOI} for all state and
 * backend communication — this component is purely presentational.
 */

function generateAoiId(): string {
  return "aoi-" + Date.now();
}

import * as L from "leaflet";

interface AoiDemoToolbarProps {
  map: L.Map | null;
}

export const AoiDemoToolbar: React.FC<AoiDemoToolbarProps> = ({ map }) => {
  const { aoi, draftPolygon, isDrawing, isSaving, startDrawing, stopDrawing, clearAoi, saveAoi } =
    useAOI();

  const [aoiId, setAoiId] = useState(generateAoiId);
  const [aoiName, setAoiName] = useState("Area Demo");
  const [error, setError] = useState<string | null>(null);

  // ── Handlers ─────────────────────────────────────────────────────────

  const handleDraw = () => {
    setError(null);
    if (map) startDrawing(map);
  };

  const handleCancel = () => {
    if (map) stopDrawing(map);
    clearAoi();
    setAoiId(generateAoiId());
    setAoiName("Area Demo");
    setError(null);
  };

  const handleSave = async () => {
    setError(null);
    try {
      await saveAoi(aoiId.trim(), aoiName.trim());
    } catch (err: unknown) {
      if (err instanceof Error) {
        setError(err.message);
      } else if (typeof err === "object" && err !== null && "message" in err) {
        setError(String(err.message));
      } else {
        setError("Terjadi kesalahan yang tidak diketahui");
      }
    }
  };

  // ── Derived state ────────────────────────────────────────────────────

  const isSaveDisabled =
    isSaving || !draftPolygon || aoi !== null || aoiId.trim() === "" || aoiName.trim() === "";

  const isCancelDisabled = !isDrawing && !draftPolygon && !aoi;

  // ── Render ───────────────────────────────────────────────────────────

  return (
    <Card
      elevation={0}
      style={{
        padding: "16px",
        background: "transparent",
        color: "#3c4043",
      }}
    >
      <div style={{ marginBottom: 12 }}>
        <div
          style={{
            fontSize: "12px",
            fontWeight: 600,
            color: "var(--color-slate)",
            marginBottom: "6px",
          }}
        >
          AOI ID <span style={{ fontWeight: 400 }}>(wajib)</span>
        </div>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            background: "var(--color-gallery-white)",
            boxShadow: "var(--shadow-subtle)",
            borderRadius: "12px",
            padding: "8px 12px",
            opacity: isSaving || aoi !== null ? 0.6 : 1,
          }}
        >
          <input
            value={aoiId}
            onChange={(e) => setAoiId(e.target.value)}
            placeholder="aoi-001"
            disabled={isSaving || aoi !== null}
            style={{
              border: "none",
              outline: "none",
              background: "transparent",
              color: "var(--color-ink)",
              fontSize: "14px",
              width: "100%",
              fontFamily: "var(--font-sf-pro-text)",
            }}
          />
        </div>
      </div>

      <div style={{ marginBottom: 16 }}>
        <div
          style={{
            fontSize: "12px",
            fontWeight: 600,
            color: "var(--color-slate)",
            marginBottom: "6px",
          }}
        >
          Nama Area <span style={{ fontWeight: 400 }}>(wajib)</span>
        </div>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            background: "var(--color-gallery-white)",
            boxShadow: "var(--shadow-subtle)",
            borderRadius: "12px",
            padding: "8px 12px",
            opacity: isSaving || aoi !== null ? 0.6 : 1,
          }}
        >
          <input
            value={aoiName}
            onChange={(e) => setAoiName(e.target.value)}
            placeholder="Lahan Sawit Blok A"
            disabled={isSaving || aoi !== null}
            style={{
              border: "none",
              outline: "none",
              background: "transparent",
              color: "var(--color-ink)",
              fontSize: "14px",
              width: "100%",
              fontFamily: "var(--font-sf-pro-text)",
            }}
          />
        </div>
      </div>

      <div style={{ display: "flex", gap: "8px", paddingTop: "8px" }}>
        <Button
          icon="edit"
          text="Draw AOI"
          onClick={handleDraw}
          disabled={isDrawing || aoi !== null}
          style={{
            borderRadius: "999px",
            background: "var(--color-apple-blue)",
            color: "#fff",
            boxShadow: "none",
            fontWeight: 600,
            padding: "6px 16px",
            flex: 1,
          }}
        />
        <Button
          icon="cross"
          text="Batal"
          onClick={handleCancel}
          disabled={isCancelDisabled}
          style={{
            borderRadius: "999px",
            background: "var(--color-control-gray)",
            color: "var(--color-ink)",
            boxShadow: "none",
            fontWeight: 500,
            padding: "6px 16px",
          }}
        />
      </div>

      <div style={{ marginTop: "8px" }}>
        <Button
          icon="floppy-disk"
          text="Simpan ke Rust"
          fill
          onClick={() => void handleSave()}
          disabled={isSaveDisabled}
          style={{
            borderRadius: "999px",
            background: isSaveDisabled ? "var(--color-control-gray)" : "#34c759" /* Apple Green */,
            color: isSaveDisabled ? "var(--color-slate)" : "#fff",
            boxShadow: "none",
            fontWeight: 600,
            padding: "8px 16px",
          }}
        />
      </div>

      {isSaving && (
        <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 8 }}>
          <Spinner size={16} />
          <span>Menyimpan AOI...</span>
        </div>
      )}

      {error && (
        <Callout intent="danger" style={{ marginBottom: 8 }}>
          {error}
        </Callout>
      )}

      {aoi && (
        <Card style={{ marginTop: 8 }}>
          <h5 style={{ margin: "0 0 8px" }}>✅ AOI Tersimpan</h5>
          <p style={{ margin: "2px 0" }}>
            <strong>ID:</strong> {aoi.properties.aoi_id}
          </p>
          <p style={{ margin: "2px 0" }}>
            <strong>Nama:</strong> {aoi.properties.name}
          </p>
          <p style={{ margin: "2px 0" }}>
            <strong>Luas:</strong> {aoi.properties.calculated_area_sq_m.toFixed(2)} m²
          </p>
          <p style={{ margin: "2px 0" }}>
            <strong>Keliling:</strong> {aoi.properties.perimeter_m.toFixed(2)} m
          </p>
        </Card>
      )}
    </Card>
  );
};
