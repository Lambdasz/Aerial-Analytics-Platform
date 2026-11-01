import React, { useState } from "react";
import { Icon } from "@blueprintjs/core";

export const MapSearchBar: React.FC<{ onMenuClick?: () => void }> = ({ onMenuClick }) => {
  const [query, setQuery] = useState("");

  const [isFocused, setIsFocused] = useState(false);

  return (
    <div
      style={{
        width: "100%",
        padding: "8px 14px",
        borderRadius: "999px",
        background: "rgba(255, 255, 255, 0.85)",
        backdropFilter: "blur(12px)",
        WebkitBackdropFilter: "blur(12px)",
        boxShadow: isFocused
          ? "0 8px 24px -4px rgba(0, 102, 204, 0.16), 0 2px 6px rgba(0, 0, 0, 0.04)"
          : "0 4px 16px -2px rgba(0, 0, 0, 0.06), 0 1px 3px rgba(0, 0, 0, 0.04)",
        display: "flex",
        alignItems: "center",
        fontFamily: "var(--font-sf-pro-text)",
        border: "1px solid rgba(255, 255, 255, 0.6)",
        transition: "all 0.2s cubic-bezier(0.4, 0, 0.2, 1)",
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
          outline: "none",
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
