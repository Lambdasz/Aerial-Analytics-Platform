import { MapCanvas } from "./map/components/MapCanvas";
import { ImageSyncProvider } from "./map/store/ImageSyncContext";
import "./App.css";

function App() {
  return (
    <ImageSyncProvider>
      <div className="app-container">
        <MapCanvas />
      </div>
    </ImageSyncProvider>
  );
}

export default App;
