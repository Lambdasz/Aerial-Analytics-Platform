import { Menu, MenuDivider, MenuItem } from "@blueprintjs/core";

import { MODULES } from "./modules";

export interface SidebarProps {
  /** Id of the module currently shown in the content area. */
  readonly activeModuleId: string;
  /** Called with the id of the module the user picked. */
  readonly onSelectModule: (moduleId: string) => void;
}

export function Sidebar({ activeModuleId, onSelectModule }: SidebarProps) {
  return (
    <nav className="app-shell__sidebar" aria-label="Navigasi modul">
      <Menu className="app-shell__menu" size="large">
        <MenuDivider title="Modul" />
        {MODULES.map((entry) => (
          <MenuItem
            key={entry.id}
            text={`${entry.moduleNumber}. ${entry.label}`}
            active={entry.id === activeModuleId}
            aria-current={entry.id === activeModuleId ? "page" : undefined}
            onClick={() => {
              onSelectModule(entry.id);
            }}
          />
        ))}
      </Menu>
    </nav>
  );
}
