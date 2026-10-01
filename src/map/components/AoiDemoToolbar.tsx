import React, { useState } from "react";
import { useMap } from "react-leaflet";
import {
  Button,
  ButtonGroup,
  Callout,
  Card,
  FormGroup,
  InputGroup,
  Spinner,
} from "@blueprintjs/core";
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

export const AoiDemoToolbar: React.FC = () => {
  const map = useMap();
  const { aoi, draftPolygon, isDrawing, isSaving, startDrawing, stopDrawing, clearAoi, saveAoi } =
    useAOI();

  const [aoiId, setAoiId] = useState(generateAoiId);
  const [aoiName, setAoiName] = useState("Area Demo");
  const [error, setError] = useState<string | null>(null);

  // ── Handlers ─────────────────────────────────────────────────────────

  const handleDraw = () => {
    setError(null);
    startDrawing(map);
  };

  const handleCancel = () => {
    stopDrawing(map);
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
    <div
      style={{
        position: "absolute",
        top: 10,
        left: 10,
        zIndex: 1000,
        background: "white",
        padding: 12,
        borderRadius: 4,
        boxShadow: "0 2px 8px rgba(0,0,0,0.15)",
        minWidth: 280,
        color: "#182026",
      }}
    >
      <FormGroup label="AOI ID" labelInfo="(wajib)" style={{ marginBottom: 8 }}>
        <InputGroup
          value={aoiId}
          onChange={(e) => setAoiId(e.target.value)}
          placeholder="aoi-001"
          disabled={isSaving || aoi !== null}
        />
      </FormGroup>

      <FormGroup label="Nama Area" labelInfo="(wajib)" style={{ marginBottom: 8 }}>
        <InputGroup
          value={aoiName}
          onChange={(e) => setAoiName(e.target.value)}
          placeholder="Lahan Sawit Blok A"
          disabled={isSaving || aoi !== null}
        />
      </FormGroup>

      <div style={{ borderTop: "1px solid #e1e8ed", paddingTop: 8, marginBottom: 8 }}>
        <ButtonGroup>
          <Button
            icon="edit"
            text="Draw AOI"
            intent="primary"
            onClick={handleDraw}
            disabled={isDrawing || aoi !== null}
          />
          <Button icon="cross" text="Batal" onClick={handleCancel} disabled={isCancelDisabled} />
          <Button
            icon="floppy-disk"
            text="Simpan ke Rust"
            intent="success"
            onClick={() => void handleSave()}
            disabled={isSaveDisabled}
          />
        </ButtonGroup>
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
    </div>
  );
};
