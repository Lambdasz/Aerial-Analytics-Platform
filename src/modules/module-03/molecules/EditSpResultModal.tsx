import React, { useState, useEffect, useRef } from "react";
import {
  Dialog,
  Button,
  FormGroup,
  InputGroup,
  Classes,
  Callout,
  Tag,
  Icon,
  Tabs,
  Tab,
  HTMLSelect,
  Spinner,
} from "@blueprintjs/core";
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
  const [selectedTab, setSelectedTab] = useState<string>("general");
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
      <div className={Classes.DIALOG_BODY} style={{ padding: "16px 20px" }}>
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
          <Tabs
            id="edit-sp-result-tabs"
            selectedTabId={selectedTab}
            onChange={(id) => setSelectedTab(id as string)}
          >
            {/* TAB 1: UMUM */}
            <Tab
              id="general"
              title="Umum & Visual"
              panel={
                <div style={{ paddingTop: "12px" }}>
                  {/* Ringkasan Metadata Immutable */}
                  <div
                    style={{
                      background: "rgba(16, 22, 26, 0.04)",
                      border: "1px solid rgba(16, 22, 26, 0.1)",
                      borderRadius: "6px",
                      padding: "10px 14px",
                      marginBottom: "16px",
                      display: "grid",
                      gridTemplateColumns: "1fr 1fr 1fr",
                      gap: "8px",
                      fontSize: "12px",
                    }}
                  >
                    <div>
                      <span style={{ color: "#64748b" }}>ID Layer:</span>{" "}
                      <code style={{ fontSize: "11px" }}>{item.id}</code>
                    </div>
                    <div>
                      <span style={{ color: "#64748b" }}>Geometri:</span>{" "}
                      <Tag minimal round style={{ fontSize: "10px" }}>
                        {item.geom_type}
                      </Tag>
                    </div>
                    <div>
                      <span style={{ color: "#64748b" }}>Jumlah Fitur:</span>{" "}
                      <strong>{item.feature_count} fitur</strong>
                    </div>
                  </div>

                  {/* Nama Layer */}
                  <FormGroup
                    label="Nama Hasil Analisis"
                    labelFor="layer-name-input"
                    helperText="Nama yang tampil pada sidebar dan judul popup"
                  >
                    <InputGroup
                      id="layer-name-input"
                      value={name}
                      onChange={(e) => setName(e.target.value)}
                      placeholder="Nama layer..."
                      disabled={isSubmitting}
                    />
                  </FormGroup>

                  {/* Dropdown Ikon (Hanya jika geometri Point) */}
                  {isPointGeom && (
                    <FormGroup
                      label="Ikon Titik Marker"
                      labelFor="layer-icon-select"
                      helperText="Pilih visualisasi titik penanda pada peta Leaflet"
                    >
                      <HTMLSelect
                        id="layer-icon-select"
                        value={icon}
                        onChange={(e) => setIcon(e.target.value)}
                        fill
                        disabled={isSubmitting}
                      >
                        {iconOptions.map((opt) => (
                          <option key={opt.value} value={opt.value}>
                            {opt.label}
                          </option>
                        ))}
                      </HTMLSelect>
                    </FormGroup>
                  )}

                  {/* Warna Tampilan */}
                  <FormGroup
                    label="Warna Tampilan (HEX)"
                    labelFor="layer-color-input"
                    helperText="Warna penanda titik, garis batas poligon, atau jalur inspeksi"
                  >
                    <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
                      <div
                        style={{
                          width: "32px",
                          height: "32px",
                          borderRadius: "6px",
                          backgroundColor: isValidHexColor(displayColor) ? displayColor : "#e2e8f0",
                          border: "1px solid rgba(16, 22, 26, 0.2)",
                          flexShrink: 0,
                        }}
                      />
                      <InputGroup
                        id="layer-color-input"
                        value={displayColor}
                        onChange={(e) => setDisplayColor(e.target.value)}
                        placeholder="#2ecc40"
                        style={{ width: "120px" }}
                        disabled={isSubmitting}
                      />
                      {/* Palet Preset */}
                      <div style={{ display: "flex", gap: "5px", flexWrap: "wrap" }}>
                        {COLOR_PRESETS.map((c) => (
                          <button
                            key={c}
                            type="button"
                            onClick={() => setDisplayColor(c)}
                            style={{
                              width: "24px",
                              height: "24px",
                              borderRadius: "4px",
                              backgroundColor: c,
                              border:
                                displayColor.toLowerCase() === c.toLowerCase()
                                  ? "2px solid #000"
                                  : "1px solid rgba(0,0,0,0.15)",
                              cursor: "pointer",
                              padding: 0,
                            }}
                          />
                        ))}
                      </div>
                    </div>
                  </FormGroup>

                  {/* Pengaturan Foto Layer */}
                  <FormGroup
                    label="Foto Layer / Citra Utama"
                    helperText="Foto representasi keseluruhan layer (tersimpan pada display_preference.image_url)"
                  >
                    <div
                      style={{
                        display: "flex",
                        alignItems: "center",
                        gap: "14px",
                        padding: "12px",
                        borderRadius: "8px",
                        border: "1px solid rgba(16, 22, 26, 0.15)",
                        background: "rgba(16, 22, 26, 0.02)",
                      }}
                    >
                      <div style={{ flexShrink: 0 }}>
                        {imageAction === "delete" ? (
                          <div
                            style={{
                              width: 72,
                              height: 56,
                              borderRadius: 6,
                              background: "rgba(143, 153, 168, 0.12)",
                              border: "1px dashed rgba(143, 153, 168, 0.3)",
                              display: "flex",
                              flexDirection: "column",
                              alignItems: "center",
                              justifyContent: "center",
                              color: "#8a9ba8",
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

                      <div
                        style={{ flex: 1, display: "flex", flexDirection: "column", gap: "6px" }}
                      >
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
                            text={previewUrl ? "Ganti Foto" : "Unggah Foto"}
                            onClick={() => {
                              void handleSelectFile();
                            }}
                            disabled={isSubmitting}
                          />
                          {previewUrl && imageAction !== "delete" && (
                            <Button
                              icon="trash"
                              small
                              intent="danger"
                              minimal
                              text="Hapus Foto"
                              onClick={handleDeletePhoto}
                              disabled={isSubmitting}
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
                  </FormGroup>
                </div>
              }
            />

            {/* TAB 2: FIELD & ATRIBUT */}
            <Tab
              id="fields"
              title={`Field & Atribut (${draftFields.length})`}
              panel={
                <div
                  style={{
                    paddingTop: "12px",
                    display: "flex",
                    flexDirection: "column",
                    gap: "24px",
                  }}
                >
                  {/* Seksi 1: Tabel Kelola Field */}
                  <FieldManagerTable
                    fields={draftFields}
                    onChange={setDraftFields}
                    onDeleteCustomField={handleDeleteCustomField}
                    onAddField={handleAddField}
                  />

                  {/* Seksi 2: Editor Nilai per Fitur */}
                  {layerDetail && layerDetail.spatial_results.length > 0 && (
                    <FeatureValuesEditor
                      features={layerDetail.spatial_results}
                      fields={draftFields}
                      selectedIndex={selectedFeatureIndex}
                      onSelectIndex={setSelectedFeatureIndex}
                      draftSets={draftSets}
                      onUpdateValue={handleUpdatePropertyValue}
                    />
                  )}

                  {/* Seksi 3: Pratinjau Tampilan Popup Peta */}
                  <div
                    style={{
                      borderTop: "1px solid #e2e8f0",
                      paddingTop: "16px",
                      display: "flex",
                      flexDirection: "column",
                      alignItems: "center",
                    }}
                  >
                    <div
                      style={{
                        width: "100%",
                        maxWidth: "340px",
                        display: "flex",
                        flexDirection: "column",
                        gap: "8px",
                      }}
                    >
                      <div
                        style={{
                          fontSize: "12px",
                          fontWeight: 600,
                          color: "#475569",
                          display: "flex",
                          alignItems: "center",
                          gap: "6px",
                        }}
                      >
                        <Icon icon="eye-open" size={14} color="#64748b" />
                        <span>Pratinjau Tampilan Popup Peta</span>
                      </div>
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
                </div>
              }
            />
          </Tabs>
        )}
      </div>

      {/* Footer All-or-Nothing */}
      <div className={Classes.DIALOG_FOOTER}>
        <div className={Classes.DIALOG_FOOTER_ACTIONS}>
          <Button text="Batal" onClick={handleRequestClose} disabled={isSubmitting} />
          <Button
            text={isSubmitting ? "Menyimpan..." : "Simpan Perubahan"}
            intent="primary"
            type="submit"
            loading={isSubmitting}
            disabled={!isDirty || isSubmitting || isLoadingDetail}
          />
        </div>
      </div>

      {/* Dialog Konfirmasi Batal jika ada perubahan */}
      <Dialog
        isOpen={showCloseConfirm}
        onClose={() => setShowCloseConfirm(false)}
        title="Perubahan Belum Disimpan"
        style={{ width: "380px" }}
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
            <Button text="Lanjut Mengedit" onClick={() => setShowCloseConfirm(false)} />
            <Button
              text="Tutup & Buang Perubahan"
              intent="danger"
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
      style={{ width: "640px", maxWidth: "95vw" }}
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
