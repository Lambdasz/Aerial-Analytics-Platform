import React from "react";
import { Button } from "@blueprintjs/core";

export interface TactileToggleProps {
  isChecked: boolean;
  onToggle: () => void;
  labelOn?: string;
  labelOff?: string;
  disabled?: boolean;
}

export const TactileToggle: React.FC<TactileToggleProps> = ({
  isChecked,
  onToggle,
  labelOn = "ON",
  labelOff = "OFF",
  disabled = false,
}) => {
  return (
    <Button
      text={isChecked ? labelOn : labelOff}
      small
      minimal
      disabled={disabled}
      style={{
        borderRadius: "999px",
        fontWeight: 600,
        fontSize: "11px",
        padding: "4px 12px",
        background: isChecked ? "#0f172a" : "#f1f5f9",
        color: isChecked ? "#ffffff" : "#64748b",
        boxShadow: "none",
        outline: "none",
      }}
      onClick={onToggle}
    />
  );
};
