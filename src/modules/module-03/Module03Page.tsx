import { MapCanvas } from "../../map/components/MapCanvas";
import { ImageGallerySidebar } from "../../map/components/ImageGallerySidebar";
import { ImageSyncProvider } from "../../map/store/ImageSyncContext";
import "./Module03Page.css";

export function Module03Page() {
  return (
    <ImageSyncProvider>
      <div className="app-container bp5-dark">
        <ImageGallerySidebar />
        <MapCanvas />
      </div>
    </ImageSyncProvider>
  );
}
