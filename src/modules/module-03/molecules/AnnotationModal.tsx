import React, { useState, useEffect, useRef, useMemo } from "react";
import * as L from "leaflet";
import "../leaflet-setup";
import "@geoman-io/leaflet-geoman-free";
import { Dialog, Classes } from "@blueprintjs/core";
import type { AnnotationFeature, SpatialGeometry } from "../types/layer";
import { AnnotationForm, type AnnotationGeomType } from "./AnnotationForm";
import { latLngsToLineString, latLngsToPolygon } from "../lib/geoJson";

export interface AnnotationModalProps {
  isOpen: boolean;
  onClose: () => void;
  map: L.Map | null;
  initialAnnotation?: AnnotationFeature | null;
  onSaveAnnotation: (args: { text: string; geometry: SpatialGeometry }) => Promise<void>;
  onDrawingStateChange?: (isPicking: boolean, geomType: AnnotationGeomType) => void;
}

/**
 * Modal dialog terpisah untuk pembuatan dan pengeditan Anotasi Spasial.
 * Menjamin sidebar tetap bersih dan lega, serta mendukung workflow interaksi peta:
 * saat pengguna memilih titik atau menggambar garis/poligon di peta, dialog tertutup sementara,
 * dan otomatis terbuka kembali saat penggambaran selesai.
 */
export const AnnotationModal: React.FC<AnnotationModalProps> = ({
  isOpen,
  onClose,
  map,
  initialAnnotation,
  onSaveAnnotation,
  onDrawingStateChange,
}) => {
  const [annotationText, setAnnotationText] = useState("");
  const [geomType, setGeomType] = useState<AnnotationGeomType>("Point");
  const [latitude, setLatitude] = useState("");
  const [longitude, setLongitude] = useState("");
  const [manualCoordsText, setManualCoordsText] = useState("");
  const [draftGeometry, setDraftGeometry] = useState<SpatialGeometry | null>(null);
  const [coordMode, setCoordMode] = useState<"map" | "manual">("map");
  const [isPickingLocation, setIsPickingLocation] = useState(false);
  const [isTemporarilyClosedForMap, setIsTemporarilyClosedForMap] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Ref layer pratinjau sementara di peta Leaflet
  const previewLayerRef = useRef<L.Layer | null>(null);

  // Sinkronisasi state saat prop isOpen atau initialAnnotation berganti (state during render)
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
      setIsPickingLocation(false);
      setIsTemporarilyClosedForMap(false);
      setError(null);
    } else {
      setIsPickingLocation(false);
      setIsTemporarilyClosedForMap(false);
    }
  }

  // Bersihkan interaksi peta eksternal saat modal ditutup
  useEffect(() => {
    if (!isOpen) {
      if (previewLayerRef.current && map) {
        map.removeLayer(previewLayerRef.current);
        previewLayerRef.current = null;
      }
      if (map) {
        map.pm?.disableDraw?.();
        map.getContainer().style.cursor = "";
      }
    }
  }, [isOpen, map]);

  // Mode Titik: Listener klik pada peta
  useEffect(() => {
    if (!map || !isPickingLocation || geomType !== "Point") return;

    const container = map.getContainer();
    const prevCursor = container.style.cursor;
    container.style.cursor = "crosshair";

    const handleMapClick = (e: L.LeafletMouseEvent) => {
      const lat = Number(e.latlng.lat.toFixed(6));
      const lng = Number(e.latlng.lng.toFixed(6));
      setLatitude(lat.toString());
      setLongitude(lng.toString());
      setDraftGeometry({ type: "Point", coordinates: [lng, lat] });
      setIsPickingLocation(false);
      setIsTemporarilyClosedForMap(false);
      onDrawingStateChange?.(false, "Point");
    };

    map.on("click", handleMapClick);

    return () => {
      container.style.cursor = prevCursor;
      map.off("click", handleMapClick);
    };
  }, [map, isPickingLocation, geomType, onDrawingStateChange]);

  // Mode Garis & Poligon: Listener Geoman pm:create
  useEffect(() => {
    if (!map) return;

    const handlePmCreate = (e: { shape: string; layer: L.Layer }) => {
      if (!isPickingLocation) return;

      if (geomType === "LineString" && (e.shape === "Line" || e.layer instanceof L.Polyline)) {
        const latLngs = (e.layer as L.Polyline).getLatLngs();
        const coords = latLngsToLineString(latLngs);
        if (coords.length >= 2) {
          setDraftGeometry({ type: "LineString", coordinates: coords });
          if (previewLayerRef.current && previewLayerRef.current !== e.layer) {
            map.removeLayer(previewLayerRef.current);
          }
          previewLayerRef.current = e.layer;
          setIsPickingLocation(false);
          setIsTemporarilyClosedForMap(false);
          onDrawingStateChange?.(false, "LineString");
          map.pm?.disableDraw?.();
          setError(null);
        }
      } else if (
        geomType === "Polygon" &&
        (e.shape === "Polygon" || e.shape === "Rectangle" || e.layer instanceof L.Polygon)
      ) {
        const latLngs = (e.layer as L.Polygon).getLatLngs();
        const poly = latLngsToPolygon(latLngs);
        if (poly.length > 0 && poly[0].length >= 4) {
          setDraftGeometry({ type: "Polygon", coordinates: poly });
          if (previewLayerRef.current && previewLayerRef.current !== e.layer) {
            map.removeLayer(previewLayerRef.current);
          }
          previewLayerRef.current = e.layer;
          setIsPickingLocation(false);
          setIsTemporarilyClosedForMap(false);
          onDrawingStateChange?.(false, "Polygon");
          map.pm?.disableDraw?.();
          setError(null);
        }
      }
    };

    map.on("pm:create", handlePmCreate as unknown as L.LeafletEventHandlerFn);

    return () => {
      map.off("pm:create", handlePmCreate as unknown as L.LeafletEventHandlerFn);
    };
  }, [map, isPickingLocation, geomType, onDrawingStateChange]);

  // Sinkronisasi layer pratinjau sementara di peta saat koordinat berubah
  useEffect(() => {
    if (!map) return;

    if (isPickingLocation && (geomType === "LineString" || geomType === "Polygon")) {
      return;
    }

    if (!draftGeometry) {
      if (previewLayerRef.current) {
        map.removeLayer(previewLayerRef.current);
        previewLayerRef.current = null;
      }
      return;
    }

    if (
      previewLayerRef.current &&
      (geomType === "LineString" || geomType === "Polygon") &&
      coordMode === "map"
    ) {
      return;
    }

    if (previewLayerRef.current) {
      map.removeLayer(previewLayerRef.current);
      previewLayerRef.current = null;
    }

    if (draftGeometry.type === "Point") {
      const [lng, lat] = draftGeometry.coordinates;
      const previewIcon = L.divIcon({
        className: "annotation-preview-marker",
        html: `<div style="
          width: 24px;
          height: 24px;
          background: #f59e0b;
          border: 2.5px solid #ffffff;
          border-radius: 50%;
          box-shadow: 0 2px 8px rgba(0,0,0,0.4);
          display: flex;
          align-items: center;
          justify-content: center;
        ">
          <div style="width: 6px; height: 6px; background: white; border-radius: 50%;"></div>
        </div>`,
        iconSize: [24, 24],
        iconAnchor: [12, 12],
      });

      const marker = L.marker([lat, lng], {
        icon: previewIcon,
        zIndexOffset: 1000,
      }).addTo(map);

      marker.bindTooltip("Titik Anotasi Baru", {
        permanent: false,
        direction: "top",
        offset: [0, -12],
      });

      previewLayerRef.current = marker;
    } else if (draftGeometry.type === "LineString") {
      const positions: [number, number][] = draftGeometry.coordinates.map(([lng, lat]) => [
        lat,
        lng,
      ]);
      const polyline = L.polyline(positions, {
        color: "#f59e0b",
        weight: 4,
        dashArray: "6, 8",
        opacity: 0.95,
      }).addTo(map);
      previewLayerRef.current = polyline;
    } else if (draftGeometry.type === "Polygon") {
      const ring = (draftGeometry.coordinates[0] || []) as [number, number][];
      const positions: [number, number][] = ring.map(([lng, lat]) => [lat, lng]);
      const polygon = L.polygon(positions, {
        color: "#f59e0b",
        fillColor: "#f59e0b",
        fillOpacity: 0.25,
        weight: 2.5,
        dashArray: "5, 5",
      }).addTo(map);
      previewLayerRef.current = polygon;
    }
  }, [map, draftGeometry, isPickingLocation, geomType, coordMode]);

  // Event listener eksternal untuk pembatalan atau penyelesaian picking dari sidebar banner
  useEffect(() => {
    const handleCancelPicking = () => {
      if (map) {
        map.pm?.disableDraw?.();
        map.getContainer().style.cursor = "";
      }
      setIsPickingLocation(false);
      setIsTemporarilyClosedForMap(false);
      onDrawingStateChange?.(false, geomType);
    };

    const handleFinishPicking = () => {
      if (map) {
        map.pm?.disableDraw?.();
        map.getContainer().style.cursor = "";
      }
      setIsPickingLocation(false);
      setIsTemporarilyClosedForMap(false);
      onDrawingStateChange?.(false, geomType);
    };

    window.addEventListener("annotation:cancel-picking", handleCancelPicking);
    window.addEventListener("annotation:finish-picking", handleFinishPicking);

    return () => {
      window.removeEventListener("annotation:cancel-picking", handleCancelPicking);
      window.removeEventListener("annotation:finish-picking", handleFinishPicking);
    };
  }, [map, geomType, onDrawingStateChange]);

  // Bersihkan preview layer saat unmount
  useEffect(() => {
    return () => {
      if (previewLayerRef.current && map) {
        map.removeLayer(previewLayerRef.current);
        previewLayerRef.current = null;
      }
      if (map) {
        map.pm?.disableDraw?.();
        map.getContainer().style.cursor = "";
      }
    };
  }, [map]);

  const handleGeomTypeChange = (newType: AnnotationGeomType) => {
    if (isPickingLocation && map) {
      map.pm?.disableDraw?.();
      map.getContainer().style.cursor = "";
    }
    if (previewLayerRef.current && map) {
      map.removeLayer(previewLayerRef.current);
      previewLayerRef.current = null;
    }
    setIsPickingLocation(false);
    setIsTemporarilyClosedForMap(false);
    onDrawingStateChange?.(false, newType);
    setGeomType(newType);
    setDraftGeometry(null);
    setLatitude("");
    setLongitude("");
    setManualCoordsText("");
    setError(null);
  };

  const handleCoordModeChange = (newMode: "map" | "manual") => {
    if (isPickingLocation && map) {
      map.pm?.disableDraw?.();
      map.getContainer().style.cursor = "";
    }
    setIsPickingLocation(false);
    setIsTemporarilyClosedForMap(false);
    onDrawingStateChange?.(false, geomType);
    setCoordMode(newMode);
    setError(null);

    // Sinkronkan koordinat jika draftGeometry sudah ada agar tidak hilang saat berpindah tab
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
    if (isPickingLocation && map) {
      map.pm?.disableDraw?.();
      map.getContainer().style.cursor = "";
    }
    if (previewLayerRef.current && map) {
      map.removeLayer(previewLayerRef.current);
      previewLayerRef.current = null;
    }
    setIsPickingLocation(false);
    setIsTemporarilyClosedForMap(false);
    onDrawingStateChange?.(false, geomType);
    setDraftGeometry(null);
    setLatitude("");
    setLongitude("");
    setManualCoordsText("");
  };

  const handleTogglePickLocation = () => {
    if (!map) {
      setError("Peta belum siap untuk menggambar atau memilih lokasi.");
      return;
    }
    setError(null);

    if (isPickingLocation) {
      setIsPickingLocation(false);
      setIsTemporarilyClosedForMap(false);
      onDrawingStateChange?.(false, geomType);
      map.pm?.disableDraw?.();
      map.getContainer().style.cursor = "";
      return;
    }

    if (previewLayerRef.current) {
      map.removeLayer(previewLayerRef.current);
      previewLayerRef.current = null;
    }
    setDraftGeometry(null);

    // Tutup modal sementara dan aktifkan listener Leaflet / Geoman
    setIsPickingLocation(true);
    setIsTemporarilyClosedForMap(true);
    onDrawingStateChange?.(true, geomType);

    if (geomType === "Point") {
      // Map on click listener akan aktif via effect
    } else if (geomType === "LineString") {
      map.pm?.enableDraw?.("Line", {
        pathOptions: {
          color: "#f59e0b",
          weight: 4,
          opacity: 0.95,
          lineCap: "round",
          lineJoin: "round",
          dashArray: "6, 8",
        },
      });
    } else if (geomType === "Polygon") {
      map.pm?.enableDraw?.("Polygon", {
        pathOptions: {
          color: "#f59e0b",
          fillColor: "#f59e0b",
          fillOpacity: 0.25,
          weight: 2.5,
          dashArray: "5, 5",
        },
      });
    }
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
        if (lat < -90 || lat > 90 || lng < -180 || lng > 180) {
          throw new Error("Koordinat di luar rentang WGS84");
        }
        return [lng, lat];
      });

      if (geomType === "LineString") {
        if (parsedPoints.length >= 2) {
          setDraftGeometry({ type: "LineString", coordinates: parsedPoints });
        } else {
          setDraftGeometry(null);
        }
      } else if (geomType === "Polygon") {
        if (parsedPoints.length >= 3) {
          const first = parsedPoints[0];
          const last = parsedPoints[parsedPoints.length - 1];
          if (first[0] !== last[0] || first[1] !== last[1]) {
            parsedPoints.push([first[0], first[1]]);
          }
          if (parsedPoints.length >= 4) {
            setDraftGeometry({ type: "Polygon", coordinates: [parsedPoints] });
          } else {
            setDraftGeometry(null);
          }
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
      await onSaveAnnotation({
        text: annotationText.trim(),
        geometry: draftGeometry,
      });
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
    if (previewLayerRef.current && map) {
      map.removeLayer(previewLayerRef.current);
      previewLayerRef.current = null;
    }
    if (map) {
      map.pm?.disableDraw?.();
      map.getContainer().style.cursor = "";
    }
    setIsPickingLocation(false);
    setIsTemporarilyClosedForMap(false);
    onDrawingStateChange?.(false, geomType);
    onClose();
  };

  return (
    <Dialog
      isOpen={isOpen && !isTemporarilyClosedForMap}
      onClose={handleModalClose}
      canEscapeKeyClose={!isSubmitting}
      canOutsideClickClose={!isSubmitting}
      style={{
        width: "480px",
        maxWidth: "92vw",
        borderRadius: "16px",
        overflow: "hidden",
      }}
    >
      <div className={Classes.DIALOG_BODY} style={{ padding: "24px 24px 16px 24px" }}>
        <AnnotationForm
          annotationText={annotationText}
          geomType={geomType}
          latitude={latitude}
          longitude={longitude}
          manualCoordsText={manualCoordsText}
          coordMode={coordMode}
          isPickingLocation={isPickingLocation}
          hasGeometry={draftGeometry !== null}
          geometrySummary={geometrySummary}
          isEditing={Boolean(initialAnnotation)}
          isSubmitting={isSubmitting}
          error={error}
          onTextChange={setAnnotationText}
          onGeomTypeChange={handleGeomTypeChange}
          onLatitudeChange={handleLatitudeChange}
          onLongitudeChange={handleLongitudeChange}
          onManualCoordsChange={handleManualCoordsChange}
          onCoordModeChange={handleCoordModeChange}
          onTogglePickLocation={handleTogglePickLocation}
          onClearGeometry={handleClearGeometry}
          onSubmit={() => void handleSubmit()}
          onCancel={handleModalClose}
        />
      </div>
    </Dialog>
  );
};
