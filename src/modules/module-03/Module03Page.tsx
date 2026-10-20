import { MapCanvas } from "./components/MapCanvas";
import { ImageSyncProvider } from "./store/ImageSyncContext";

export function Module03Page() {
  return (
    <ImageSyncProvider>
      <div className="app-container" style={{ width: "100%", height: "100%" }}>
        <MapCanvas />
      </div>
    </ImageSyncProvider>
  );
}
