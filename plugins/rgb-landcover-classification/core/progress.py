from __future__ import annotations
import json

def report_progress(execution_id: str, percent: int, stage: str, stream=None) -> None:
    import sys
    stream = stream or sys.stdout
    print("PROGRESS:" + json.dumps({"job_id": execution_id, "percent": max(0, min(100, int(percent))), "stage": stage}, separators=(",", ":")), file=stream, flush=True)
