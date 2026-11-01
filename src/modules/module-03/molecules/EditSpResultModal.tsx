import React, { useState, useEffect, useRef } from "react";
import { Dialog, Button, Classes, Callout, Tag, Icon, Spinner } from "@blueprintjs/core";
import { open } from "@tauri-apps/plugin-dialog";
import { convertFileSrc, isTauri } from "@tauri-apps/api/core";
import type {
  SpatialResultSummary,
  UpdateSpatialResultPayload,
  FieldDef,
  FeatureEdit,
} from "../types/spatialResult";
import type { LayerPayload, SpatialResult } from "../types/map";
import { ResultPhoto } from "./ResultPhoto";
import { FieldManagerTable } from "./FieldManagerTable";
import { FeatureValuesEditor } from "./FeatureValuesEditor";
import { PopupPreviewCard } from "./PopupPreviewCard";
import { resolveDisplayFields } from "../lib/fieldResolution";
import { isValidHexColor, validateFieldLabel } from "../lib/fieldValidation";

export interface EditSpResultModalProps {
  isOpen: boolean;
  item: SpatialResultSummary | null;
  onClose: () => void;
  onSave: (payload: UpdateSpatialResultPayload) => Promise<void>;
  onLoadDetail?: (id: string) => Promise<LayerPayload>;
  isSubmitting?: boolean;
}

const COLOR_PRESETS = [
  "#2ecc40", // Hijau terang
  "#0074d9", // Biru
  "#ff851b", // Oranye
  "#e74c3c", // Merah
  "#b10dc9", // Ungu
  "#39cccc", // Teal
  "#ffdc00", // Kuning
];

const KNOWN_ICONS = [
  { value: "", label: "(Standar / Titik Bulat)" },
  { value: "tree_marker", label: "Pohon (Tree Marker)" },
];

interface EditSpResultFormProps {
  item: SpatialResultSummary;
  onClose: () => void;
  onSave: (payload: UpdateSpatialResultPayload) => Promise<void>;
  onLoadDetail?: (id: string) => Promise<LayerPayload>;
  isSubmitting?: boolean;
}

const EditSpResultForm: React.FC<EditSpResultFormProps> = ({
  item,
  onClose,
  onSave,
  onLoadDetail,
  isSubmitting = false,
}) => {
  const [selectedTab, setSelectedTab] = useState<"general" | "fields" | "values" | "preview">(
    "general",
  );
  const [isLoadingDetail, setIsLoadingDetail] = useState<boolean>(Boolean(onLoadDetail));
  const [layerDetail, setLayerDetail] = useState<LayerPayload | null>(null);

  // Tab Umum State
  const [name, setName] = useState<string>(item.layer_name || "");
  const [displayColor, setDisplayColor] = useState<string>(item.display_color || "#2ecc40");
  const [icon, setIcon] = useState<string>(item.icon || "");
  const [imageAction, setImageAction] = useState<"keep" | "delete" | "replace">("keep");
  const [selectedFilePath, setSelectedFilePath] = useState<string | null>(null);
  const [selectedFileName, setSelectedFileName] = useState<string | null>(null);
  const [previewUrl, setPreviewUrl] = useState<string | null>(item.thumbnail_url || null);

  // Tab Field State
  const [draftFields, setDraftFields] = useState<FieldDef[]>([]);
  const [selectedFeatureIndex, setSelectedFeatureIndex] = useState<number>(0);
  const [draftSets, setDraftSets] = useState<Map<number, Record<string, unknown>>>(new Map());
  const [draftRemoves, setDraftRemoves] = useState<Map<number, Set<string>>>(new Map());

  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [showCloseConfirm, setShowCloseConfirm] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    let isMounted = true;
    if (onLoadDetail) {
      onLoadDetail(item.id)
        .then((detail) => {
          if (!isMounted) return;
          setLayerDetail(detail);
          const initialFields = resolveDisplayFields(
            detail.display_preference.fields,
            detail.spatial_results.map((r) => r.properties),
          );
          setDraftFields(initialFields);
          const layerImg = detail.display_preference?.image_url || null;
          setPreviewUrl(layerImg);
        })
        .catch((err: unknown) => {
          if (!isMounted) return;
          console.warn("Gagal memuat detail layer:", err);
          setErrorMessage("Gagal memuat detail layer. Menampilkan informasi ringkas.");
        })
        .finally(() => {
          if (isMounted) setIsLoadingDetail(false);
        });
    }

    return () => {
      isMounted = false;
    };
  }, [item.id, onLoadDetail]);

  const isPointGeom = item.geom_type === "Point";

  // Check if form is dirty
  const isDirty =
    name !== (item.layer_name || "") ||
    displayColor !== (item.display_color || "#2ecc40") ||
    icon !== (item.icon || "") ||
    imageAction !== "keep" ||
    draftSets.size > 0 ||
    draftRemoves.size > 0 ||
    JSON.stringify(draftFields) !==
      JSON.stringify(
        layerDetail
          ? resolveDisplayFields(
              layerDetail.display_preference.fields,
              layerDetail.spatial_results.map((r) => r.properties),
            )
          : [],
      );

  const handleRequestClose = () => {
    if (isDirty && !isSubmitting) {
      setShowCloseConfirm(true);
    } else {
      onClose();
    }
  };

  // Upload Foto Layer
  const handleSelectFile = async () => {
    if (isTauri()) {
      try {
        const selected = await open({
          multiple: false,
          directory: false,
          filters: [
            {
              name: "Citra (JPG, PNG, WebP)",
              extensions: ["jpg", "jpeg", "png", "webp"],
            },
          ],
        });

        if (selected && typeof selected === "string") {
          const filename = selected.split(/[/\\]/).pop() || selected;
          setSelectedFilePath(selected);
          setSelectedFileName(filename);
          setPreviewUrl(convertFileSrc(selected));
          setImageAction("replace");
          setErrorMessage(null);
          return;
        }
      } catch (err) {
        console.warn("Tauri dialog error, fallback to HTML file input:", err);
      }
    }
    fileInputRef.current?.click();
  };

  const handleFileInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    setSelectedFilePath(file.name);
    setSelectedFileName(file.name);
    setPreviewUrl(URL.createObjectURL(file));
    setImageAction("replace");
    setErrorMessage(null);
  };

  const handleDeletePhoto = () => {
    setImageAction("delete");
    setSelectedFilePath(null);
    setSelectedFileName(null);
    setPreviewUrl(null);
  };

  const handleResetPhoto = () => {
    setImageAction("keep");
    setSelectedFilePath(null);
    setSelectedFileName(null);
    setPreviewUrl(item.thumbnail_url || null);
  };

  // Field Management Handlers
  const handleAddField = (field: FieldDef, initialVal?: unknown) => {
    setDraftFields((prev) => [...prev, field]);
    if (initialVal !== undefined && layerDetail) {
      const nextSets = new Map(draftSets);
      layerDetail.spatial_results.forEach((_, idx) => {
        const setMap = { ...(nextSets.get(idx) || {}) };
        setMap[field.key] = initialVal;
        nextSets.set(idx, setMap);
      });
      setDraftSets(nextSets);
    }
  };

  const handleDeleteCustomField = (field: FieldDef) => {
    setDraftFields((prev) => prev.filter((f) => f.key !== field.key));
    if (layerDetail) {
      const nextSets = new Map(draftSets);
      const nextRemoves = new Map(draftRemoves);

      layerDetail.spatial_results.forEach((_, idx) => {
        const setMap = { ...(nextSets.get(idx) || {}) };
        delete setMap[field.key];
        nextSets.set(idx, setMap);

        const removeSet = new Set(nextRemoves.get(idx) || []);
        removeSet.add(field.key);
        nextRemoves.set(idx, removeSet);
      });

      setDraftSets(nextSets);
      setDraftRemoves(nextRemoves);
    }
  };

  const handleUpdatePropertyValue = (featureIndex: number, key: string, value: unknown) => {
    const nextSets = new Map(draftSets);
    const setMap = { ...(nextSets.get(featureIndex) || {}) };
    setMap[key] = value;
    nextSets.set(featureIndex, setMap);
    setDraftSets(nextSets);
  };

  // Submit Handler All-or-Nothing
  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const trimmedName = name.trim();
    if (!trimmedName) {
      setErrorMessage("Nama hasil analisis tidak boleh kosong");
      setSelectedTab("general");
      return;
    }
    if (trimmedName.length > 100) {
      setErrorMessage("Nama hasil analisis maksimal 100 karakter");
      setSelectedTab("general");
      return;
    }
    if (!isValidHexColor(displayColor)) {
      setErrorMessage("Format warna tidak valid. Harus format HEX (#RGB atau #RRGGBB)");
      setSelectedTab("general");
      return;
    }

    // Validasi seluruh label field
    for (const f of draftFields) {
      const labelErr = validateFieldLabel(f.label);
      if (labelErr) {
        setErrorMessage(`Field '${f.key}': ${labelErr}`);
        setSelectedTab("fields");
        return;
      }
    }

    // Bangun payload feature_edits
    const featureEdits: FeatureEdit[] = [];
    const allFeatureIndices = new Set([
      ...Array.from(draftSets.keys()),
      ...Array.from(draftRemoves.keys()),
    ]);

    allFeatureIndices.forEach((idx) => {
      const set = draftSets.get(idx) || {};
      const remove = Array.from(draftRemoves.get(idx) || []);
      if (Object.keys(set).length > 0 || remove.length > 0) {
        featureEdits.push({ index: idx, set, remove });
      }
    });

    const concurrencyToken = layerDetail?.concurrency_token || item.concurrency_token;

    const payload: UpdateSpatialResultPayload = {
      id: item.id,
      name: trimmedName,
      display_color: displayColor,
      icon: isPointGeom ? icon.trim() || undefined : undefined,
      image_action: imageAction,
      image_path: selectedFilePath || undefined,
      fields: draftFields,
      feature_edits: featureEdits.length > 0 ? featureEdits : undefined,
      expected_updated_at: concurrencyToken || undefined,
    };

    setErrorMessage(null);
    void onSave(payload)
      .then(() => {
        onClose();
      })
      .catch((err: unknown) => {
        const msg = err instanceof Error ? err.message : String(err);
        setErrorMessage(msg);
      });
  };

  // Ikon yang didukung
  const isCustomIcon = Boolean(icon) && !KNOWN_ICONS.some((ki) => ki.value === icon);
  const iconOptions = [...KNOWN_ICONS];
  if (isCustomIcon) {
    iconOptions.push({ value: icon, label: `(Kustom) ${icon}` });
  }

  const selectedFeature: SpatialResult | null =
    layerDetail?.spatial_results[selectedFeatureIndex] || null;

  return (
    <form onSubmit={handleSubmit}>
      <div
        className={Classes.DIALOG_BODY}
        style={{ minHeight: "320px", maxHeight: "72vh", overflowY: "auto", padding: "16px 20px" }}
      >
        {errorMessage && (
          <Callout intent="danger" style={{ marginBottom: "14px" }}>
            {errorMessage}
          </Callout>
        )}

        {isLoadingDetail ? (
          <div
            style={{
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              padding: "40px",
              gap: "10px",
            }}
          >
            <Spinner size={30} />
            <span style={{ fontSize: "12px", color: "#64748b" }}>
              Memuat data dan atribut layer...
            </span>
          </div>
        ) : (
          <>
            {/* Segmented Control Pill Bar */}
            <div
              style={{
                display: "flex",
                background: "#f1f5f9",
                padding: "3px",
                borderRadius: "8px",
                gap: "3px",
                marginBottom: "16px",
              }}
            >
              <button
                type="button"
                onClick={() => setSelectedTab("general")}
                style={{
                  flex: 1,
                  padding: "8px 10px",
                  fontSize: "12px",
                  fontWeight: selectedTab === "general" ? 600 : 500,
                  color: selectedTab === "general" ? "#0f172a" : "#64748b",
                  background: selectedTab === "general" ? "#ffffff" : "transparent",
                  border: "none",
                  outline: "none",
                  borderRadius: "6px",
                  boxShadow: selectedTab === "general" ? "0 1px 3px rgba(0,0,0,0.08)" : "none",
                  cursor: "pointer",
                  transition: "all 0.15s ease",
                  whiteSpace: "nowrap",
                  userSelect: "none",
                }}
              >
                Umum & Visual
              </button>
              <button
                type="button"
                onClick={() => setSelectedTab("fields")}
                style={{
                  flex: 1,
                  padding: "8px 10px",
                  fontSize: "12px",
                  fontWeight: selectedTab === "fields" ? 600 : 500,
                  color: selectedTab === "fields" ? "#0f172a" : "#64748b",
                  background: selectedTab === "fields" ? "#ffffff" : "transparent",
                  border: "none",
                  outline: "none",
                  borderRadius: "6px",
                  boxShadow: selectedTab === "fields" ? "0 1px 3px rgba(0,0,0,0.08)" : "none",
                  cursor: "pointer",
                  transition: "all 0.15s ease",
                  whiteSpace: "nowrap",
                  userSelect: "none",
                }}
              >
                {`Skema Field (${draftFields.length})`}
              </button>
              <button
                type="button"
                onClick={() => setSelectedTab("values")}
                style={{
                  flex: 1,
                  padding: "8px 10px",
                  fontSize: "12px",
                  fontWeight: selectedTab === "values" ? 600 : 500,
                  color: selectedTab === "values" ? "#0f172a" : "#64748b",
                  background: selectedTab === "values" ? "#ffffff" : "transparent",
                  border: "none",
                  outline: "none",
                  borderRadius: "6px",
                  boxShadow: selectedTab === "values" ? "0 1px 3px rgba(0,0,0,0.08)" : "none",
                  cursor: "pointer",
                  transition: "all 0.15s ease",
                  whiteSpace: "nowrap",
                  userSelect: "none",
                }}
              >
                Nilai Atribut Fitur
              </button>
              <button
                type="button"
                onClick={() => setSelectedTab("preview")}
                style={{
                  flex: 1,
                  padding: "8px 10px",
                  fontSize: "12px",
                  fontWeight: selectedTab === "preview" ? 600 : 500,
                  color: selectedTab === "preview" ? "#0f172a" : "#64748b",
                  background: selectedTab === "preview" ? "#ffffff" : "transparent",
                  border: "none",
                  outline: "none",
                  borderRadius: "6px",
                  boxShadow: selectedTab === "preview" ? "0 1px 3px rgba(0,0,0,0.08)" : "none",
                  cursor: "pointer",
                  transition: "all 0.15s ease",
                  whiteSpace: "nowrap",
                  userSelect: "none",
                }}
              >
                Pratinjau Popup
              </button>
            </div>

            {/* TAB 1: UMUM */}
            {selectedTab === "general" && (
              <div style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
                {/* Ringkasan Metadata Immutable */}
                <div
                  style={{
                    background: "#f8fafc",
                    borderRadius: "12px",
                    padding: "12px 16px",
                    display: "grid",
                    gridTemplateColumns: "1fr 1fr 1fr",
                    gap: "8px",
                    fontSize: "12px",
                  }}
                >
                  <div>
                    <span style={{ color: "#64748b" }}>ID Layer:</span>{" "}
                    <code style={{ fontSize: "11px", color: "#0f172a" }}>{item.id}</code>
                  </div>
                  <div>
                    <span style={{ color: "#64748b" }}>Geometri:</span>{" "}
                    <span
                      style={{
                        background: "#e2e8f0",
                        color: "#334155",
                        padding: "2px 8px",
                        borderRadius: "6px",
                        fontSize: "10px",
                        fontWeight: 600,
                      }}
                    >
                      {item.geom_type}
                    </span>
                  </div>
                  <div>
                    <span style={{ color: "#64748b" }}>Jumlah Fitur:</span>{" "}
                    <strong style={{ color: "#0f172a" }}>{item.feature_count} fitur</strong>
                  </div>
                </div>

                {/* Nama Layer */}
                <div>
                  <div
                    style={{
                      fontSize: "12px",
                      fontWeight: 600,
                      color: "#64748b",
                      marginBottom: "6px",
                    }}
                  >
                    Nama Hasil Analisis
                  </div>
                  <input
                    id="layer-name-input"
                    type="text"
                    value={name}
                    onChange={(e) => setName(e.target.value)}
                    placeholder="Nama layer..."
                    disabled={isSubmitting}
                    style={{
                      width: "100%",
                      padding: "9px 12px",
                      fontSize: "13px",
                      background: "#f8fafc",
                      border: "none",
                      borderRadius: "10px",
                      outline: "none",
                      color: "#0f172a",
                      boxSizing: "border-box",
                    }}
                  />
                  <div style={{ fontSize: "11px", color: "#64748b", marginTop: "4px" }}>
                    Nama yang tampil pada sidebar dan judul popup
                  </div>
                </div>

                {/* Dropdown Ikon (Hanya jika geometri Point) */}
                {isPointGeom && (
                  <div>
                    <div
                      style={{
                        fontSize: "12px",
                        fontWeight: 600,
                        color: "#64748b",
                        marginBottom: "6px",
                      }}
                    >
                      Ikon Titik Marker
                    </div>
                    <select
                      id="layer-icon-select"
                      value={icon}
                      onChange={(e) => setIcon(e.target.value)}
                      disabled={isSubmitting}
                      style={{
                        width: "100%",
                        padding: "9px 12px",
                        fontSize: "13px",
                        borderRadius: "10px",
                        border: "none",
                        background: "#f8fafc",
                        color: "#0f172a",
                        outline: "none",
                        boxSizing: "border-box",
                        cursor: "pointer",
                      }}
                    >
                      {iconOptions.map((opt) => (
                        <option key={opt.value} value={opt.value}>
                          {opt.label}
                        </option>
                      ))}
                    </select>
                    <div style={{ fontSize: "11px", color: "#64748b", marginTop: "4px" }}>
                      Pilih visualisasi titik penanda pada peta Leaflet
                    </div>
                  </div>
                )}

                {/* Warna Tampilan */}
                <div>
                  <div
                    style={{
                      fontSize: "12px",
                      fontWeight: 600,
                      color: "#64748b",
                      marginBottom: "6px",
                    }}
                  >
                    Warna Tampilan (HEX)
                  </div>
                  <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
                    <div
                      style={{
                        width: "34px",
                        height: "34px",
                        borderRadius: "8px",
                        backgroundColor: isValidHexColor(displayColor) ? displayColor : "#e2e8f0",
                        border: "none",
                        boxShadow: "0 1px 3px rgba(0,0,0,0.1)",
                        flexShrink: 0,
                      }}
                    />
                    <input
                      id="layer-color-input"
                      type="text"
                      value={displayColor}
                      onChange={(e) => setDisplayColor(e.target.value)}
                      placeholder="#2ecc40"
                      disabled={isSubmitting}
                      style={{
                        width: "110px",
                        padding: "8px 12px",
                        fontSize: "13px",
                        background: "#f8fafc",
                        border: "none",
                        borderRadius: "10px",
                        outline: "none",
                        color: "#0f172a",
                        boxSizing: "border-box",
                      }}
                    />
                    {/* Palet Preset */}
                    <div
                      style={{
                        display: "flex",
                        gap: "6px",
                        flexWrap: "wrap",
                        alignItems: "center",
                      }}
                    >
                      {COLOR_PRESETS.map((c) => (
                        <button
                          key={c}
                          type="button"
                          onClick={() => setDisplayColor(c)}
                          style={{
                            width: "24px",
                            height: "24px",
                            borderRadius: "6px",
                            backgroundColor: c,
                            border:
                              displayColor.toLowerCase() === c.toLowerCase()
                                ? "2px solid #0f172a"
                                : "none",
                            cursor: "pointer",
                            padding: 0,
                          }}
                        />
                      ))}
                    </div>
                  </div>
                  <div style={{ fontSize: "11px", color: "#64748b", marginTop: "4px" }}>
                    Warna penanda titik, garis batas poligon, atau jalur inspeksi
                  </div>
                </div>

                {/* Pengaturan Foto Layer */}
                <div>
                  <div
                    style={{
                      fontSize: "12px",
                      fontWeight: 600,
                      color: "#64748b",
                      marginBottom: "6px",
                    }}
                  >
                    Foto Layer / Citra Utama
                  </div>
                  <div
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: "14px",
                      padding: "14px",
                      borderRadius: "12px",
                      background: "#f8fafc",
                    }}
                  >
                    <div style={{ flexShrink: 0 }}>
                      {imageAction === "delete" ? (
                        <div
                          style={{
                            width: 72,
                            height: 56,
                            borderRadius: 8,
                            background: "#f1f5f9",
                            border: "1px dashed #cbd5e1",
                            display: "flex",
                            flexDirection: "column",
                            alignItems: "center",
                            justifyContent: "center",
                            color: "#94a3b8",
                          }}
                        >
                          <Icon icon="media" size={20} />
                          <span style={{ fontSize: 10, marginTop: 2, fontWeight: 500 }}>
                            No Image
                          </span>
                        </div>
                      ) : (
                        <ResultPhoto
                          resultId={item.id}
                          imageUrl={previewUrl}
                          size="medium"
                          alt="Foto Layer"
                        />
                      )}
                    </div>

                    <div style={{ flex: 1, display: "flex", flexDirection: "column", gap: "6px" }}>
                      <div style={{ fontSize: "12px" }}>
                        {imageAction === "delete" && (
                          <Tag intent="danger" minimal style={{ fontSize: "11px" }}>
                            Foto layer akan dihapus (No Image)
                          </Tag>
                        )}
                        {imageAction === "replace" && (
                          <Tag intent="success" minimal style={{ fontSize: "11px" }}>
                            Foto baru: {selectedFileName}
                          </Tag>
                        )}
                        {imageAction === "keep" && (
                          <span style={{ color: "#64748b" }}>
                            {previewUrl ? "Foto layer aktif terpasang" : "Belum ada foto layer"}
                          </span>
                        )}
                      </div>

                      <div style={{ display: "flex", gap: "8px", flexWrap: "wrap" }}>
                        <Button
                          icon="upload"
                          small
                          minimal
                          text={previewUrl ? "Ganti Foto" : "Unggah Foto"}
                          onClick={() => {
                            void handleSelectFile();
                          }}
                          disabled={isSubmitting}
                          style={{
                            background: "#ffffff",
                            color: "#0f172a",
                            borderRadius: "999px",
                            padding: "4px 12px",
                            boxShadow: "0 1px 3px rgba(0,0,0,0.06)",
                            fontWeight: 600,
                          }}
                        />
                        {previewUrl && imageAction !== "delete" && (
                          <Button
                            icon="trash"
                            small
                            minimal
                            text="Hapus Foto"
                            onClick={handleDeletePhoto}
                            disabled={isSubmitting}
                            style={{
                              background: "#f1f5f9",
                              color: "#475569",
                              borderRadius: "999px",
                              padding: "4px 12px",
                              fontWeight: 600,
                            }}
                          />
                        )}
                        {imageAction !== "keep" && (
                          <Button
                            icon="undo"
                            small
                            minimal
                            text="Batal Ubah"
                            onClick={handleResetPhoto}
                            disabled={isSubmitting}
                            style={{
                              background: "#f1f5f9",
                              color: "#475569",
                              borderRadius: "999px",
                              padding: "4px 12px",
                              fontWeight: 600,
                            }}
                          />
                        )}
                      </div>
                    </div>

                    <input
                      ref={fileInputRef}
                      type="file"
                      accept="image/jpeg,image/png,image/webp"
                      style={{ display: "none" }}
                      onChange={handleFileInputChange}
                    />
                  </div>
                  <div style={{ fontSize: "11px", color: "#64748b", marginTop: "4px" }}>
                    Foto representasi keseluruhan layer (tersimpan pada
                    display_preference.image_url)
                  </div>
                </div>
              </div>
            )}

            {/* TAB 2: SKEMA FIELD */}
            {selectedTab === "fields" && (
              <div
                style={{
                  display: "flex",
                  flexDirection: "column",
                  gap: "16px",
                }}
              >
                <FieldManagerTable
                  fields={draftFields}
                  onChange={setDraftFields}
                  onDeleteCustomField={handleDeleteCustomField}
                  onAddField={handleAddField}
                />
              </div>
            )}

            {/* TAB 3: NILAI ATRIBUT FITUR */}
            {selectedTab === "values" && (
              <div
                style={{
                  display: "flex",
                  flexDirection: "column",
                  gap: "20px",
                }}
              >
                {layerDetail && layerDetail.spatial_results.length > 0 ? (
                  <FeatureValuesEditor
                    features={layerDetail.spatial_results}
                    fields={draftFields}
                    selectedIndex={selectedFeatureIndex}
                    onSelectIndex={setSelectedFeatureIndex}
                    draftSets={draftSets}
                    onUpdateValue={handleUpdatePropertyValue}
                  />
                ) : (
                  <div
                    style={{
                      textAlign: "center",
                      padding: "40px 20px",
                      color: "#64748b",
                      fontSize: "13px",
                      background: "#f8fafc",
                      borderRadius: "12px",
                    }}
                  >
                    Tidak ada fitur spasial untuk diedit.
                  </div>
                )}
              </div>
            )}

            {/* TAB 4: PRATINJAU POPUP */}
            {selectedTab === "preview" && (
              <div
                style={{
                  display: "flex",
                  flexDirection: "column",
                  alignItems: "center",
                  gap: "16px",
                  padding: "8px 0 16px 0",
                }}
              >
                {layerDetail && layerDetail.spatial_results.length > 1 && (
                  <div
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: "10px",
                      background: "#f8fafc",
                      padding: "6px 14px",
                      borderRadius: "999px",
                      border: "1px solid #f1f5f9",
                    }}
                  >
                    <button
                      type="button"
                      disabled={selectedFeatureIndex === 0}
                      onClick={() => setSelectedFeatureIndex((prev) => Math.max(0, prev - 1))}
                      style={{
                        background: "none",
                        border: "none",
                        cursor: selectedFeatureIndex === 0 ? "not-allowed" : "pointer",
                        opacity: selectedFeatureIndex === 0 ? 0.3 : 1,
                        padding: "2px 4px",
                        display: "flex",
                        alignItems: "center",
                        outline: "none",
                      }}
                    >
                      <Icon icon="chevron-left" size={14} color="#0f172a" />
                    </button>
                    <span style={{ fontSize: "11px", fontWeight: 600, color: "#334155" }}>
                      Fitur #{selectedFeatureIndex + 1} dari {layerDetail.spatial_results.length}
                    </span>
                    <button
                      type="button"
                      disabled={selectedFeatureIndex >= layerDetail.spatial_results.length - 1}
                      onClick={() =>
                        setSelectedFeatureIndex((prev) =>
                          Math.min(layerDetail.spatial_results.length - 1, prev + 1),
                        )
                      }
                      style={{
                        background: "none",
                        border: "none",
                        cursor:
                          selectedFeatureIndex >= layerDetail.spatial_results.length - 1
                            ? "not-allowed"
                            : "pointer",
                        opacity:
                          selectedFeatureIndex >= layerDetail.spatial_results.length - 1 ? 0.3 : 1,
                        padding: "2px 4px",
                        display: "flex",
                        alignItems: "center",
                        outline: "none",
                      }}
                    >
                      <Icon icon="chevron-right" size={14} color="#0f172a" />
                    </button>
                  </div>
                )}

                <div style={{ width: "100%", maxWidth: "340px" }}>
                  <PopupPreviewCard
                    layerName={name.trim() || item.layer_name}
                    displayColor={displayColor}
                    layerImageUrl={imageAction === "delete" ? null : previewUrl}
                    feature={selectedFeature}
                    fields={draftFields}
                    draftValues={draftSets.get(selectedFeatureIndex)}
                  />
                </div>
              </div>
            )}
          </>
        )}
      </div>

      {/* Footer All-or-Nothing */}
      <div className={Classes.DIALOG_FOOTER}>
        <div className={Classes.DIALOG_FOOTER_ACTIONS}>
          <Button
            text="Batal"
            minimal
            onClick={handleRequestClose}
            disabled={isSubmitting}
            style={{
              borderRadius: "999px",
              background: "#f1f5f9",
              color: "#475569",
              fontWeight: 600,
              padding: "6px 16px",
              boxShadow: "none",
            }}
          />
          <Button
            minimal
            text={isSubmitting ? "Menyimpan..." : "Simpan Perubahan"}
            type="submit"
            loading={isSubmitting}
            disabled={!isDirty || isSubmitting || isLoadingDetail}
            style={{
              borderRadius: "999px",
              background: !isDirty || isSubmitting || isLoadingDetail ? "#f1f5f9" : "#0f172a",
              color: !isDirty || isSubmitting || isLoadingDetail ? "#94a3b8" : "#ffffff",
              fontWeight: 600,
              padding: "6px 18px",
              boxShadow:
                !isDirty || isSubmitting || isLoadingDetail
                  ? "none"
                  : "0 2px 8px rgba(15,23,42,0.15)",
            }}
          />
        </div>
      </div>

      {/* Dialog Konfirmasi Batal jika ada perubahan */}
      <Dialog
        isOpen={showCloseConfirm}
        onClose={() => setShowCloseConfirm(false)}
        title="Perubahan Belum Disimpan"
        style={{ width: "380px", borderRadius: "16px" }}
      >
        <div style={{ padding: "16px 20px" }}>
          <p style={{ fontSize: "13px", lineHeight: "1.5" }}>
            Anda memiliki perubahan yang belum disimpan. Yakin ingin menutup formulir?
          </p>
          <div
            style={{
              display: "flex",
              justifyContent: "flex-end",
              gap: "8px",
              marginTop: "16px",
            }}
          >
            <Button
              text="Lanjut Mengedit"
              minimal
              style={{
                borderRadius: "8px",
                background: "#f1f5f9",
                color: "#475569",
                fontWeight: 600,
              }}
              onClick={() => setShowCloseConfirm(false)}
            />
            <Button
              text="Tutup & Buang Perubahan"
              minimal
              style={{
                borderRadius: "8px",
                background: "#ef4444",
                color: "#ffffff",
                fontWeight: 600,
              }}
              onClick={() => {
                setShowCloseConfirm(false);
                onClose();
              }}
            />
          </div>
        </div>
      </Dialog>
    </form>
  );
};

export const EditSpResultModal: React.FC<EditSpResultModalProps> = ({
  isOpen,
  item,
  onClose,
  onSave,
  onLoadDetail,
  isSubmitting = false,
}) => {
  if (!isOpen || !item) return null;

  return (
    <Dialog
      isOpen={isOpen}
      onClose={onClose}
      title={`Edit Layer Hasil Analisis: ${item.layer_name || item.id}`}
      style={{ width: "720px", maxWidth: "94vw", borderRadius: "16px" }}
    >
      <EditSpResultForm
        key={item.id}
        item={item}
        onClose={onClose}
        onSave={onSave}
        onLoadDetail={onLoadDetail}
        isSubmitting={isSubmitting}
      />
    </Dialog>
  );
};
