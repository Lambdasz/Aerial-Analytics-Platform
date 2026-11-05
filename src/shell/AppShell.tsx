import { useCallback, useEffect, useState } from "react";

import { Navbar } from "./Navbar";
import { Sidebar } from "./Sidebar";
import { DEFAULT_MODULE_ID, findModule, findModuleBySlug, moduleHash } from "./modules";

export function AppShell() {
  console.log("TESTSTESTS");
  // Read the URL syncs on first render
  const [activeModuleId, setActiveModuleId] = useState(
    () => findModuleBySlug(window.location.hash)?.id ?? DEFAULT_MODULE_ID,
  );

  const activeModule = findModule(activeModuleId);
  const ActivePage = activeModule.Page;

  // Writes slug to url
  const navigateToModule = useCallback((moduleId: string) => {
    const entry = findModule(moduleId);
    const target = moduleHash(entry);
    if (window.location.hash === target) {
      setActiveModuleId(entry.id);
      return;
    }

    // Assigning to location.hash pushes a history entry → Back/Forward work.
    window.location.hash = target;
  }, []);

  // Single source of truth: the URL. Anything that changes it updates the page.
  useEffect(() => {
    const syncFromUrl = () => {
      const entry = findModuleBySlug(window.location.hash);
      if (entry) {
        setActiveModuleId(entry.id);
        return;
      }

      // Empty or unknown slug: fall back to the default and clean the URL.
      const fallback = findModule(DEFAULT_MODULE_ID);
      window.history.replaceState(null, "", moduleHash(fallback));
      setActiveModuleId(fallback.id);
    };

    syncFromUrl(); // normalise on first load
    window.addEventListener("hashchange", syncFromUrl);
    return () => window.removeEventListener("hashchange", syncFromUrl);
  }, []);

  // Cross-module navigation: pages dispatch `open-module` with a module id (or slug).
  useEffect(() => {
    const handleOpenModule = (event: Event) => {
      const detail = (event as CustomEvent<{ moduleId?: string; slug?: string }>).detail;
      const key = detail?.moduleId ?? detail?.slug;
      if (!key) return;

      const entry = findModuleBySlug(key);
      if (entry) navigateToModule(entry.id);
    };

    window.addEventListener("open-module", handleOpenModule);
    return () => window.removeEventListener("open-module", handleOpenModule);
  }, [navigateToModule]);

  return (
    <div className="app-shell">
      <Navbar />
      <div className="app-shell_body">
        <Sidebar activeModuleId={activeModuleId} onSelectModule={navigateToModule} />
        <main className="app-shell_content">
          <ActivePage />
        </main>
      </div>
    </div>
  );
}
