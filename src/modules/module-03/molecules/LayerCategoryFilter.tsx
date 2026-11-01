import React from "react";

export interface LayerCategoryFilterProps {
  categories: string[];
  selectedCategory: string;
  onSelectCategory: (category: string) => void;
}

function formatCategoryLabel(category: string): string {
  switch (category.toLowerCase()) {
    case "all":
      return "Semua";
    case "image_overlay":
      return "Citra";
    case "raster":
      return "Raster";
    case "polygon":
      return "Poligon";
    case "spatial_result":
      return "Hasil Analisis";
    default:
      return category.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
  }
}

export const LayerCategoryFilter: React.FC<LayerCategoryFilterProps> = ({
  categories,
  selectedCategory,
  onSelectCategory,
}) => {
  return (
    <div
      style={{
        display: "flex",
        gap: "3px",
        background: "#f1f5f9",
        padding: "3px",
        borderRadius: "8px",
        marginBottom: "12px",
      }}
    >
      {categories.map((category) => {
        const isActive = selectedCategory === category;
        const label = formatCategoryLabel(category);
        return (
          <button
            key={category}
            type="button"
            onClick={() => onSelectCategory(category)}
            style={{
              flex: 1,
              textAlign: "center",
              padding: "5px 4px",
              borderRadius: "6px",
              fontSize: "11px",
              fontWeight: isActive ? 600 : 500,
              cursor: "pointer",
              border: "none",
              outline: "none",
              background: isActive ? "#ffffff" : "transparent",
              color: isActive ? "#0f172a" : "#64748b",
              boxShadow: isActive ? "0 1px 3px rgba(0, 0, 0, 0.08)" : "none",
              transition: "all 0.15s ease",
              whiteSpace: "nowrap",
              userSelect: "none",
            }}
          >
            {label}
          </button>
        );
      })}
    </div>
  );
};
