//Module 3
export function getChangeAreaResult() {
  return {
    layer_id: "temporal-change-area",
    layer_name: "Temporal Change Area",
    display_preference: {
      type: "polygon" as const,
    },
    spatial_results: [],
  };
}

//Module 11
export function getTemporalChangeResult() {
  return {
    layer_id: "temporal-change-result",
    layer_name: "Temporal Change Result",
    display_preference: {
      type: "raster_mask" as const,
    },
    spatial_results: [],
  };
}
