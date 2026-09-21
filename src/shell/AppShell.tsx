import { useState } from "react";
import { Navbar } from "@blueprintjs/core";

import { Sidebar } from "./Sidebar";
import { DEFAULT_MODULE_ID, findModule } from "./modules";

/**
 * Application shell: a fixed sidebar for module navigation and a content area
 * that renders the selected module's page.
 *
 * Navigation is plain component state on purpose — no router dependency is
 * installed in this repo, and adding one would need to go through the team's
 * dependency procedure.
 */
export function AppShell() {
  const [activeModuleId, setActiveModuleId] = useState(DEFAULT_MODULE_ID);
  const activeModule = findModule(activeModuleId);
  const ActivePage = activeModule.Page;

  return (
    <div className="app-shell">
      <Navbar className="app-shell__navbar">
        <Navbar.Group>
          <Navbar.Heading>Aerial Analytics Platform</Navbar.Heading>
        </Navbar.Group>
      </Navbar>

      <div className="app-shell__body">
        <Sidebar activeModuleId={activeModuleId} onSelectModule={setActiveModuleId} />
        <main className="app-shell__content">
          <ActivePage />
        </main>
      </div>
    </div>
  );
}
