import React, { useState } from "react";
import { Button, NonIdealState, Tooltip, Tag, InputGroup } from "@blueprintjs/core";
import type { LayerState } from "../types/layer";
import type { DroneImageMetadata } from "../types/marker";
import { LayerItem } from "../molecules/LayerItem";
import { FLIGHT_PHOTOS_LAYER_ID } from "../config/mapConfig";

export interface RasterLayerListSectionProps {
  visibleLayers: LayerState[];
  onToggleVisibility: (layer: LayerState) => void;
  onOpacityChange: (layer: LayerState, value: number) => void;
  markers?: ReadonlyArray<DroneImageMetadata>;
  hiddenPhotoIds?: ReadonlySet<string>;
  onTogglePhotoHidden?: (imageId: string) => void;
}

export const RasterLayerListSection: React.FC<RasterLayerListSectionProps> = ({
  visibleLayers,
  onToggleVisibility,
  onOpacityChange,
  markers,
  hiddenPhotoIds,
  onTogglePhotoHidden,
}) => {
  const [isPhotoListOpen, setIsPhotoListOpen] = useState(false);
  const [photoFilter, setPhotoFilter] = useState("");

  const photoList = markers ?? [];
  const hiddenPhotoCount = photoList.filter((m) => hiddenPhotoIds?.has(m.image_id)).length;
  const query = photoFilter.trim().toLowerCase();
  const filteredPhotoList = query
    ? photoList.filter((m) => m.file_name.toLowerCase().includes(query))
    : photoList;
  const showPhotoList =
    photoList.length > 0 && visibleLayers.some((l) => l.layer_id === FLIGHT_PHOTOS_LAYER_ID);

  return (
    <div>
      {/* Layer List */}
      {visibleLayers.length === 0 ? (
        <NonIdealState
          icon="layers"
          title="Tidak ada layer"
          description="Tidak ada layer dalam kategori ini"
        />
      ) : (
        visibleLayers.map((layer) => (
          <LayerItem
            key={layer.layer_id}
            layer={layer}
            onToggleVisibility={onToggleVisibility}
            onOpacityChange={onOpacityChange}
          />
        ))
      )}

      {/* Daftar Per-Foto (Anak Layer Flight Photos) */}
      {showPhotoList && (
        <div
          style={{
            marginBottom: "16px",
            padding: "12px 16px 16px",
            background: "var(--color-gallery-white)",
            borderRadius: "var(--radius-cards)",
            boxShadow: "var(--shadow-subtle)",
          }}
        >
          <div
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              gap: "8px",
            }}
          >
            <Button
              minimal
              small
              icon={isPhotoListOpen ? "chevron-down" : "chevron-right"}
              text={`Foto Penerbangan (${photoList.length})`}
              style={{ fontWeight: 600, paddingLeft: 0 }}
              onClick={() => setIsPhotoListOpen((open) => !open)}
            />
            {hiddenPhotoCount > 0 && (
              <Tag
                minimal
                round
                style={{
                  background: "var(--color-control-gray)",
                  color: "var(--color-slate)",
                  fontSize: "10px",
                  fontWeight: 600,
                }}
              >
                {hiddenPhotoCount} disembunyikan
              </Tag>
            )}
          </div>

          {isPhotoListOpen && (
            <>
              <InputGroup
                small
                fill
                leftIcon="search"
                placeholder="Filter nama file…"
                value={photoFilter}
                onChange={(event) => setPhotoFilter(event.target.value)}
                style={{ margin: "8px 0" }}
              />
              {hiddenPhotoCount > 0 && (
                <Button
                  minimal
                  small
                  text="Tampilkan semua"
                  onClick={() => {
                    for (const photo of photoList) {
                      if (hiddenPhotoIds?.has(photo.image_id)) {
                        onTogglePhotoHidden?.(photo.image_id);
                      }
                    }
                  }}
                  style={{ marginBottom: "6px" }}
                />
              )}
              <div style={{ maxHeight: "280px", overflowY: "auto" }}>
                {filteredPhotoList.length === 0 ? (
                  <div style={{ fontSize: "11px", color: "var(--color-slate)" }}>
                    Tidak ada foto yang cocok.
                  </div>
                ) : (
                  filteredPhotoList.map((photo) => {
                    const isHidden = hiddenPhotoIds?.has(photo.image_id) ?? false;
                    return (
                      <div
                        key={photo.image_id}
                        style={{
                          display: "flex",
                          alignItems: "center",
                          justifyContent: "space-between",
                          gap: "8px",
                          padding: "4px 0",
                          borderBottom: "1px solid var(--color-hairline-silver)",
                        }}
                      >
                        <span
                          title={photo.file_name}
                          style={{
                            flex: 1,
                            overflow: "hidden",
                            textOverflow: "ellipsis",
                            whiteSpace: "nowrap",
                            fontSize: "11px",
                            fontFamily: "monospace",
                            color: isHidden ? "var(--color-slate)" : "var(--color-ink)",
                          }}
                        >
                          {photo.file_name}
                        </span>
                        <Tooltip
                          content={
                            isHidden ? "Tampilkan foto di peta" : "Sembunyikan foto dari peta"
                          }
                          placement="top"
                        >
                          <Button
                            minimal
                            small
                            icon={isHidden ? "eye-off" : "eye-open"}
                            style={{
                              color: isHidden ? "var(--color-slate)" : "var(--color-apple-blue)",
                            }}
                            onClick={() => onTogglePhotoHidden?.(photo.image_id)}
                          />
                        </Tooltip>
                      </div>
                    );
                  })
                )}
              </div>
            </>
          )}
        </div>
      )}
    </div>
  );
};
