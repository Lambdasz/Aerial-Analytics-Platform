import { useRef, useState, type ChangeEvent } from "react";
import "./PluginManager.css";

type Plugin = {
  name: string;
  version: string;
  input: string;
  outputs: string[];
  enabled: boolean;
  description?: string;
};

const initialPlugins: Plugin[] = [
  {
    name: "Vegetation Index",
    version: "1.4.2",
    input: "RGB",
    outputs: ["Mask", "Stats"],
    enabled: true,
    description:
      "Analyzes RGB aerial imagery with the Excess Green (ExG) index to identify vegetation, generate a vegetation mask, and calculate coverage statistics.",
  },
  {
    name: "Tree Detector",
    version: "2.1.0",
    input: "RGB",
    outputs: ["Points", "Counts"],
    enabled: true,
    description:
      "Detects visible tree crowns in RGB aerial imagery and returns tree locations, detection points, and total tree counts.",
  },
  {
    name: "Land Cover Classifier",
    version: "3.0.1",
    input: "RGB",
    outputs: ["Thematic", "Areas"],
    enabled: true,
    description:
      "Classifies aerial imagery into land-cover categories such as vegetation, soil, water, and built areas with area statistics.",
  },
  {
    name: "Temporal Change Engine",
    version: "0.9.7",
    input: "Mask",
    outputs: ["Delta", "Heatmap"],
    enabled: false,
    description:
      "Compares analytical outputs from different observation dates to highlight spatial changes and produce change heatmaps.",
  },
  {
    name: "Ortho Mosaic Builder",
    version: "1.2.5",
    input: "Images",
    outputs: ["Ortho"],
    enabled: false,
    description:
      "Combines overlapping aerial images into a georeferenced orthomosaic ready for mapping and further analysis.",
  },
  {
    name: "Image QC Screener",
    version: "1.0.3",
    input: "Images",
    outputs: ["Flags"],
    enabled: true,
    description:
      "Checks aerial images for quality issues such as blur, poor exposure, unsupported formats, and missing metadata.",
  },
  {
    name: "NDVI Proxy",
    version: "0.4.1",
    input: "RGB",
    outputs: ["Index"],
    enabled: true,
    description:
      "Estimates vegetation vigor from available RGB information and produces a proxy index for comparative analysis.",
  },
  {
    name: "Canopy Height Estimator",
    version: "0.6.0",
    input: "RGB + Sun",
    outputs: ["Heights"],
    enabled: true,
    description:
      "Estimates relative canopy height using RGB imagery and sun or flight metadata where available.",
  },
];

export function PluginManager() {
  const [plugins, setPlugins] = useState(initialPlugins);
  const [selectedName, setSelectedName] = useState(initialPlugins[0].name);
  const [isInstallOpen, setIsInstallOpen] = useState(false);
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [uploadError, setUploadError] = useState("");
  const fileInputRef = useRef<HTMLInputElement>(null);

  const selectedPlugin = plugins.find((plugin) => plugin.name === selectedName) ?? plugins[0];
  const enabledCount = plugins.filter((plugin) => plugin.enabled).length;

  function togglePlugin(name: string) {
    setPlugins((current) =>
      current.map((plugin) =>
        plugin.name === name ? { ...plugin, enabled: !plugin.enabled } : plugin,
      ),
    );
  }

  function chooseFile(file: File | undefined) {
    setUploadError("");
    if (!file) return;
    if (!file.name.toLowerCase().endsWith(".zip")) {
      setSelectedFile(null);
      setUploadError("Plugin package must be a .zip file.");
      return;
    }
    setSelectedFile(file);
  }

  function handleFileChange(event: ChangeEvent<HTMLInputElement>) {
    chooseFile(event.currentTarget.files?.[0]);
  }

  return (
    <main className="plugin-shell">
      <section className="plugin-content">
        <header className="topbar">
          <div>
            <p className="eyebrow">Plugin system</p>
            <h1>Manage plugins</h1>
          </div>
          <button className="install-button" onClick={() => setIsInstallOpen(true)} type="button">
            ＋ Install plugin
          </button>
        </header>

        {isInstallOpen && (
          <div
            className="install-overlay"
            role="presentation"
            onMouseDown={(event) => {
              if (event.target === event.currentTarget) setIsInstallOpen(false);
            }}
          >
            <section className="install-dialog" aria-labelledby="install-title">
              <div className="dialog-header">
                <div>
                  <p className="eyebrow">Extension manager</p>
                  <h2 id="install-title">Install a plugin</h2>
                  <p className="dialog-subtitle">
                    Upload a validated plugin package to this workspace.
                  </p>
                </div>
              </div>
              <div className="drop-zone">
                <span>Plugin package · ZIP only · max 250 MB</span>
                <button
                  className="browse-button"
                  onClick={() => fileInputRef.current?.click()}
                  type="button"
                >
                  Browse files
                </button>
                <input
                  ref={fileInputRef}
                  accept=".zip,application/zip"
                  className="hidden-file-input"
                  onChange={handleFileChange}
                  type="file"
                />
              </div>
              {selectedFile && (
                <div className="selected-file">
                  <span className="file-symbol">ZIP</span>
                  <div>
                    <strong>{selectedFile.name}</strong>
                    <small>
                      {(selectedFile.size / 1024 / 1024).toFixed(2)} MB · Ready to install
                    </small>
                  </div>
                  <button
                    className="remove-file"
                    onClick={() => setSelectedFile(null)}
                    type="button"
                    aria-label="Remove selected file"
                  >
                    ×
                  </button>
                </div>
              )}
              {uploadError && <p className="upload-error">{uploadError}</p>}
              <div className="dialog-footer">
                <button
                  className="cancel-button"
                  onClick={() => setIsInstallOpen(false)}
                  type="button"
                >
                  Cancel
                </button>
                <button className="install-confirm-button" disabled={!selectedFile} type="button">
                  Install plugin
                </button>
              </div>
            </section>
          </div>
        )}

        <section className="stats-grid" aria-label="Plugin summary">
          <div>
            <span>Installed</span>
            <strong>{plugins.length}</strong>
          </div>
          <div>
            <span>Enabled</span>
            <strong>{enabledCount}</strong>
          </div>
          <div>
            <span>Warnings</span>
            <strong>2</strong>
          </div>
          <div>
            <span>Errors</span>
            <strong>1</strong>
          </div>
        </section>

        <section className="manager-grid">
          <div className="plugin-panel">
            <div className="panel-heading">
              <div>
                <h2>Installed plugins</h2>
                <p>Available analytical capabilities for this workspace</p>
              </div>
              <input aria-label="Search plugins" placeholder="Search plugins" />
            </div>
            <div className="plugin-table" role="table">
              <div className="table-row table-head" role="row">
                <span>Name</span>
                <span>Version</span>
                <span>Input</span>
                <span>Outputs</span>
                <span>Enabled</span>
                <span />
              </div>
              {plugins.map((plugin) => (
                <button
                  className={`table-row plugin-row ${selectedName === plugin.name ? "selected" : ""}`}
                  key={plugin.name}
                  onClick={() => setSelectedName(plugin.name)}
                  type="button"
                >
                  <span className="plugin-name">{plugin.name}</span>
                  <span>{plugin.version}</span>
                  <span>
                    <em>{plugin.input}</em>
                  </span>
                  <span className="output-list">
                    {plugin.outputs.map((output) => (
                      <em key={output}>{output}</em>
                    ))}
                  </span>
                  <span>
                    <input
                      aria-label={`Enable ${plugin.name}`}
                      checked={plugin.enabled}
                      onChange={() => togglePlugin(plugin.name)}
                      onClick={(event) => event.stopPropagation()}
                      type="checkbox"
                    />
                  </span>
                  <span className="settings">⚙</span>
                </button>
              ))}
            </div>
          </div>

          <aside className="details-panel">
            <p className="eyebrow">Selected plugin</p>
            <h2>{selectedPlugin.name}</h2>
            <p className="version">
              v{selectedPlugin.version} ·{" "}
              <span className={selectedPlugin.enabled ? "status-ready" : "status-disabled"}>
                {selectedPlugin.enabled ? "Ready" : "Disabled"}
              </span>
            </p>
            <p className="details-copy">
              {selectedPlugin.description ??
                "Configurable analytical plugin available through the standard execution contract."}
            </p>
            <div className="legend">
              <span>INPUT</span>
              <strong>{selectedPlugin.input}</strong>
              <span>OUTPUTS</span>
              {selectedPlugin.outputs.map((output) => (
                <strong key={output}>{output}</strong>
              ))}
            </div>
            <button className="configure-button" type="button">
              Configure plugin
            </button>
          </aside>
        </section>

        <section className="log-panel">
          <div className="panel-heading">
            <div>
              <h2>Execution log</h2>
              <p>Recent plugin health and analysis events</p>
            </div>
            <span className="live-dot">● Live</span>
          </div>
          <pre>
            [16:08:02] INFO vegetation-index: loaded ortho tile set (412 tiles){"\n"}[16:08:41] INFO
            exG computed, mean=0.318 sd=0.094{"\n"}[16:11:15] WARN qc-screener: 38 frames below
            exposure floor{"\n"}[16:12:09] ERROR change-engine: worker OOM at tile 14/4823/5921
          </pre>
        </section>
      </section>
    </main>
  );
}
