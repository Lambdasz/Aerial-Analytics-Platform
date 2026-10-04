// Sidebar.tsx

import { MODULES } from "./modules";

export interface SidebarProps {
  readonly activeModuleId: string;
  readonly onSelectModule: (moduleId: string) => void;
}

export function Sidebar({ activeModuleId, onSelectModule }: SidebarProps) {
  return (
    <nav className="app-shell_sidebar" aria-label="Navigasi modul">
      <div className="app-shell_brand">
        <span className="app-shell_brand-bold">Aerial</span>
        <span>Analysis</span>
      </div>
      <ul className="app-shell_menu">
        {MODULES.map((entry) => {
          const active = entry.id === activeModuleId;
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
                <span className="app-shell_icon" />
                <span className="app-shell_label">{entry.label}</span>
              </button>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
