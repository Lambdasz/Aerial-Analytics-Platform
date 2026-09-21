import type { ComponentType } from "react";

import { Module01Page } from "../modules/module-01/Module01Page";
import { Module02Page } from "../modules/module-02/Module02Page";
import { Module03Page } from "../modules/module-03/Module03Page";
import { Module09Page } from "../modules/module-09/Module09Page";
import { Module10Page } from "../modules/module-10/Module10Page";
import { Module11Page } from "../modules/module-11/Module11Page";

export interface ModuleEntry {
  /** Stable identifier used for navigation state. */
  readonly id: string;
  /** Module number as written in the project brief. */
  readonly moduleNumber: number;
  /** Short label shown in the sidebar. */
  readonly label: string;
  /** Page rendered when this module is selected. */
  readonly Page: ComponentType;
}

/**
 * Single source of truth for the sidebar and the content area.
 *
 * To add a module: create its page under `src/modules/`, then append one entry
 * here. Nothing else in the shell needs to change.
 */
export const MODULES: readonly ModuleEntry[] = [
  {
    id: "module-01",
    moduleNumber: 1,
    label: "Image & Project Manager",
    Page: Module01Page,
  },
  {
    id: "module-02",
    moduleNumber: 2,
    label: "Plugin System",
    Page: Module02Page,
  },
  {
    id: "module-03",
    moduleNumber: 3,
    label: "Map Explorer",
    Page: Module03Page,
  },
  {
    id: "module-09",
    moduleNumber: 9,
    label: "Area & Plot Analytics",
    Page: Module09Page,
  },
  {
    id: "module-10",
    moduleNumber: 10,
    label: "Temporal Change",
    Page: Module10Page,
  },
  {
    id: "module-11",
    moduleNumber: 11,
    label: "Dashboard & Reporting",
    Page: Module11Page,
  },
];

/** First module in the list, used as the initial selection. */
export const DEFAULT_MODULE_ID = MODULES[0].id;

/** Pure lookup: returns the matching module, or the first one as a fallback. */
export function findModule(id: string): ModuleEntry {
  return MODULES.find((entry) => entry.id === id) ?? MODULES[0];
}
