"""Feature 6 - Tree Density Analysis."""

from .errors import ZeroAreaError
from .models import DensityOutput, DensityUnit

SQUARE_METERS_PER_HECTARE = 10_000.0


def calculate_tree_density(
    tree_count: int, area_m2: float, unit: DensityUnit = "per_ha"
) -> DensityOutput:
    """Trees per m² or per hectare. Raises ``ZeroAreaError`` when ``area_m2 <= 0``."""
    if area_m2 <= 0:
        raise ZeroAreaError(f"Cannot compute density for an area of {area_m2} m²")
    per_m2 = tree_count / area_m2
    value = per_m2 * SQUARE_METERS_PER_HECTARE if unit == "per_ha" else per_m2
    return DensityOutput(density_value=value, unit=unit, area_m2=area_m2)
