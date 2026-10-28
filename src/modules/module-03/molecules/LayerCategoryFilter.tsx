import React from "react";
import { HTMLSelect } from "@blueprintjs/core";

export interface LayerCategoryFilterProps {
  categories: string[];
  selectedCategory: string;
  onSelectCategory: (category: string) => void;
}

export const LayerCategoryFilter: React.FC<LayerCategoryFilterProps> = ({
  categories,
  selectedCategory,
  onSelectCategory,
}) => {
  return (
    <div style={{ marginBottom: "16px" }}>
      <div
        style={{
          fontSize: "12px",
          fontWeight: 600,
          marginBottom: "6px",
        }}
      >
        Filter Kategori
      </div>

      <HTMLSelect
        fill
        className="apple-select"
        value={selectedCategory}
        onChange={(event) => {
          onSelectCategory(event.target.value);
        }}
      >
        {categories.map((category) => (
          <option key={category} value={category}>
            {category}
          </option>
        ))}
      </HTMLSelect>
    </div>
  );
};
