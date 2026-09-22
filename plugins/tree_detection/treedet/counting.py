"""Feature 2 - Tree Counting."""

from collections.abc import Sequence
from datetime import datetime

from .models import CountOutput, TreeDetection


def count_trees(
    detections: Sequence[TreeDetection], image_id: str, counted_at: datetime
) -> CountOutput:
    """Count detections (already de-duplicated) and average their confidence.

    ``counted_at`` is passed in instead of read from the clock so the function
    stays pure. No detections gives ``total_trees=0``, not an error.
    """
    total = len(detections)
    avg = sum(d.confidence for d in detections) / total if total else 0.0
    return CountOutput(
        total_trees=total,
        image_id=image_id,
        avg_confidence=avg,
        counted_at=counted_at,
    )
