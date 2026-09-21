"""Fitur 3: Ground Sampling Distance Resolution.

Urutan fallback: parameter eksplisit -> EXIF/XMP -> CRS raster -> none.
Kegagalan menentukan GSD BUKAN error: mengembalikan ScaleInfo(source="none").
"""

from __future__ import annotations

import math
import re

from .models import ImageMetadata, ScaleInfo

# Belum jelas apakah `altitude` pada metadata Modul 1 adalah relative altitude atau
# ketinggian di atas permukaan laut (Temuan 5). Selama belum dikonfirmasi, nilai itu
# TIDAK dipakai untuk estimasi GSD; hanya XMP drone-dji:RelativeAltitude yang dipakai.
TRUST_METADATA_ALTITUDE_AS_RELATIVE = False

_EXIF_IFD = 0x8769
_TAG_FOCAL_LENGTH = 0x920A
_TAG_FOCAL_35MM = 0xA405
_TAG_FP_X_RES = 0xA20E
_TAG_FP_UNIT = 0xA210
_FP_UNIT_MM = {2: 25.4, 3: 10.0, 4: 1.0, 5: 0.001}  # inci, cm, mm, um -> mm
_DIAG_35MM = 43.2666  # diagonal film 35 mm (mm)
_XMP_REL_ALT = re.compile(
    rb"RelativeAltitude(?:\s*=\s*\"|\s*>\s*)\s*([+-]?\d+(?:\.\d+)?)", re.IGNORECASE
)


def gsd_from_optics(altitude_m: float, sensor_width_mm: float, focal_length_mm: float, image_width_px: float) -> float:
    """GSD (m/px) = (altitude x lebar sensor) / (focal length x lebar citra dalam piksel)."""
    return (altitude_m * sensor_width_mm) / (focal_length_mm * image_width_px)


def _read_xmp_relative_altitude(path: str) -> float | None:
    try:
        with open(path, "rb") as fh:
            head = fh.read(1_048_576)  # paket XMP berada di awal berkas JPEG
    except OSError:
        return None
    m = _XMP_REL_ALT.search(head)
    return float(m.group(1)) if m else None


def _read_exif_optics(path: str, width_px: float, height_px: float) -> tuple[float, float] | None:
    """Kembalikan (focal_length_mm, sensor_width_mm) dari EXIF, atau None."""
    try:
        from PIL import Image

        with Image.open(path) as im:
            ifd = im.getexif().get_ifd(_EXIF_IFD)
    except Exception:
        return None
    focal = ifd.get(_TAG_FOCAL_LENGTH)
    if not focal:
        return None
    focal = float(focal)
    fp_res, fp_unit = ifd.get(_TAG_FP_X_RES), ifd.get(_TAG_FP_UNIT)
    if fp_res and float(fp_res) > 0 and int(fp_unit or 0) in _FP_UNIT_MM:
        return focal, width_px / float(fp_res) * _FP_UNIT_MM[int(fp_unit)]
    f35 = ifd.get(_TAG_FOCAL_35MM)
    if f35 and float(f35) > 0:
        diag_sensor = _DIAG_35MM * focal / float(f35)
        return focal, diag_sensor * width_px / math.hypot(width_px, height_px)
    return None


def _gsd_from_exif_xmp(image_path: str, metadata: ImageMetadata | None) -> float | None:
    altitude = _read_xmp_relative_altitude(image_path)
    if altitude is None and TRUST_METADATA_ALTITUDE_AS_RELATIVE and metadata and metadata.altitude:
        altitude = metadata.altitude
    if altitude is None or altitude <= 0:
        return None
    if metadata is not None:
        width_px, height_px = metadata.resolution
    else:
        try:
            from PIL import Image

            with Image.open(image_path) as im:
                width_px, height_px = float(im.width), float(im.height)
        except Exception:
            return None
    optics = _read_exif_optics(image_path, width_px, height_px)
    if optics is None:
        return None
    focal_mm, sensor_w_mm = optics
    gsd = gsd_from_optics(altitude, sensor_w_mm, focal_mm, width_px)
    return gsd if math.isfinite(gsd) and gsd > 0 else None


def _gsd_from_raster(image_path: str) -> float | None:
    try:
        import rasterio

        with rasterio.open(image_path) as src:
            if src.crs is None or src.transform.is_identity:
                return None
            t = src.transform
            if src.crs.is_geographic:  # derajat -> meter di lintang tengah citra
                lat = (src.bounds.top + src.bounds.bottom) / 2.0
                dx = abs(t.a) * 111_320.0 * math.cos(math.radians(lat))
                dy = abs(t.e) * 110_540.0
                gsd = (dx + dy) / 2.0
            else:
                factor = src.crs.linear_units_factor[1]
                gsd = (abs(t.a) + abs(t.e)) / 2.0 * factor
        return gsd if math.isfinite(gsd) and gsd > 0 else None
    except Exception:
        return None


def resolve_gsd(parameters: dict, metadata: ImageMetadata | None, image_path: str) -> ScaleInfo:
    explicit = float(parameters.get("gsd_m_per_px") or 0.0)
    if explicit > 0:
        return ScaleInfo(gsd_m_per_px=explicit, source="explicit_parameter", confidence="measured")

    gsd = _gsd_from_exif_xmp(image_path, metadata)
    if gsd is not None:
        return ScaleInfo(gsd_m_per_px=gsd, source="exif_xmp", confidence="estimated")

    gsd = _gsd_from_raster(image_path)
    if gsd is not None:
        return ScaleInfo(gsd_m_per_px=gsd, source="raster_crs", confidence="measured")

    return ScaleInfo()  # source="none": persentase tetap dihitung, luas tidak
