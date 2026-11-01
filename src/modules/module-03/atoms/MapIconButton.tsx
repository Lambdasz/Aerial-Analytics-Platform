import React from "react";
import { Button, Tooltip } from "@blueprintjs/core";
import type { IconName, Placement } from "@blueprintjs/core";

export interface MapIconButtonProps {
  icon: IconName;
  tooltip: string;
  onClick: () => void;
  placement?: Placement;
}

export const MapIconButton: React.FC<MapIconButtonProps> = ({
  icon,
  tooltip,
  onClick,
  placement = "left",
}) => {
  return (
    <Tooltip content={tooltip} placement={placement}>
      <Button icon={icon} minimal onClick={onClick} />
    </Tooltip>
  );
};
