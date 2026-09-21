export interface Plot {
  plot_id: string;
  plot_name: string;
  crs: string;
  rings: [number, number][][];
}

export interface ImportResult {
  plots: Plot[];
  skipped: string[];
}
