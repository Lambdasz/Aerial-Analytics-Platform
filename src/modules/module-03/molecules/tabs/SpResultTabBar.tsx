import React from "react";

export type SpResultTabKey = "general" | "fields" | "values" | "preview";

export interface SpResultTabBarProps {
  selectedTab: SpResultTabKey;
  onSelectTab: (tab: SpResultTabKey) => void;
  fieldsCount: number;
}

interface TabItem {
  key: SpResultTabKey;
  label: string;
}

export const SpResultTabBar: React.FC<SpResultTabBarProps> = ({
  selectedTab,
  onSelectTab,
  fieldsCount,
}) => {
  const tabs: TabItem[] = [
    { key: "general", label: "Umum & Visual" },
    { key: "fields", label: `Skema Field (${fieldsCount})` },
    { key: "values", label: "Nilai Atribut Fitur" },
    { key: "preview", label: "Pratinjau Popup" },
  ];

  return (
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
      {tabs.map((tab) => {
        const isActive = selectedTab === tab.key;
        return (
          <button
            key={tab.key}
            type="button"
            onClick={() => onSelectTab(tab.key)}
            style={{
              flex: 1,
              padding: "8px 10px",
              fontSize: "12px",
              fontWeight: isActive ? 600 : 500,
              color: isActive ? "#0f172a" : "#64748b",
              background: isActive ? "#ffffff" : "transparent",
              border: "none",
              outline: "none",
              borderRadius: "6px",
              boxShadow: isActive ? "0 1px 3px rgba(0,0,0,0.08)" : "none",
              cursor: "pointer",
              transition: "all 0.15s ease",
              whiteSpace: "nowrap",
              userSelect: "none",
            }}
          >
            {tab.label}
          </button>
        );
      })}
    </div>
  );
};
