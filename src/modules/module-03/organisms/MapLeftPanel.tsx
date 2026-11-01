import React from "react";
import { ImageGallerySidebar } from "./ImageGallerySidebar";
import { LayerManager } from "./LayerManager";
import { AoiToolbar } from "./AoiToolbar";
import { SpResultListPanel } from "./SpResultListPanel";
import { PlotAnalyticsPanel } from "../../module-09/PlotAnalyticsPanel";
import { MapSearchBar } from "../molecules/MapSearchBar";
import L from "leaflet";

import type { LayerState, AnnotationFeature } from "../types/layer";
import type { DroneImageMetadata } from "../types/marker";

interface MapLeftPanelProps {
  map: L.Map | null;
  markers: ReadonlyArray<DroneImageMetadata>;
  /** image_id yang disembunyikan per-foto (panel Layers) */
  hiddenPhotoIds?: ReadonlySet<string>;
  onTogglePhotoHidden?: (imageId: string) => void;
  onAnnotationsChange?: (annotations: AnnotationFeature[]) => void;
  onLayersChange?: (layers: LayerState[]) => void;
  isCollapsed: boolean;
  setIsCollapsed: (collapsed: boolean) => void;
}

export const MapLeftPanel: React.FC<MapLeftPanelProps> = ({
  map,
  markers,
  hiddenPhotoIds,
  onTogglePhotoHidden,
  onAnnotationsChange,
  onLayersChange,
  isCollapsed,
  setIsCollapsed,
}) => {
  const [activeTab, setActiveTab] = React.useState<string>("layers");

  React.useEffect(() => {
    const handleOpenGallery = () => {
      setActiveTab("gallery");
      setIsCollapsed(false);
    };
    const handleOpenResults = () => {
      setActiveTab("results");
      setIsCollapsed(false);
    };
    window.addEventListener("open-gallery-tab", handleOpenGallery);
    window.addEventListener("open-results-tab", handleOpenResults);
    return () => {
      window.removeEventListener("open-gallery-tab", handleOpenGallery);
      window.removeEventListener("open-results-tab", handleOpenResults);
    };
  }, [setIsCollapsed]);

  return (
    <div
      style={{
        position: "relative",
        width: isCollapsed ? "0px" : "380px",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        transition: "width 0.3s cubic-bezier(0.4, 0.0, 0.2, 1)",
        zIndex: 500,
      }}
    >
      {/* SEARCH BAR (SELALU MUNCUL, FLOATING OVER MAP JIKA COLLAPSED) */}
      <div
        style={{
          position: "absolute",
          top: "15px",
          left: "15px",
          width: "350px", // Tetap 350px walau panel 0px
          zIndex: 501,
          pointerEvents: "auto",
        }}
      >
        <MapSearchBar onMenuClick={() => setIsCollapsed(!isCollapsed)} />
      </div>

      {/* ISI SIDEBAR (TABS & KONTEN) */}
      <div
        style={{
          width: "380px",
          height: "100%",
          background: "linear-gradient(180deg, #f8fafc 0%, #ffffff 100%)",
          borderRight: "1px solid rgba(0, 0, 0, 0.06)",
          display: "flex",
          flexDirection: "column",
          paddingTop: "70px" /* Spacer untuk Search Bar */,
          opacity: isCollapsed ? 0 : 1,
          pointerEvents: isCollapsed ? "none" : "auto",
          transition: "opacity 0.2s",
          visibility: isCollapsed ? "hidden" : "visible",
          boxShadow: isCollapsed ? "none" : "4px 0 24px -4px rgba(0, 0, 0, 0.05)",
          overflow: "hidden",
          fontFamily: "var(--font-sf-pro-text)",
        }}
      >
        <div
          style={{
            padding: "0 15px",
            background: "transparent",
          }}
        >
          <div
            style={{
              display: "flex",
              gap: "4px",
              background: "rgba(241, 245, 249, 0.8)",
              padding: "4px",
              borderRadius: "999px",
              margin: "10px 0 14px 0",
              border: "1px solid rgba(226, 232, 240, 0.8)",
            }}
          >
            {[
              { id: "layers", label: "Layers" },
              { id: "aoi", label: "AOI" },
              { id: "results", label: "Hasil Analisis" },
              { id: "plots", label: "Plots" },
              { id: "gallery", label: "Gallery" },
            ].map((tab) => (
              <div
                key={tab.id}
                role="button"
                tabIndex={0}
                onClick={() => setActiveTab(tab.id)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    setActiveTab(tab.id);
                  }
                }}
                style={{
                  flex: 1,
                  textAlign: "center",
                  padding: "6px 8px",
                  borderRadius: "999px",
                  fontSize: "12px",
                  fontWeight: activeTab === tab.id ? 600 : 500,
                  color: activeTab === tab.id ? "#0f172a" : "#64748b",
                  background: activeTab === tab.id ? "#ffffff" : "transparent",
                  boxShadow:
                    activeTab === tab.id
                      ? "0 2px 8px -1px rgba(0, 0, 0, 0.08), 0 1px 2px rgba(0, 0, 0, 0.04)"
                      : "none",
                  cursor: "pointer",
                  transition: "all 0.2s cubic-bezier(0.4, 0, 0.2, 1)",
                  whiteSpace: "nowrap",
                  outline: "none",
                  userSelect: "none",
                }}
              >
                {tab.label}
              </div>
            ))}
          </div>
        </div>

        <div
          style={{
            flex: 1,
            position: "relative",
            overflow: "hidden",
            color: "var(--color-ink)",
            background: "var(--color-studio-mist)",
          }}
        >
          <div
            className="no-scrollbar"
            style={{
              display: activeTab === "layers" ? "block" : "none",
              height: "100%",
              overflowY: "auto",
              padding: "15px",
            }}
          >
            <LayerManager
              map={map}
              markers={markers}
              hiddenPhotoIds={hiddenPhotoIds}
              onTogglePhotoHidden={onTogglePhotoHidden}
              onAnnotationsChange={onAnnotationsChange}
              onLayersChange={onLayersChange}
            />
          </div>
          <div
            className="no-scrollbar"
            style={{
              display: activeTab === "aoi" ? "block" : "none",
              height: "100%",
              overflowY: "auto",
              padding: "15px",
            }}
          >
            <AoiToolbar map={map} />
          </div>
          <div
            className="no-scrollbar"
            style={{
              display: activeTab === "results" ? "block" : "none",
              height: "100%",
              overflowY: "auto",
              padding: "15px",
            }}
          >
            <SpResultListPanel map={map} />
          </div>
          <div
            className="no-scrollbar"
            style={{
              display: activeTab === "plots" ? "block" : "none",
              height: "100%",
              overflowY: "auto",
              padding: "15px",
            }}
          >
            <PlotAnalyticsPanel />
          </div>
          <div
            className="no-scrollbar"
            style={{
              display: activeTab === "gallery" ? "block" : "none",
              height: "100%",
              overflowY: "auto",
              padding: "15px",
            }}
          >
            <ImageGallerySidebar map={map} markers={markers} />
          </div>
        </div>
      </div>
    </div>
  );
};
