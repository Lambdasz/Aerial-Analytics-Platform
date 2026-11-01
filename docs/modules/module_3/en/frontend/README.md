# Frontend Architecture: Module 03 (Aerial Image Map Explorer)

This section provides an architectural overview of the frontend implementation for **Module 03 (Aerial Image Map Explorer)**.

---

## 1. Architectural Philosophy & Technology Stack

The frontend is constructed using modern web and geospatial technologies tailored for a responsive desktop experience:

| Technology                   | Role & Responsibility                             | Key Architectural Constraints                                      |
| :--------------------------- | :------------------------------------------------ | :----------------------------------------------------------------- |
| **React 19**                 | Component declarative rendering and UI reactivity | Strict compliance with Functional Programming (pure render, hooks) |
| **TypeScript (Strict Mode)** | Type safety across IPC boundaries and state       | `noUnusedLocals`, `noUnusedParameters`, strict null checks         |
| **Blueprint.js (v5)**        | Desktop application design system and widgets     | Native desktop feel, dark mode integration, consistent modals      |
| **Leaflet & React-Leaflet**  | Interactive geospatial map engine                 | Custom layer bindings, coordinate projection, high-DPI scaling     |
| **Leaflet-Geoman**           | Geometry drawing and vector editing               | Polygon creation, edge manipulation, and coordinate snapping       |
| **Tauri 2 IPC**              | Bridge between React frontend and Rust backend    | All commands routed through typed wrappers (`invokeMap`)           |

---

## 2. Atomic Design Hierarchy

All UI components under `src/modules/module-03/` strictly adhere to the **Atomic Design** pattern:

```text
Atoms       -> Smallest standalone elements (e.g., StatusBadge)
Molecules   -> Compositions of atoms (e.g., AoiCardItem, OrthoCardItem, ResultPhoto)
Organisms   -> Complex composite functional units (e.g., AoiToolbar, LayerManager, MapCanvas)
Pages       -> Layout-level container views (e.g., MapPage)
```

### Component Rules

- **Stateless Molecules**: Molecule components must avoid initiating direct backend IPC calls. Instead, they receive props, display data, and emit user action callbacks.
- **Controlled State Organisms**: Organisms coordinate custom React hooks with child molecules. They manage loading indicators, error banners, and modal visibility.
- **Pure Helpers**: Complex computations (Shoelace area, WGS84 bounding box calculations, popup HTML templating) are delegated to `lib/` pure utility functions.

---

## 3. State Management & Hook Boundaries

State in Module 03 is divided into three distinct lifecycle tiers:

1. **Persistent Domain State (Backend-Backed)**:
   - Area of Interest polygons (`useAOI`)
   - Raster orthomosaics & job progress (`useSpatialResultLayers`, `useAOI`)
   - Classification/detection layers (`useSpatialResultLayers`)
   - Geotagged drone photos (`useImageMarkers`)
   - Managed through custom hooks that invoke Tauri IPC commands and maintain synchronized local state.

2. **Ephemeral Map Interaction State (Client-Only)**:
   - Active zoom level, center coordinates, and bounding box (`MapCanvas`)
   - Drawing mode state (Geoman active shape: polygon, rectangle, marker)
   - Temporary distance/area measurement overlays (`MeasureResultCard`)
   - Sidebar tab selection (AOI, Layers, Results, Ortho)

3. **Cross-Component Context**:
   - `ImageSyncContext`: Coordinates the active image highlight between the interactive map markers and the photo gallery sidebar.

---

## 4. Standardized Tauri IPC Integration

All communication between the frontend and the Rust backend is routed through the central wrapper `invokeMap`:

```typescript
// Located in src/modules/module-03/services/error.ts
export async function invokeMap<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (error) {
    const normalized = normalizeMapError(error);
    throw normalized;
  }
}
```

This ensures consistent error normalization (`MapControllerError`), automatic error code translation, and safe mock fallbacks when running in a standalone web browser environment without the Tauri runtime.
