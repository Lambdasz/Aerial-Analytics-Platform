import { useCallback, useMemo, useState } from "react";
import type * as L from "leaflet";
import type { AnnotationFeature, SpatialGeometry } from "../types/layer";
import { useAnnotationMapPicker, type AnnotationGeomType } from "./useAnnotationMapPicker";

export interface UseAnnotationModalFormOptions {
  map: L.Map | null;
  isOpen: boolean;
  initialAnnotation?: AnnotationFeature | null;
  onSaveAnnotation: (args: { text: string; geometry: SpatialGeometry }) => Promise<void>;
  onDrawingStateChange?: (isPicking: boolean, geomType: AnnotationGeomType) => void;
  onClose: () => void;
}

/**
 * Controller hook untuk form modal anotasi spasial.
 * Mengelola state input teks, tipe geometri, koordinat (peta / manual), validasi, dan persistensi.
 */
export function useAnnotationModalForm({
  map,
  isOpen,
  initialAnnotation,
  onSaveAnnotation,
  onDrawingStateChange,
  onClose,
}: UseAnnotationModalFormOptions) {
  const [annotationText, setAnnotationText] = useState("");
  const [geomType, setGeomType] = useState<AnnotationGeomType>("Point");
  const [latitude, setLatitude] = useState("");
  const [longitude, setLongitude] = useState("");
  const [manualCoordsText, setManualCoordsText] = useState("");
  const [draftGeometry, setDraftGeometry] = useState<SpatialGeometry | null>(null);
  const [coordMode, setCoordMode] = useState<"map" | "manual">("map");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [prevIsOpen, setPrevIsOpen] = useState(isOpen);
  const [prevInitialAnnotation, setPrevInitialAnnotation] = useState(initialAnnotation);

  if (isOpen !== prevIsOpen || initialAnnotation !== prevInitialAnnotation) {
    setPrevIsOpen(isOpen);
    setPrevInitialAnnotation(initialAnnotation);

    if (isOpen) {
      if (initialAnnotation) {
        setAnnotationText(initialAnnotation.text || "");
        setGeomType(initialAnnotation.geometry.type || "Point");
        setDraftGeometry(initialAnnotation.geometry || null);
        if (initialAnnotation.geometry.type === "Point") {
          const [lng, lat] = initialAnnotation.geometry.coordinates;
          setLatitude(lat.toString());
          setLongitude(lng.toString());
          setManualCoordsText("");
        } else if (initialAnnotation.geometry.type === "LineString") {
          setLatitude("");
          setLongitude("");
          setManualCoordsText(
            initialAnnotation.geometry.coordinates.map(([lng, lat]) => `${lat}, ${lng}`).join("\n"),
          );
        } else if (initialAnnotation.geometry.type === "Polygon") {
          setLatitude("");
          setLongitude("");
          const ring = (initialAnnotation.geometry.coordinates[0] || []) as [number, number][];
          const pts =
            ring.length >= 4 &&
            ring[0][0] === ring[ring.length - 1][0] &&
            ring[0][1] === ring[ring.length - 1][1]
              ? ring.slice(0, -1)
              : ring;
          setManualCoordsText(pts.map(([lng, lat]) => `${lat}, ${lng}`).join("\n"));
        }
      } else {
        setAnnotationText("");
        setGeomType("Point");
        setLatitude("");
        setLongitude("");
        setManualCoordsText("");
        setDraftGeometry(null);
      }
      setCoordMode("map");
      setError(null);
    }
  }

  const handleGeometryPicked = useCallback(
    (geom: SpatialGeometry, latStr?: string, lngStr?: string) => {
      setDraftGeometry(geom);
      if (latStr !== undefined) setLatitude(latStr);
      if (lngStr !== undefined) setLongitude(lngStr);
      setError(null);
    },
    [],
  );

  const {
    isPickingLocation,
    isTemporarilyClosedForMap,
    togglePickLocation,
    clearPreviewLayer,
    stopPicking,
  } = useAnnotationMapPicker({
    map,
    isOpen,
    geomType,
    draftGeometry,
    coordMode,
    onGeometryPicked: handleGeometryPicked,
    onDrawingStateChange,
    onError: setError,
  });

  const handleGeomTypeChange = (newType: AnnotationGeomType) => {
    stopPicking();
    clearPreviewLayer();
    setGeomType(newType);
    setDraftGeometry(null);
    setLatitude("");
    setLongitude("");
    setManualCoordsText("");
    setError(null);
  };

  const handleCoordModeChange = (newMode: "map" | "manual") => {
    stopPicking();
    setCoordMode(newMode);
    setError(null);

    if (draftGeometry) {
      if (draftGeometry.type === "Point") {
        const [lng, lat] = draftGeometry.coordinates;
        setLatitude(lat.toString());
        setLongitude(lng.toString());
      } else if (draftGeometry.type === "LineString") {
        setManualCoordsText(
          draftGeometry.coordinates.map(([lng, lat]) => `${lat}, ${lng}`).join("\n"),
        );
      } else if (draftGeometry.type === "Polygon") {
        const ring = (draftGeometry.coordinates[0] || []) as [number, number][];
        const pts =
          ring.length >= 4 &&
          ring[0][0] === ring[ring.length - 1][0] &&
          ring[0][1] === ring[ring.length - 1][1]
            ? ring.slice(0, -1)
            : ring;
        setManualCoordsText(pts.map(([lng, lat]) => `${lat}, ${lng}`).join("\n"));
      }
    }
  };

  const handleClearGeometry = () => {
    stopPicking();
    clearPreviewLayer();
    setDraftGeometry(null);
    setLatitude("");
    setLongitude("");
    setManualCoordsText("");
  };

  const handleLatitudeChange = (latVal: string) => {
    setLatitude(latVal);
    const lat = parseFloat(latVal);
    const lng = parseFloat(longitude);
    if (
      !Number.isNaN(lat) &&
      !Number.isNaN(lng) &&
      lat >= -90 &&
      lat <= 90 &&
      lng >= -180 &&
      lng <= 180
    ) {
      setDraftGeometry({ type: "Point", coordinates: [lng, lat] });
    } else {
      setDraftGeometry(null);
    }
  };

  const handleLongitudeChange = (lngVal: string) => {
    setLongitude(lngVal);
    const lat = parseFloat(latitude);
    const lng = parseFloat(lngVal);
    if (
      !Number.isNaN(lat) &&
      !Number.isNaN(lng) &&
      lat >= -90 &&
      lat <= 90 &&
      lng >= -180 &&
      lng <= 180
    ) {
      setDraftGeometry({ type: "Point", coordinates: [lng, lat] });
    } else {
      setDraftGeometry(null);
    }
  };

  const handleManualCoordsChange = (textVal: string) => {
    setManualCoordsText(textVal);
    setError(null);
    try {
      const lines = textVal
        .split("\n")
        .map((l) => l.trim())
        .filter((l) => l.length > 0);
      const parsedPoints: [number, number][] = lines.map((line) => {
        const parts = line.split(/[,\s]+/).filter(Boolean);
        if (parts.length < 2) throw new Error("Format baris harus 'lat, lng'");
        const lat = parseFloat(parts[0]);
        const lng = parseFloat(parts[1]);
        if (Number.isNaN(lat) || Number.isNaN(lng)) throw new Error("Koordinat bukan angka valid");
        if (lat < -90 || lat > 90 || lng < -180 || lng > 180)
          throw new Error("Koordinat di luar rentang WGS84");
        return [lng, lat];
      });

      if (geomType === "LineString") {
        setDraftGeometry(
          parsedPoints.length >= 2 ? { type: "LineString", coordinates: parsedPoints } : null,
        );
      } else if (geomType === "Polygon") {
        if (parsedPoints.length >= 3) {
          const first = parsedPoints[0];
          const last = parsedPoints[parsedPoints.length - 1];
          if (first[0] !== last[0] || first[1] !== last[1]) {
            parsedPoints.push([first[0], first[1]]);
          }
          setDraftGeometry(
            parsedPoints.length >= 4 ? { type: "Polygon", coordinates: [parsedPoints] } : null,
          );
        } else {
          setDraftGeometry(null);
        }
      }
    } catch {
      setDraftGeometry(null);
    }
  };

  const geometrySummary = useMemo(() => {
    if (!draftGeometry) return undefined;
    if (draftGeometry.type === "Point") {
      const [lng, lat] = draftGeometry.coordinates;
      return `Titik: ${lat.toFixed(6)}, ${lng.toFixed(6)}`;
    }
    if (draftGeometry.type === "LineString") {
      return `Garis: ${draftGeometry.coordinates.length} titik koordinat`;
    }
    if (draftGeometry.type === "Polygon") {
      const ring = draftGeometry.coordinates[0] || [];
      return `Poligon: ${ring.length} titik sudut (tertutup)`;
    }
    return undefined;
  }, [draftGeometry]);

  const handleSubmit = async () => {
    if (!annotationText.trim()) {
      setError("Catatan teks anotasi tidak boleh kosong.");
      return;
    }
    if (!draftGeometry) {
      setError("Silakan tentukan koordinat atau gambar geometri di peta terlebih dahulu.");
      return;
    }

    setIsSubmitting(true);
    setError(null);
    try {
      await onSaveAnnotation({ text: annotationText.trim(), geometry: draftGeometry });
      onClose();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(`Gagal menyimpan anotasi: ${msg}`);
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleModalClose = () => {
    if (isSubmitting) return;
    stopPicking();
    clearPreviewLayer();
    onClose();
  };

  return {
    isTemporarilyClosedForMap,
    isSubmitting,
    handleModalClose,
    formProps: {
      annotationText,
      geomType,
      latitude,
      longitude,
      manualCoordsText,
      coordMode,
      isPickingLocation,
      hasGeometry: draftGeometry !== null,
      geometrySummary,
      isEditing: Boolean(initialAnnotation),
      isSubmitting,
      error,
      onTextChange: setAnnotationText,
      onGeomTypeChange: handleGeomTypeChange,
      onLatitudeChange: handleLatitudeChange,
      onLongitudeChange: handleLongitudeChange,
      onManualCoordsChange: handleManualCoordsChange,
      onCoordModeChange: handleCoordModeChange,
      onTogglePickLocation: togglePickLocation,
      onClearGeometry: handleClearGeometry,
      onSubmit: () => void handleSubmit(),
    },
  };
}
