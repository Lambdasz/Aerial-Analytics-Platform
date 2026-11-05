import type { ComponentType } from "react";
import type { IconType } from "react-icons";
import {
  FaImage,
  FaPlug,
  FaMap,
  FaChartColumn,
  FaClockRotateLeft,
  FaGaugeHigh,
  FaBookOpen,
} from "react-icons/fa6";

import { Module01Page } from "../modules/module-01/Module01Page";
import { Module02Page } from "../modules/module-02/Module02Page";
import { Module03Page } from "../modules/module-03";
import { Module09Page } from "../modules/module-09/Module09Page";
import { Module10Page } from "../modules/module-10/Module10Page";
import { Module11Page } from "../modules/module-11/Module11Page";
import { Example } from "./example";

export interface ModuleEntry {
  readonly id: string;
  readonly moduleNumber: number;
  readonly slug: string;
  readonly label: string;
  readonly Icon: IconType;
  readonly Page: ComponentType;
}

export const MODULES: readonly ModuleEntry[] = [
  {
    id: "module-01",
    moduleNumber: 1,
    slug: "image-project-manager",
    label: "Image & Project Manager",
    Icon: FaImage,
    Page: Module01Page,
  },
  {
    id: "module-02",
    moduleNumber: 2,
    slug: "plugin-system",
    label: "Plugin System",
    Icon: FaPlug,
    Page: Module02Page,
  },
  {
    id: "module-03",
    moduleNumber: 3,
    slug: "map-explorer",
    label: "Map Explorer",
    Icon: FaMap,
    Page: Module03Page,
  },
  {
    id: "module-09",
    moduleNumber: 9,
    slug: "area-plot-analytics",
    label: "Area & Plot Analytics",
    Icon: FaChartColumn,
    Page: Module09Page,
  },
  {
    id: "module-10",
    moduleNumber: 10,
    slug: "temporal-change",
    label: "Temporal Change",
    Icon: FaClockRotateLeft,
    Page: Module10Page,
  },
  {
    id: "module-11",
    moduleNumber: 11,
    slug: "dashboard-reporting",
    label: "Dashboard & Reporting",
    Icon: FaGaugeHigh,
    Page: Module11Page,
  },
  {
    id: "example",
    moduleNumber: 7,
    slug: "examples",
    label: "Examples",
    Icon: FaBookOpen,
    Page: Example,
  },
];

// Initial page
export const DEFAULT_MODULE_ID = MODULES[0].id;

// check module existence
export function findModule(id: string): ModuleEntry {
  return MODULES.find((entry) => entry.id === id) ?? MODULES[0];
}

// normalize raw URL
export function normalizeSlug(raw: string): string {
  return raw.trim().replace(/^#/, "").replace(/^\/+/, "").replace(/\/+$/, "").toLowerCase();
}

// Look up url slug
export function findModuleBySlug(raw: string): ModuleEntry | undefined {
  const slug = normalizeSlug(raw);
  if (!slug) return undefined;
  return MODULES.find((entry) => entry.slug === slug || entry.id === slug);
}

export function moduleHash(entry: ModuleEntry): string {
  return `#/${entry.slug}`;
}
