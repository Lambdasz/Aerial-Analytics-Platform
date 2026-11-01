import React, { useState } from "react";
import { Button, ButtonGroup } from "@blueprintjs/core";
import type { SpatialResult } from "../types/map";
import type { FieldDef } from "../types/spatialResult";
import { validatePropertyValueType } from "../lib/fieldValidation";
import { getFeatureDisplayName } from "../lib/fieldResolution";

export interface FeatureValuesEditorProps {
  features: SpatialResult[];
  fields: FieldDef[];
  selectedIndex: number;
  onSelectIndex: (index: number) => void;
  draftSets: Map<number, Record<string, unknown>>;
  onUpdateValue: (featureIndex: number, key: string, value: unknown) => void;
}

export const FeatureValuesEditor: React.FC<FeatureValuesEditorProps> = ({
  features,
  fields,
  selectedIndex,
  onSelectIndex,
  draftSets,
  onUpdateValue,
}) => {
  const [page, setPage] = useState(0);
  const PAGE_SIZE = 20;
  const isPaginated = features.length > 50;

  const totalPages = Math.ceil(features.length / PAGE_SIZE);
  const displayedFeatures = isPaginated
    ? features.slice(page * PAGE_SIZE, (page + 1) * PAGE_SIZE)
    : features;

  const currentFeature = features[selectedIndex];
  const currentDraft = draftSets.get(selectedIndex) || {};
  const currentProperties = {
    ...(currentFeature?.properties || {}),
    ...currentDraft,
  };

  const editedFields = new Set(currentFeature?.edited_fields || []);
  Object.keys(currentDraft).forEach((k) => editedFields.add(k));

  if (!currentFeature) {
    return (
      <div style={{ color: "#64748b", fontStyle: "italic", fontSize: "12px", padding: "10px" }}>
        Tidak ada data fitur untuk diedit.
      </div>
    );
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
      {/* Header Selector Fitur */}
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          flexWrap: "wrap",
          gap: "8px",
        }}
      >
        <div style={{ minWidth: "180px", flex: 1 }}>
          <label
            htmlFor="feature-picker"
            style={{
              fontSize: "11px",
              fontWeight: 600,
              color: "#475569",
              display: "block",
              marginBottom: 2,
            }}
          >
            Pilih Fitur yang Diedit ({features.length} total)
          </label>
          <select
            id="feature-picker"
            value={selectedIndex}
            onChange={(e) => onSelectIndex(Number(e.target.value))}
            style={{
              width: "100%",
              padding: "9px 12px",
              fontSize: "13px",
              borderRadius: "10px",
              border: "none",
              background: "#f8fafc",
              color: "#0f172a",
              outline: "none",
              cursor: "pointer",
              marginTop: "4px",
            }}
          >
            {displayedFeatures.map((f, localIdx) => {
              const realIdx = isPaginated ? page * PAGE_SIZE + localIdx : localIdx;
              const draft = draftSets.get(realIdx);
              const label = getFeatureDisplayName(f, realIdx, draft);
              const isEdited =
                (f.edited_fields && f.edited_fields.length > 0) ||
                (draft && Object.keys(draft).length > 0);
              return (
                <option key={realIdx} value={realIdx}>
                  {label} {isEdited ? "•" : ""}
                </option>
              );
            })}
          </select>
        </div>

        {/* Paginasi jika fitur > 50 */}
        {isPaginated && (
          <div style={{ display: "flex", alignItems: "center", gap: "4px" }}>
            <ButtonGroup minimal>
              <Button
                icon="chevron-left"
                small
                disabled={page === 0}
                onClick={() => setPage((p) => Math.max(0, p - 1))}
              />
              <span style={{ fontSize: "11px", color: "#64748b", padding: "0 4px" }}>
                Hal {page + 1} / {totalPages}
              </span>
              <Button
                icon="chevron-right"
                small
                disabled={page >= totalPages - 1}
                onClick={() => setPage((p) => Math.min(totalPages - 1, p + 1))}
              />
            </ButtonGroup>
          </div>
        )}
      </div>

      {/* Editor Nilai Properti Per Field */}
      <div
        style={{
          maxHeight: "280px",
          overflowY: "auto",
          display: "flex",
          flexDirection: "column",
          gap: "12px",
          paddingRight: "4px",
        }}
      >
        {fields.map((f) => {
          const originalVal = currentFeature.properties?.[f.key];
          const currentVal = currentProperties[f.key];
          const isFieldEdited = editedFields.has(f.key);
          const valError = validatePropertyValueType(originalVal, currentVal, f.kind);

          return (
            <div key={f.key}>
              <div
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  alignItems: "center",
                  marginBottom: "4px",
                }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                  <span style={{ fontWeight: 600, fontSize: "12px", color: "#475569" }}>
                    {f.label}
                  </span>
                  {f.unit && <span style={{ color: "#64748b", fontSize: "11px" }}>({f.unit})</span>}
                  {isFieldEdited && (
                    <span
                      style={{
                        fontSize: "9px",
                        padding: "1px 6px",
                        borderRadius: "4px",
                        background: "#f1f5f9",
                        color: "#475569",
                        fontWeight: 600,
                      }}
                    >
                      diedit manual
                    </span>
                  )}
                  {!f.visible && (
                    <span
                      style={{
                        fontSize: "9px",
                        padding: "1px 6px",
                        borderRadius: "4px",
                        background: "#f1f5f9",
                        color: "#94a3b8",
                        fontWeight: 600,
                      }}
                    >
                      tersembunyi
                    </span>
                  )}
                </div>
                <span
                  style={{
                    fontSize: "10px",
                    color: "#94a3b8",
                    fontFamily: "monospace",
                  }}
                >
                  key: <code>{f.key}</code>
                </span>
              </div>

              {f.kind === "boolean" ? (
                <select
                  id={`prop-input-${f.key}`}
                  value={currentVal === true ? "true" : currentVal === false ? "false" : ""}
                  onChange={(e) => {
                    const v = e.target.value;
                    onUpdateValue(
                      selectedIndex,
                      f.key,
                      v === "true" ? true : v === "false" ? false : null,
                    );
                  }}
                  style={{
                    width: "100%",
                    padding: "9px 12px",
                    fontSize: "13px",
                    borderRadius: "10px",
                    border: "none",
                    background: "#f8fafc",
                    color: "#0f172a",
                    outline: "none",
                    cursor: "pointer",
                    boxSizing: "border-box",
                  }}
                >
                  <option value="">(Belum Diatur)</option>
                  <option value="true">Ya (True)</option>
                  <option value="false">Tidak (False)</option>
                </select>
              ) : f.kind === "number" ? (
                <input
                  id={`prop-input-${f.key}`}
                  type="number"
                  step="any"
                  value={
                    typeof currentVal === "number" || typeof currentVal === "string"
                      ? currentVal
                      : ""
                  }
                  onChange={(e) => {
                    const raw = e.target.value;
                    if (raw.trim() === "") {
                      onUpdateValue(selectedIndex, f.key, null);
                    } else {
                      const num = Number(raw);
                      onUpdateValue(selectedIndex, f.key, Number.isFinite(num) ? num : raw);
                    }
                  }}
                  style={{
                    width: "100%",
                    padding: "9px 12px",
                    fontSize: "13px",
                    background: "#f8fafc",
                    border: valError ? "1px solid #ef4444" : "none",
                    borderRadius: "10px",
                    color: "#0f172a",
                    outline: "none",
                    boxSizing: "border-box",
                  }}
                />
              ) : (
                <input
                  id={`prop-input-${f.key}`}
                  type="text"
                  value={
                    typeof currentVal === "string"
                      ? currentVal
                      : typeof currentVal === "number" || typeof currentVal === "boolean"
                        ? currentVal.toString()
                        : ""
                  }
                  onChange={(e) => {
                    onUpdateValue(selectedIndex, f.key, e.target.value);
                  }}
                  placeholder={`Isi ${f.label.toLowerCase()}...`}
                  style={{
                    width: "100%",
                    padding: "9px 12px",
                    fontSize: "13px",
                    background: "#f8fafc",
                    border: valError ? "1px solid #ef4444" : "none",
                    borderRadius: "10px",
                    color: "#0f172a",
                    outline: "none",
                    boxSizing: "border-box",
                  }}
                />
              )}

              {valError && (
                <div style={{ fontSize: "11px", color: "#dc2626", marginTop: "3px" }}>
                  {valError}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
};
