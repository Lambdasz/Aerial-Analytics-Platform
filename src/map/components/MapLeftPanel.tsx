import React from "react";
import { ImageGallerySidebar } from "./ImageGallerySidebar";
import { LayerManager } from "./LayerManager";
import { AoiDemoToolbar } from "./AoiDemoToolbar";
import { MapSearchBar } from "./MapSearchBar";
import L from "leaflet";

import type { LayerState, AnnotationFeature } from "../types/layer";

interface MapLeftPanelProps {
  map: L.Map | null;
  onAnnotationsChange?: (annotations: AnnotationFeature[]) => void;
  onLayersChange?: (layers: LayerState[]) => void;
  isCollapsed: boolean;
  setIsCollapsed: (collapsed: boolean) => void;
}

export const MapLeftPanel: React.FC<MapLeftPanelProps> = ({
  map,
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
    window.addEventListener("open-gallery-tab", handleOpenGallery);
    return () => window.removeEventListener("open-gallery-tab", handleOpenGallery);
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
        zIndex: 1000,
      }}
    >
      {/* SEARCH BAR (SELALU MUNCUL, FLOATING OVER MAP JIKA COLLAPSED) */}
      <div
        style={{
          position: "absolute",
          top: "15px",
          left: "15px",
          width: "350px", // Tetap 350px walau panel 0px
          zIndex: 1001,
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
          background: "var(--color-gallery-white)",
          borderRight: "1px solid var(--color-hairline-silver)",
          display: "flex",
          flexDirection: "column",
          paddingTop: "70px" /* Spacer untuk Search Bar */,
          opacity: isCollapsed ? 0 : 1,
          pointerEvents: isCollapsed ? "none" : "auto",
          transition: "opacity 0.2s",
          visibility: isCollapsed ? "hidden" : "visible",
          boxShadow: isCollapsed ? "none" : "var(--shadow-subtle)",
          overflow: "hidden",
          fontFamily: "var(--font-sf-pro-text)",
        }}
      >
        <div
          style={{
            padding: "0 15px",
            borderBottom: "1px solid var(--color-hairline-silver)",
            background: "var(--color-gallery-white)",
          }}
        >
          <div
            style={{
              display: "flex",
              gap: "8px",
              background: "var(--color-control-gray)",
              padding: "4px",
              borderRadius: "12px",
              margin: "12px 0",
            }}
          >
            {[
              { id: "layers", label: "Layers" },
              { id: "aoi", label: "AOI Tools" },
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
                  padding: "6px 12px",
                  borderRadius: "8px",
                  fontSize: "13px",
                  fontWeight: activeTab === tab.id ? 600 : 500,
                  color: activeTab === tab.id ? "var(--color-ink)" : "var(--color-slate)",
                  background: activeTab === tab.id ? "var(--color-gallery-white)" : "transparent",
                  boxShadow: activeTab === tab.id ? "0 2px 4px rgba(0,0,0,0.1)" : "none",
                  cursor: "pointer",
                  transition: "all 0.2s cubic-bezier(0.4, 0, 0.2, 1)",
                }}
              >
                {tab.label}
              </div>
            ))}
          </div>
        </div>

        <div
          className="no-scrollbar"
          style={{
            flex: 1,
            overflowY: "auto",
            position: "relative",
            padding: "15px",
            color: "var(--color-ink)",
            background: "var(--color-studio-mist)",
          }}
        >
          {activeTab === "layers" && (
            <LayerManager
              onAnnotationsChange={onAnnotationsChange}
              onLayersChange={onLayersChange}
            />
          )}
          {activeTab === "aoi" && <AoiDemoToolbar map={map} />}
          {activeTab === "gallery" && <ImageGallerySidebar />}
        </div>
      </div>
    </div>
  );
};
