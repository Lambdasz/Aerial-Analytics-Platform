# Tree Detection & Counting (Modul 7)

Plugin analitik Aerial Analytics Platform untuk mendeteksi tajuk pohon dari citra drone RGB,
menyaring hasilnya berdasarkan confidence, memetakan tiap pohon ke koordinat WGS84, lalu
menghitung jumlah dan kepadatan pohon.

- **Plugin ID:** `tree-detection-counting`
- **Kategori:** `tree_detection`
- **Runtime:** Python ≥ 3.10
- **Kontrak antar-modul:** [CONTRACT.md](CONTRACT.md) (baca ini kalau kamu dari Modul 2, 3, 9, 10, atau 11)

> **Status:** kerangka. Semua fitur sudah berjalan dan teruji, **kecuali model deteksinya**.
> Detector bawaan (`stub`) selalu mengembalikan 0 deteksi, dan hasilnya diberi status `warning`
> supaya tidak disangka hasil nyata. Lihat [Menambah model deteksi](#menambah-model-deteksi).

## Struktur

```text
plugins/tree_detection/
├── manifest.json            # Kontrak 1: identitas & runtime plugin
├── parameters.json          # Kontrak 2: parameter yang bisa diatur user (form Modul 2)
├── inputs.json              # Kontrak 3: input yang dibutuhkan (format gambar, metadata, AOI)
├── outputs.json             # Kontrak 4: metrik & artefak yang dihasilkan
├── execution_payload.json   # Fixture: contoh payload dari host
├── execution_result.json    # Fixture: contoh hasil yang ditulis plugin
├── main.py                  # Entrypoint CLI (lapisan I/O)
├── requirements.txt
├── CONTRACT.md              # Dokumen kontrak Modul 7 untuk modul lain
├── treedet/                 # Logika inti (fungsi murni)
│   ├── models.py            # Model data bersama (Pydantic v2, immutable)
│   ├── errors.py            # PluginExecutionError dan turunannya
│   ├── detectors.py         # Registry model deteksi (tempat menambah model)
│   ├── detection.py         # Fitur 1: detect_trees (+ deduplikasi antar-tile)
│   ├── counting.py          # Fitur 2: count_trees
│   ├── filtering.py         # Fitur 3: filter_by_confidence
│   ├── location.py          # Fitur 4: map_tree_locations
│   ├── area.py              # Fitur 5: count_trees_in_area (+ luas poligon)
│   ├── density.py           # Fitur 6: calculate_tree_density
│   ├── review.py            # Fitur 7: apply_review
│   ├── output.py            # Fitur 8: build_plugin_output → execution_result
│   └── pipeline.py          # Payload → fitur-fitur di atas
└── tests/                   # Unit test & end-to-end test (unittest)
```

## Setup

Jalankan dari root repo:

```powershell
py -3.10 -m venv $HOME\.venvs\aerial-m7
& $HOME\.venvs\aerial-m7\Scripts\Activate.ps1
pip install -r plugins\tree_detection\requirements.txt
pip install check-jsonschema   # untuk validasi kontrak
```

Di Linux/macOS: `python3.10 -m venv ~/.venvs/aerial-m7 && source ~/.venvs/aerial-m7/bin/activate`.

> Taruh virtual environment **di luar repo**. Prettier pada pre-commit (`npm run format:check`)
> tidak membaca `.gitignore` di subfolder, jadi venv di dalam `plugins/` akan ikut diperiksa dan
> membuat commit gagal.

## Menjalankan

```bash
# Healthcheck (dipanggil Modul 2 sebelum menampilkan plugin sebagai aktif)
python plugins/tree_detection/main.py --healthcheck

# Analisis
python plugins/tree_detection/main.py --input <execution_payload.json> --output <result.json>
```

`execution_payload.json` di folder ini berisi path gambar contoh (`/absolute/path/...`),
jadi kalau dijalankan apa adanya hasilnya adalah `status: "failure"` dengan pesan
`Image not found`. Itu disengaja dan membuktikan plugin gagal dengan rapi tanpa crash.
Ganti `target.image_path` dengan gambar asli untuk mencoba alur sukses.

## Test

```bash
cd plugins/tree_detection
python -m unittest discover -s tests -v
```

Test mencakup kedelapan fitur, termasuk kasus batas dari spesifikasi: tidak ada deteksi
(`0`, bukan error), threshold di luar 0–1 (ditolak Pydantic), luas 0 (`ZeroAreaError`),
GeoJSON tidak valid (`InvalidGeometryError`), gambar hilang (`PluginExecutionError`), dan
format tidak didukung (`UnsupportedFormatError`).

## Validasi kontrak

Dari root repo:

```bash
check-jsonschema --schemafile schemas/plugin.schema.json            plugins/tree_detection/manifest.json
check-jsonschema --schemafile schemas/parameters.schema.json        plugins/tree_detection/parameters.json
check-jsonschema --schemafile schemas/inputs.schema.json            plugins/tree_detection/inputs.json
check-jsonschema --schemafile schemas/outputs.schema.json           plugins/tree_detection/outputs.json
check-jsonschema --schemafile schemas/execution_payload.schema.json plugins/tree_detection/execution_payload.json
check-jsonschema --schemafile schemas/execution_result.schema.json  plugins/tree_detection/execution_result.json
```

## Desain (pemrograman fungsional)

- **Pure core, impure shell.** Semua fungsi di `treedet/` bersifat murni: input yang sama
  selalu menghasilkan output yang sama, tanpa I/O. Satu-satunya pengecualian adalah
  `detect_trees`, karena detector harus membaca gambar. Baca/tulis file, jam, dan exit
  code hanya ada di `main.py`.
- **Data immutable.** Semua model Pydantic memakai `frozen=True`, dan list berbentuk `tuple`.
  Fungsi tidak pernah mengubah input; perubahan dibuat lewat `model_copy(update=...)`.
- **Waktu sebagai parameter.** `count_trees` dan `analyze` menerima `counted_at` dari luar,
  bukan membaca jam sendiri, supaya hasilnya bisa diuji dan diulang.
- **Detector sebagai fungsi.** Model deteksi adalah fungsi biasa yang disuntikkan lewat registry,
  sehingga test memakai detector palsu tanpa model sungguhan.
- **Error terisolasi.** Semua error turunan `PluginExecutionError` ditangkap `main.py`, lalu ditulis
  sebagai hasil `failure`. Plugin tidak pernah crash tanpa menulis hasil (syarat Modul 2).

## Menambah model deteksi

1. Tulis fungsi `(image_path: str, model_config: Mapping) -> Sequence[tuple[BBox, float]]`
   di `treedet/detectors.py`. Fungsi ini mengembalikan kotak tajuk dalam piksel beserta
   confidence 0–1.
2. Daftarkan fungsi itu di `DETECTORS`, misalnya `"opencv_watershed": opencv_watershed_detector`.
3. Tambahkan namanya ke `options` parameter `detector` di `parameters.json`.
4. Tambahkan paketnya ke `requirements.txt` dan `check_dependencies()` di `main.py`.
5. Kalau memakai GPU, sesuaikan `execution.gpu` di `manifest.json`.

Detector mengurus pembagian tile (ukurannya ada di `model_config["tile_size"]`) dan inferensi.
Deduplikasi antar-tile, filter confidence, georeferensi, dan penghitungan sudah ditangani
pipeline.
