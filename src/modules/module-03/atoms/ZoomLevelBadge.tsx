import React from "react";

export interface ZoomLevelBadgeProps {
  zoom: number;
}

export const ZoomLevelBadge: React.FC<ZoomLevelBadgeProps> = ({ zoom }) => {
  return <div className="zoom-indicator">{zoom}z</div>;
};
