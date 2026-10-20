import React, { useState } from "react";
import { Icon } from "@blueprintjs/core";

export const MapSearchBar: React.FC<{ onMenuClick?: () => void }> = ({ onMenuClick }) => {
  const [query, setQuery] = useState("");

  const [isFocused, setIsFocused] = useState(false);

  return (
    <div
      style={{
        width: "100%",
        padding: "10px 16px",
        borderRadius: "999px",
        background: "var(--color-gallery-white)",
        boxShadow: isFocused
          ? "0 4px 20px rgba(0, 102, 204, 0.15), var(--shadow-subtle)"
          : "0 4px 12px rgba(0, 0, 0, 0.08), var(--shadow-subtle)",
        display: "flex",
        alignItems: "center",
        fontFamily: "var(--font-sf-pro-text)",
        border: "1px solid var(--color-hairline-silver)",
        transition: "box-shadow 0.2s cubic-bezier(0.4, 0, 0.2, 1)",
      }}
    >
      <div
        role="button"
        tabIndex={0}
        onClick={onMenuClick}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onMenuClick?.();
          }
        }}
        style={{
          cursor: "pointer",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--color-slate)",
          padding: "4px",
        }}
      >
        <Icon icon="menu" size={18} />
      </div>

      <input
        type="text"
        placeholder="Search Aerial Analytics..."
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onFocus={() => setIsFocused(true)}
        onBlur={() => setIsFocused(false)}
        style={{
          border: "none",
          outline: "none",
          background: "transparent",
          fontSize: "15px",
          width: "100%",
          padding: "0 14px",
          color: "var(--color-ink)",
          fontFamily: "var(--font-sf-pro-text)",
          fontWeight: 400,
        }}
      />
    </div>
  );
};
