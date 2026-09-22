"""Feature 7 - Detection Review."""

from collections.abc import Sequence

from .models import ReviewCorrections, ReviewOutput, TreeDetection

MANUAL_PREFIX = "manual-"


def _as_manual(detection: TreeDetection) -> TreeDetection:
    """Make sure a manually added detection's id starts with ``manual-``."""
    if detection.detection_id.startswith(MANUAL_PREFIX):
        return detection
    return detection.model_copy(
        update={"detection_id": MANUAL_PREFIX + detection.detection_id}
    )


def apply_review(
    detections: Sequence[TreeDetection], corrections: ReviewCorrections
) -> ReviewOutput:
    """Apply a reviewer's corrections: remove false positives, add missed trees.

    ``removed_ids`` only lists ids that actually existed, so unknown ids in
    ``corrections.remove_detection_ids`` are ignored.
    """
    to_remove = frozenset(corrections.remove_detection_ids)
    kept = tuple(d for d in detections if d.detection_id not in to_remove)
    removed = tuple(d.detection_id for d in detections if d.detection_id in to_remove)
    added = tuple(_as_manual(d) for d in corrections.manual_additions)
    return ReviewOutput(
        final_detections=kept + added,
        removed_ids=removed,
        added_count=len(added),
        reviewer_id=corrections.reviewer_id,
    )
