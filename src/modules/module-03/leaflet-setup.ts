import * as L from "leaflet";

// Leaflet plugins like Geoman (@geoman-io/leaflet-geoman-free) require window.L globally.
if (typeof window !== "undefined") {
  (window as unknown as { L: typeof L }).L = L;
}

export { L };
