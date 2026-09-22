"""Pelaporan progres ke stdout (Bagian 3.7 spesifikasi Modul 8).

Supervisor Modul 2 membaca stdout baris demi baris. Baris berawalan `PROGRESS:`
diikuti JSON {"job_id", "percent", "stage"} diubah menjadi event progres di UI;
baris lain hanya dianggap log.
"""

from __future__ import annotations

import json


def report_progress(execution_id: str, percent: int, stage: str) -> None:
    percent = max(0, min(100, int(percent)))
    payload = {"job_id": execution_id, "percent": percent, "stage": stage}
    print("PROGRESS: " + json.dumps(payload), flush=True)
