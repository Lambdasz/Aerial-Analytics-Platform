"""Feature 3 - Confidence Filtering."""

from collections.abc import Sequence
from typing import Annotated

from pydantic import Field, validate_call

from .models import FilteredOutput, TreeDetection

Threshold = Annotated[float, Field(ge=0.0, le=1.0)]


@validate_call
def filter_by_confidence(
    detections: Sequence[TreeDetection], threshold: Threshold
) -> FilteredOutput:
    """Keep detections with ``confidence >= threshold``.

    A threshold outside 0.0-1.0 is rejected by Pydantic (``ValidationError``)
    before the function body runs.
    """
    kept = tuple(d for d in detections if d.confidence >= threshold)
    return FilteredOutput(
        kept=kept,
        removed_count=len(detections) - len(kept),
        threshold_used=threshold,
    )
