import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Button, Callout, Card, H4, HTMLTable, NonIdealState } from "@blueprintjs/core";
import type { ImportResult, Plot } from "../types/plot";

function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "Terjadi kesalahan yang tidak diketahui.";
}

export default function PlotBoundaryManager() {
  const [plots, setPlots] = useState<Plot[]>([]);
  const [skipped, setSkipped] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const handleImportClick = async () => {
    setLoading(true);
    setError(null);
    try {
      const path = await open({
        multiple: false,
        filters: [{ name: "GeoJSON", extensions: ["geojson", "json"] }],
      });
      if (path === null) return;

      const result = await invoke<ImportResult>("import_plots", { path });
      setPlots(result.plots);
      setSkipped(result.skipped);
    } catch (e) {
      setPlots([]);
      setSkipped([]);
      setError(errorMessage(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <Card>
      <H4>Modul 9: Area &amp; Plot Analytics</H4>
      <Button
        icon="import"
        intent="primary"
        loading={loading}
        text="Import Data Lahan (GeoJSON)"
        onClick={() => {
          void handleImportClick();
        }}
      />

      {error !== null && (
        <Callout intent="danger" title="Impor gagal">
          {error}
        </Callout>
      )}

      {skipped.length > 0 && (
        <Callout intent="warning" title="Sebagian fitur dilewati">
          <ul>
            {skipped.map((reason) => (
              <li key={reason}>{reason}</li>
            ))}
          </ul>
        </Callout>
      )}

      {plots.length > 0 ? (
        <HTMLTable striped compact>
          <thead>
            <tr>
              <th>Nama Plot</th>
              <th>Jumlah Titik</th>
              <th>CRS</th>
              <th>ID</th>
            </tr>
          </thead>
          <tbody>
            {plots.map((plot) => (
              <tr key={plot.plot_id}>
                <td>{plot.plot_name}</td>
                <td>{plot.rings[0].length - 1}</td>
                <td>{plot.crs}</td>
                <td>{plot.plot_id.slice(0, 8)}</td>
              </tr>
            ))}
          </tbody>
        </HTMLTable>
      ) : (
        <NonIdealState
          icon="polygon-filter"
          title="Belum ada plot"
          description="Impor berkas GeoJSON untuk menampilkan batas lahan."
        />
      )}
    </Card>
  );
}
