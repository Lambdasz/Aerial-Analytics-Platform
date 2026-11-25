import { useState, useEffect, useRef } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { convertFileSrc, isTauri } from "@tauri-apps/api/core";
import type {
  SpatialResultSummary,
  UpdateSpatialResultPayload,
  FieldDef,
  FeatureEdit,
} from "../types/spatialResult";
import type { LayerPayload, SpatialResult } from "../types/map";
import { resolveDisplayFields } from "../lib/fieldResolution";
import { isValidHexColor, validateFieldLabel } from "../lib/fieldValidation";

export interface UseSpResultFormControllerOptions {
  item: SpatialResultSummary;
  onClose: () => void;
  onSave: (payload: UpdateSpatialResultPayload) => Promise<void>;
  onLoadDetail?: (id: string) => Promise<LayerPayload>;
  isSubmitting?: boolean;
}

export function useSpResultFormController({
  item,
  onClose,
  onSave,
  onLoadDetail,
  isSubmitting = false,
}: UseSpResultFormControllerOptions) {
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

    for (const f of draftFields) {
      const labelErr = validateFieldLabel(f.label);
      if (labelErr) {
        setErrorMessage(`Field '${f.key}': ${labelErr}`);
        setSelectedTab("fields");
        return;
      }
    }

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

  const selectedFeature: SpatialResult | null =
    layerDetail?.spatial_results[selectedFeatureIndex] || null;

  return {
    selectedTab,
    setSelectedTab,
    isLoadingDetail,
    layerDetail,
    // General Tab State
    name,
    setName,
    displayColor,
    setDisplayColor,
    icon,
    setIcon,
    imageAction,
    selectedFileName,
    previewUrl,
    fileInputRef,
    handleSelectFile,
    handleFileInputChange,
    handleDeletePhoto,
    handleResetPhoto,
    // Fields & Values Tab State
    draftFields,
    setDraftFields,
    handleAddField,
    handleDeleteCustomField,
    selectedFeatureIndex,
    setSelectedFeatureIndex,
    draftSets,
    handleUpdatePropertyValue,
    selectedFeature,
    // Form & Modal State
    isDirty,
    errorMessage,
    showCloseConfirm,
    setShowCloseConfirm,
    handleRequestClose,
    handleSubmit,
  };
}
