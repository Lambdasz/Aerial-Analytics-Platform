import React, { useState } from "react";
import {
  Button,
  ButtonGroup,
  FormGroup,
  HTMLSelect,
  InputGroup,
  NumericInput,
  Tag,
} from "@blueprintjs/core";
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
          <HTMLSelect
            id="feature-picker"
            value={selectedIndex}
            onChange={(e) => onSelectIndex(Number(e.target.value))}
            fill
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
          </HTMLSelect>
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
          border: "1px solid #e2e8f0",
          borderRadius: "6px",
          padding: "10px 14px",
          background: "#ffffff",
          display: "flex",
          flexDirection: "column",
          gap: "10px",
        }}
      >
        {fields.map((f) => {
          const originalVal = currentFeature.properties?.[f.key];
          const currentVal = currentProperties[f.key];
          const isFieldEdited = editedFields.has(f.key);
          const valError = validatePropertyValueType(originalVal, currentVal, f.kind);

          return (
            <FormGroup
              key={f.key}
              label={
                <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                  <span style={{ fontWeight: 600 }}>{f.label}</span>
                  {f.unit && <span style={{ color: "#64748b", fontSize: "11px" }}>({f.unit})</span>}
                  {isFieldEdited && (
                    <Tag intent="warning" minimal style={{ fontSize: "9px", padding: "1px 4px" }}>
                      diedit manual
                    </Tag>
                  )}
                  {!f.visible && (
                    <Tag minimal style={{ fontSize: "9px", padding: "1px 4px", color: "#94a3b8" }}>
                      tersembunyi
                    </Tag>
                  )}
                </div>
              }
              labelFor={`prop-input-${f.key}`}
              helperText={
                valError ? (
                  <span style={{ color: "#dc2626" }}>{valError}</span>
                ) : (
                  <span style={{ color: "#94a3b8", fontSize: "10px" }}>
                    key: <code>{f.key}</code>
                  </span>
                )
              }
              intent={valError ? "danger" : "none"}
              style={{ marginBottom: "6px" }}
            >
              {f.kind === "boolean" ? (
                <HTMLSelect
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
                  fill
                >
                  <option value="">(Belum Diatur)</option>
                  <option value="true">Ya (True)</option>
                  <option value="false">Tidak (False)</option>
                </HTMLSelect>
              ) : f.kind === "number" ? (
                <NumericInput
                  id={`prop-input-${f.key}`}
                  value={
                    typeof currentVal === "number"
                      ? currentVal
                      : typeof currentVal === "string"
                        ? currentVal
                        : ""
                  }
                  onValueChange={(numericVal) => {
                    onUpdateValue(
                      selectedIndex,
                      f.key,
                      Number.isFinite(numericVal) ? numericVal : null,
                    );
                  }}
                  fill
                  stepSize={0.1}
                  majorStepSize={1}
                />
              ) : (
                <InputGroup
                  id={`prop-input-${f.key}`}
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
                />
              )}
            </FormGroup>
          );
        })}
      </div>
    </div>
  );
};
