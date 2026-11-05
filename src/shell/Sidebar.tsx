import { MODULES } from "./modules";

import "./style.css";

export interface SidebarProps {
  readonly activeModuleId: string;
  readonly onSelectModule: (moduleId: string) => void;
}

export function Sidebar({ activeModuleId, onSelectModule }: SidebarProps) {
  return (
    <nav className="app-shell_sidebar" aria-label="Navigasi modul">
      <ul className="app-shell_menu">
        {MODULES.map((entry) => {
          const active = entry.id === activeModuleId;
          const { Icon } = entry;
          return (
            <li key={entry.id}>
              <button
                type="button"
                className={`app-shell_item${active ? " app-shell_item--active" : ""}`}
                aria-current={active ? "page" : undefined}
                onClick={() => {
                  onSelectModule(entry.id);
                }}
              >
                <Icon className="app-shell_icon" aria-hidden="true" />
                <span className="app-shell_label">{entry.label}</span>
              </button>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
