import { MapCanvas } from "./../../map/components/MapCanvas";
import { ImageGallerySidebar } from "./../../map/components/ImageGallerySidebar";
import { ImageSyncProvider } from "./../../map/store/ImageSyncContext";

export function Module03Page() {
  return (
    <ImageSyncProvider>
      <div className="app-container bp5-dark">
        {" "}
        {/* Blueprint dark theme baseline */}
        <ImageGallerySidebar />
        <MapCanvas />
      </div>
    </ImageSyncProvider>
  );
}
