# Alur Kerja & Konkurensi Backend: Modul 03

Dokumen ini merinci pipeline operasional, supervisor konkurensi latar belakang, dan mekanisme rekonsiliasi filesystem yang diimplementasikan di dalam backend **Modul 03 (Aerial Image Map Explorer)**.

---

## 1. Storage to Frontend Pipeline (Read Direction)

Pipeline pembacaan mengalirkan entitas disk yang tersimpan ke dalam memori secara aman dan meneruskannya melintasi batas Tauri IPC tanpa menimbulkan risiko kepanikan (panic) aplikasi akibat data yang rusak:

```text
[ Penyimpanan Disk: map_storage/ ]
           |
           | 1. fs::read_dir() / fs::read_to_string()
           v
[ Pembaca Submodul (reader.rs / store.rs) ]
           |
           +---> JSON Valid?
           |     YA    -> Deserialisasi ke struct domain (misal SpatialResultItem)
           |     TIDAK -> Tangkap error, tandai is_corrupted = true, isolasi error
           |
           v
[ Sinkronisasi Cache Memori (state.rs: Module03State) ]
           |
           | 2. RwLock::write() -> Memperbarui cache pencarian in-memory
           v
[ Handler Perintah Tauri (commands.rs) ]
           |
           | 3. Serialisasi ke serde_json::Value
           v
[ Penerima IPC Frontend (invokeMap) ]
```

---

## 2. Frontend to Storage Pipeline (Write & Mutate Direction)

Setiap mutasi state menerapkan kebijakan Write-Through yang ketat dengan penggantian filesystem secara atomik:

```text
[ Payload Mutasi Frontend (Permintaan IPC) ]
                       |
                       | 1. Handler IPC di commands.rs
                       v
[ Verifikasi Kunci Instans Utama ]
                       |
                       +---> Apakah Proses Utama? (instance_lock::require_primary_instance)
                       |     TIDAK -> Kembalikan Err(CommandError: ACCESS_DENIED)
                       |     YA    -> Lanjutkan
                       v
[ Validasi Parameter & Geometri ]
  - Verifikasi rentang koordinat WGS84 (-90..90, -180..180)
  - Verifikasi batas atas ukuran JSON (<= 64 KB untuk anotasi/AOI)
                       |
                       v
[ Persistensi Disk Atomik (write_atomic) ]
  - Tulis payload ke buffer efemeral: temp_{uuid}.tmp
  - Flush buffer berkas sistem operasi (sync_all)
  - Penggantian nama atomik: temp_{uuid}.tmp -> target.json
                       |
                       v
[ Pembaruan Cache Memori (state.rs) ]
  - Dapatkan kunci tulis pada Arc<RwLock<HashMap<...>>>
  - Masukkan entitas yang telah berkomitmen
                       |
                       v
[ Kembalikan Payload Sukses melintasi IPC ]
```

---

## 3. Dual-Mode Orthomosaic Ingestion Pipeline

Mengimpor ortomosaik aerial dibagi menjadi dua mode tergantung pada kebutuhan pengguna:

```text
[ Pengguna Memilih Berkas GeoTIFF di UI ]
                       |
                       v
[ inspect_geotiff_header() ]
  - Validasi tag cepat (lebar, tinggi, kedalaman bit, kode CRS)
  - Konversi bounds dari UTM / CRS terproyeksi ke WGS84
                       |
                       v
[ Pengguna Memilih Mode A atau Mode B di OrthoUploadModal ]
                       |
       +---------------+---------------+
       |                               |
       v                               v
[ Mode A: Fast Copy ]       [ Mode B: Auto Tiling ]
  - Salin raw.tif ke images/  - Alokasikan job_id & CancellationToken
  - Hasilkan meta.json        - Catat status awal di .jobs/{job_id}.json
  - Daftarkan layer default   - Spawn tugas latar belakang asinkron di thread pool
  - Selesai dalam < 500 ms    - Buat tile Web Mercator (zoom 10-18)
                              - Alirkan progres % ke .jobs/{job_id}.json
                              - Bersihkan token job setelah selesai
```

---

## 4. Startup Storage Reconciliation & Auto-Heal

Saat backend diinisialisasi, `ortho_source::store::reconcile_storage_consistency()` melakukan audit terhadap `map_storage/`:

1. **Penyelesaian Karantina (`.trash_*`)**:
   - Memindai sisa direktori `.trash_{timestamp}_{id}`.
   - Jika layer yang sesuai (`layers/{id}.json`) masih ada, memulihkan folder karantina kembali ke `images/{id}/`.
   - Jika layer sudah terhapus, menghapus folder karantina secara permanen.
2. **Auto-Heal Layer Hilang (Kasus A)**:
   - Jika `images/{id}/meta.json` ada namun `layers/{id}.json` tidak ada, membuat ulang rekaman layer raster default.
3. **Pembersihan Layer Yatim (Kasus B)**:
   - Jika `layers/{id}.json` bertipe raster ada namun direktori pendukungnya `images/{id}/` tidak ada, menghapus berkas layer yatim untuk mencegah layer hantu di Leaflet.

---

## 5. Process Concurrency & Instance Locking Pipeline

```text
Peluncuran Aplikasi (run di lib.rs)
         |
         v
Periksa map_storage/.app.lock
         |
         +---> Berkas lock TIDAK ada?
         |     Klaim kunci -> Tulis PID OS saat ini -> Menjadi UTAMA (PRIMARY).
         |
         +---> Berkas lock ada?
               |
               +---> Apakah proses PID benar-benar aktif berjalan?
                     TIDAK (Stale Lock) -> Timpa dengan PID saat ini -> Menjadi UTAMA.
                     YA -> Tandai instans sebagai SEKUNDER (Mode Baca-Saja).
```
