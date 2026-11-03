"""Uji end-to-end dan unit plugin. Jalankan: python -m unittest discover -s tests -v

Opsional: set LANDCOVER_SCHEMA_DIR ke folder `schemas/` repository platform (dan pasang
`jsonschema`) untuk memvalidasi payload dan hasil terhadap schema Modul 2.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

import numpy as np
from PIL import Image

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, ROOT)

from core.classify import apply_confidence_threshold  # noqa: E402
from core.models import DEFAULT_CLASSES, ClassDefinition, ScaleInfo  # noqa: E402
from core.scale import _read_xmp_relative_altitude, gsd_from_optics, resolve_gsd  # noqa: E402
from core.statistics import compute_class_statistics  # noqa: E402
from tools.train_rf_baseline import train  # noqa: E402

try:
    import rasterio
    from rasterio.transform import from_origin
except ImportError:  # pragma: no cover
    rasterio = None

SCHEMA_DIR = os.environ.get("LANDCOVER_SCHEMA_DIR")
try:
    import jsonschema
except ImportError:  # pragma: no cover
    jsonschema = None

QUADRANTS = {  # (baris, kolom) -> (RGB rata-rata, id kelas)
    (0, 0): ((60, 140, 50), 1),  # vegetation
    (0, 1): ((140, 100, 70), 2),  # bare_soil
    (1, 0): ((30, 90, 180), 3),  # water
    (1, 1): ((130, 130, 130), 4),  # built_up
}
SIZE = 120


def make_scene(seed: int = 0):
    rng = np.random.default_rng(seed)
    rgb = np.zeros((SIZE, SIZE, 3), dtype=np.uint8)
    labels = np.zeros((SIZE, SIZE), dtype=np.uint8)
    half = SIZE // 2
    for (r, c), (color, cid) in QUADRANTS.items():
        sl = (slice(r * half, (r + 1) * half), slice(c * half, (c + 1) * half))
        noise = rng.normal(0, 6, size=(half, half, 3))
        rgb[sl] = np.clip(np.array(color) + noise, 0, 255).astype(np.uint8)
        labels[sl] = cid
    return rgb, labels


def validate(instance, schema_name):
    if SCHEMA_DIR and jsonschema:
        with open(os.path.join(SCHEMA_DIR, schema_name), encoding="utf-8") as fh:
            jsonschema.Draft7Validator(json.load(fh)).validate(instance)


class PluginTestBase(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.mkdtemp(prefix="landcover_test_")
        cls.plugin = os.path.join(cls.tmp, "plugin")
        shutil.copytree(ROOT, cls.plugin, ignore=shutil.ignore_patterns("tests", "__pycache__", "*.joblib", ".git"))
        rgb, labels = make_scene()
        cls.jpg = os.path.join(cls.tmp, "scene.jpg")
        Image.fromarray(rgb).save(cls.jpg, quality=98)
        cls.png = os.path.join(cls.tmp, "scene.png")
        Image.fromarray(rgb).save(cls.png)
        lab = os.path.join(cls.tmp, "labels.png")
        Image.fromarray(labels).save(lab)
        train([(cls.png, lab)], os.path.join(cls.plugin, "models", "rf_superpixel_v1.joblib"), segment_size=200, n_estimators=50)
        cls.rgb = rgb

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.tmp, ignore_errors=True)

    def run_plugin(self, payload, plugin_dir=None, name="run", validate_payload=True):
        plugin_dir = plugin_dir or self.plugin
        out_dir = os.path.join(self.tmp, name)
        os.makedirs(out_dir, exist_ok=True)
        payload = dict(payload, output_dir=out_dir)
        inp, outp = os.path.join(out_dir, "payload.json"), os.path.join(out_dir, "result.json")
        with open(inp, "w", encoding="utf-8") as fh:
            json.dump(payload, fh)
        if validate_payload:
            validate(payload, "execution_payload.schema.json")
        proc = subprocess.run([sys.executable, os.path.join(plugin_dir, "main.py"), "--input", inp, "--output", outp],
                              capture_output=True, text=True, timeout=300)
        with open(outp, encoding="utf-8") as fh:
            result = json.load(fh)
        validate(result, "execution_result.schema.json")
        return proc, result

    def payload(self, image, mime="image/jpeg", **extra):
        p = {
            "execution_id": "job_test_0001",
            "output_dir": "/unused",
            "target": {"granularity": "single_image", "image_path": image, "mime_type": mime,
                       "metadata": {"resolution": [SIZE, SIZE]}},
            "parameters": {"model_id": "rf_superpixel_v1", "min_confidence": 0.3, "segment_size": 200},
        }
        p.update(extra)
        return p


class TestHealthcheck(PluginTestBase):
    def test_healthcheck_ok(self):
        proc = subprocess.run([sys.executable, os.path.join(self.plugin, "main.py"), "--healthcheck"],
                              capture_output=True, text=True)
        self.assertEqual(proc.returncode, 0, proc.stderr)
        info = json.loads(proc.stdout)
        self.assertEqual(info["status"], "healthy")
        self.assertEqual(info["plugin_id"], "rgb-landcover-classification")
        self.assertTrue(info["details"]["dependencies_ready"])


class TestEndToEnd(PluginTestBase):
    def test_jpeg_without_georeference(self):
        proc, res = self.run_plugin(self.payload(self.jpg), name="jpeg")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(res["status"], "success")
        self.assertEqual(res["execution_id"], "job_test_0001")
        m = res["metrics"]
        for key in ("vegetation_pct", "bare_soil_pct", "water_pct", "built_up_pct"):
            self.assertAlmostEqual(m[key], 25.0, delta=6.0, msg=key)
        self.assertEqual(m["other_pct"], 0.0)
        self.assertEqual(m["total_pixels_analyzed"], SIZE * SIZE)
        self.assertIsNone(m["gsd_m_per_px"])
        self.assertIsNone(m["vegetation_area_m2"])
        self.assertNotIn("boundary_vector", res["artifacts"])
        self.assertNotIn("bounds", res["artifacts"]["mask_raster"])
        for art in res["artifacts"].values():
            self.assertTrue(os.path.isfile(art["file_path"]), art)
        # metrik harus persis sama dengan yang dideklarasikan di outputs.json
        with open(os.path.join(ROOT, "outputs.json"), encoding="utf-8") as fh:
            declared = json.load(fh)
        self.assertEqual(set(m), {x["key"] for x in declared["metrics"]})
        rgba = np.asarray(Image.open(res["artifacts"]["mask_raster"]["file_path"]))
        self.assertEqual(rgba.shape, (SIZE, SIZE, 4))
        # progres tercetak dengan format yang dibaca Supervisor
        lines = [ln for ln in proc.stdout.splitlines() if ln.startswith("PROGRESS:")]
        self.assertGreaterEqual(len(lines), 5)
        parsed = [json.loads(ln[len("PROGRESS:"):]) for ln in lines]
        self.assertTrue(all(p["job_id"] == "job_test_0001" for p in parsed))
        self.assertEqual([p["percent"] for p in parsed], sorted(p["percent"] for p in parsed))
        with open(os.path.join(self.tmp, "jpeg", "class_statistics.json"), encoding="utf-8") as fh:
            stats = json.load(fh)
        self.assertEqual(stats["run_metadata"]["model_name"], "rf_superpixel_v1")
        self.assertEqual(len(stats["per_class_statistics"]), 4)

    def test_explicit_gsd_gives_area(self):
        p = self.payload(self.jpg)
        p["parameters"]["gsd_m_per_px"] = 0.05
        proc, res = self.run_plugin(p, name="gsd")
        self.assertEqual(res["status"], "success", proc.stderr)
        self.assertEqual(res["metrics"]["gsd_m_per_px"], 0.05)
        total_area = sum(res["metrics"][f"{n}_area_m2"] for n in ("vegetation", "bare_soil", "water", "built_up", "other"))
        self.assertAlmostEqual(total_area, SIZE * SIZE * 0.05**2, delta=SIZE * SIZE * 0.05**2 * 0.01)

    def test_aoi_without_georeference_warns(self):
        aoi = {"type": "geojson_polygon", "geometry": {"type": "Polygon",
               "coordinates": [[[116.0, -1.0], [116.1, -1.0], [116.1, -1.1], [116.0, -1.1], [116.0, -1.0]]]}}
        proc, res = self.run_plugin(self.payload(self.jpg, aoi=aoi), name="aoi_nogeo")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(res["status"], "warning")
        self.assertIn("AOI", res["error"])
        self.assertEqual(res["metrics"]["total_pixels_analyzed"], SIZE * SIZE)

    def test_custom_classes(self):
        classes = [{"id": 10, "name": "kelas_a", "color": "#112233"}, {"id": 11, "name": "kelas_b", "color": "#445566"}]
        p = self.payload(self.jpg)
        p["parameters"]["classes_json"] = json.dumps(classes)
        proc, res = self.run_plugin(p, name="custom")
        # bobot dilatih dengan id 1-4, maka semua prediksi tidak dikenali -> tidak terklasifikasi
        self.assertEqual(res["status"], "success", proc.stderr)
        self.assertEqual(res["metrics"]["vegetation_pct"], None)
        self.assertEqual(res["metrics"]["unclassified_pct"], 100.0)

    def test_missing_image_is_failure(self):
        proc, res = self.run_plugin(self.payload(os.path.join(self.tmp, "tidak_ada.jpg")), name="noimg")
        self.assertEqual(proc.returncode, 1)
        self.assertEqual(res["status"], "failure")
        self.assertIn("tidak ditemukan", res["error"])
        self.assertEqual(res["artifacts"], {})

    def test_unsupported_mime_is_failure(self):
        proc, res = self.run_plugin(self.payload(self.jpg, mime="image/gif"), name="mime", validate_payload=False)
        self.assertEqual(proc.returncode, 1)
        self.assertIn("UnsupportedFormatError", res["error"])

    def test_missing_weights_is_failure(self):
        bare = os.path.join(self.tmp, "plugin_noweights")
        shutil.copytree(self.plugin, bare, ignore=shutil.ignore_patterns("*.joblib", "__pycache__"))
        proc, res = self.run_plugin(self.payload(self.jpg), plugin_dir=bare, name="noweights")
        self.assertEqual(proc.returncode, 1)
        self.assertIn("Bobot model tidak ditemukan", res["error"])

    def test_deeplab_not_available_is_failure(self):
        p = self.payload(self.jpg)
        p["parameters"]["model_id"] = "deeplab_resnet101_v1"
        proc, res = self.run_plugin(p, name="deeplab")
        self.assertEqual(res["status"], "failure")
        self.assertIn("deeplab_resnet101_v1", res["error"])


@unittest.skipIf(rasterio is None, "rasterio tidak terpasang")
class TestGeoTiff(PluginTestBase):
    ORIGIN_LON, ORIGIN_LAT, PIXEL = 116.8523, -1.2435, 0.00001

    def make_geotiff(self):
        path = os.path.join(self.tmp, "scene_geo.tif")
        with rasterio.open(path, "w", driver="GTiff", height=SIZE, width=SIZE, count=3, dtype="uint8",
                           crs="EPSG:4326", transform=from_origin(self.ORIGIN_LON, self.ORIGIN_LAT, self.PIXEL, self.PIXEL)) as dst:
            dst.write(np.transpose(self.rgb, (2, 0, 1)))
        return path

    def test_georeferenced_with_aoi(self):
        tif = self.make_geotiff()
        half_lon = self.ORIGIN_LON + 60 * self.PIXEL  # AOI = separuh kiri citra
        bottom = self.ORIGIN_LAT - SIZE * self.PIXEL
        aoi = {"type": "geojson_polygon", "geometry": {"type": "Polygon", "coordinates": [[
            [self.ORIGIN_LON, self.ORIGIN_LAT], [half_lon, self.ORIGIN_LAT], [half_lon, bottom],
            [self.ORIGIN_LON, bottom], [self.ORIGIN_LON, self.ORIGIN_LAT]]]}}
        proc, res = self.run_plugin(self.payload(tif, mime="image/tiff", aoi=aoi), name="geo")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(res["status"], "success", res["error"])
        m = res["metrics"]
        self.assertAlmostEqual(m["total_pixels_analyzed"], 60 * SIZE, delta=2 * SIZE)
        # separuh kiri citra = kuadran vegetation (atas) + water (bawah)
        self.assertAlmostEqual(m["vegetation_pct"], 50.0, delta=8.0)
        self.assertAlmostEqual(m["water_pct"], 50.0, delta=8.0)
        self.assertLess(m["bare_soil_pct"], 5.0)
        self.assertLess(m["built_up_pct"], 5.0)
        self.assertIsNotNone(m["gsd_m_per_px"])
        self.assertAlmostEqual(m["gsd_m_per_px"], 1.1, delta=0.3)  # 1e-5 derajat di lintang ~1.2 S
        self.assertIsNotNone(m["vegetation_area_m2"])
        bounds = res["artifacts"]["mask_raster"]["bounds"]
        self.assertEqual(len(bounds), 4)
        self.assertAlmostEqual(bounds[0], self.ORIGIN_LON, places=6)  # min_lon
        self.assertAlmostEqual(bounds[3], self.ORIGIN_LAT, places=6)  # max_lat
        # vektor ada dan urutan koordinat [lon, lat]
        with open(res["artifacts"]["boundary_vector"]["file_path"], encoding="utf-8") as fh:
            fc = json.load(fh)
        self.assertEqual(fc["type"], "FeatureCollection")
        self.assertGreater(len(fc["features"]), 0)
        for f in fc["features"]:
            for ring in f["geometry"]["coordinates"]:
                for lon, lat in ring:
                    self.assertTrue(116.85 < lon < 116.86, lon)
                    self.assertTrue(-1.25 < lat < -1.24, lat)
            self.assertIn(f["properties"]["class_name"], ("vegetation", "bare_soil", "water", "built_up"))
        # semua poligon berada di separuh kiri (di dalam AOI)
        xs = [lon for f in fc["features"] for ring in f["geometry"]["coordinates"] for lon, _ in ring]
        self.assertLessEqual(max(xs), half_lon + 1e-9)

    def test_aoi_outside_image_is_failure(self):
        tif = self.make_geotiff()
        aoi = {"type": "geojson_polygon", "geometry": {"type": "Polygon", "coordinates": [[
            [100.0, 0.0], [100.1, 0.0], [100.1, -0.1], [100.0, -0.1], [100.0, 0.0]]]}}
        proc, res = self.run_plugin(self.payload(tif, mime="image/tiff", aoi=aoi), name="aoi_out")
        self.assertEqual(res["status"], "failure")
        self.assertIn("AOI", res["error"])

    def test_swapped_coordinates_rejected(self):
        tif = self.make_geotiff()
        aoi = {"type": "geojson_polygon", "geometry": {"type": "Polygon", "coordinates": [[
            [-1.24, 116.85], [-1.25, 116.85], [-1.25, 116.86], [-1.24, 116.86], [-1.24, 116.85]]]}}
        proc, res = self.run_plugin(self.payload(tif, mime="image/tiff", aoi=aoi), name="swapped")
        self.assertEqual(res["status"], "failure")
        self.assertIn("urutan koordinat", res["error"])


class TestUnits(unittest.TestCase):
    def test_confidence_threshold(self):
        mask = np.array([[1, 2], [3, 0]], dtype=np.uint8)
        conf = np.array([[0.9, 0.2], [0.6, 0.1]], dtype=np.float32)
        out = apply_confidence_threshold(mask, conf, 0.5)
        self.assertEqual(out.class_mask.tolist(), [[1, 0], [3, 0]])
        self.assertEqual(out.unclassified_added, 1)  # hanya piksel yang tadinya terklasifikasi
        self.assertEqual(mask.tolist(), [[1, 2], [3, 0]])  # masukan tidak diubah
        with self.assertRaises(Exception):
            apply_confidence_threshold(mask, conf, 1.5)

    def test_statistics(self):
        mask = np.array([[1, 1, 2], [2, 2, 0]], dtype=np.uint8)
        conf = np.full(mask.shape, 0.8, dtype=np.float32)
        stats = compute_class_statistics(mask, conf, DEFAULT_CLASSES, ScaleInfo(gsd_m_per_px=2.0, source="explicit_parameter", confidence="measured"))
        self.assertEqual([s.name for s in stats], ["bare_soil", "vegetation"])  # urut menurun, kelas kosong dibuang
        self.assertAlmostEqual(stats[0].percent, 50.0)
        self.assertAlmostEqual(stats[0].area_m2, 3 * 4.0)
        no_scale = compute_class_statistics(mask, conf, DEFAULT_CLASSES, ScaleInfo())
        self.assertIsNone(no_scale[0].area_m2)

    def test_gsd_formula_dji_phantom4(self):
        # Phantom 4 Pro: sensor 13.2 mm, focal 8.8 mm, lebar 5472 px, tinggi 100 m -> ~2.74 cm/px
        self.assertAlmostEqual(gsd_from_optics(100, 13.2, 8.8, 5472), 0.02741, places=5)

    def test_resolve_gsd_priority_and_none(self):
        s = resolve_gsd({"gsd_m_per_px": 0.1}, None, "/tidak/ada.jpg")
        self.assertEqual((s.gsd_m_per_px, s.source, s.confidence), (0.1, "explicit_parameter", "measured"))
        s = resolve_gsd({"gsd_m_per_px": 0}, None, "/tidak/ada.jpg")
        self.assertEqual((s.gsd_m_per_px, s.source, s.confidence), (None, "none", "none"))

    def test_xmp_relative_altitude(self):
        with tempfile.NamedTemporaryFile(suffix=".jpg", delete=False) as fh:
            fh.write(b'\xff\xd8...<rdf:Description drone-dji:RelativeAltitude="+87.30" drone-dji:GpsLatitude="1"/>')
        try:
            self.assertAlmostEqual(_read_xmp_relative_altitude(fh.name), 87.3)
        finally:
            os.unlink(fh.name)

    def test_class_definition_validation(self):
        with self.assertRaises(Exception):
            ClassDefinition(id=0, name="x", color="#000000")
        with self.assertRaises(Exception):
            ClassDefinition(id=1, name="x", color="hijau")


if __name__ == "__main__":
    unittest.main()
