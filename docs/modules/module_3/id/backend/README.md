# Arsitektur Backend: Modul 03 (Aerial Image Map Explorer)

Bagian ini merinci arsitektur backend untuk **Modul 03 (Aerial Image Map Explorer)** yang diimplementasikan dalam Rust dan Tauri 2.

---

## 1. Architectural Philosophy & Layered Model

Backend dibangun di atas pemisahan tanggung jawab yang ketat, memastikan isolasi error, persistensi filesystem atomik, dan abstraksi tanpa biaya performa (zero-cost abstraction):

```text
+-------------------------------------------------------+
|                   Lapisan Tauri IPC                   |
|  commands.rs / storage_commands.rs (Handler Ramping)  |
+-------------------------------------------------------+
                           |
                           v
+-------------------------------------------------------+
|               Layanan / Pipeline Domain               |
|  image_pipeline.rs / geotiff_reader.rs / builder.rs   |
+-------------------------------------------------------+
                           |
                           v
+-------------------------------------------------------+
|             Lapisan Penyimpanan & Cache               |
|  store.rs / reader.rs / writer.rs / state.rs          |
+-------------------------------------------------------+
                           |
                           v
+-------------------------------------------------------+
|             Primitif Filesystem (Disk)                |
|  map_storage/ (Tulis atomik: temp_* -> rename)        |
+-------------------------------------------------------+
```

### Architectural Principles

1. **Handler Perintah Ramping (Thin Handlers)**: Fungsi perintah Tauri IPC bertugas memvalidasi payload yang masuk, memeriksa izin proses (`require_primary_instance`), mendelegasikan eksekusi ke layanan atau store domain, dan memetakan error internal menjadi `CommandError`.
2. **Ketahanan Terhadap Crash & Isolasi**: Tugas yang memakan waktu lama atau intensif sumber daya (seperti pembuatan tile mendalam dari ortomosaik GeoTIFF) dieksekusi dalam tugas latar belakang asinkron via Tokio dengan titik pemeriksaan `CancellationToken` yang aktif.
3. **Operasi Berkas Atomik**: Seluruh operasi penulisan menggunakan berkas sementara (`temp_{uuid}.tmp`) yang di-flush ke disk lalu diganti namanya secara atomik ke tujuan akhir, mencegah berkas JSON rusak atau terpotong jika terjadi mati listrik atau crash aplikasi.

---

## 2. Process Concurrency & Single-Instance Lock

Untuk menjamin bahwa banyak instans aplikasi desktop tidak melakukan operasi tulis yang saling berkonflik ke `map_storage/`, Modul 03 menerapkan penguncian berkas single-instance di `instance_lock.rs`:

- **Berkas Kunci**: `map_storage/.app.lock` menyimpan Process ID (PID) sistem operasi dari instans utama yang sedang aktif.
- **Heartbeat & Keaktifan**: Saat startup, aplikasi memeriksa apakah PID yang tercatat sesuai dengan proses yang benar-benar aktif berjalan. Jika proses sudah mati (stale lock), kunci diambil alih secara otomatis.
- **Penegakan Instans Sekunder**: Jendela atau proses sekunder beroperasi secara ketat dalam **mode baca-saja (read-only)**. Perintah Tauri yang memutasi data memeriksa kunci melalui `require_primary_instance()?`. Jika dipanggil oleh instans sekunder, perintah langsung mengembalikan error `ACCESS_DENIED`.

---

## 3. Building Documentation (`cargo doc`)

Untuk membangun rangkaian dokumentasi Rustdoc dengan penegakan tanpa peringatan (`-D warnings`), jalankan perintah berikut di shell Anda:

### PowerShell (Windows)

```powershell
# 1. Bangun dokumentasi API publik (Mode 1: tanpa item privat)
$env:RUSTDOCFLAGS="-D warnings"
cargo doc --manifest-path src-tauri/Cargo.toml --no-deps

# 2. Bangun dokumentasi lengkap termasuk item privat/internal (Mode 2: dengan item privat)
$env:RUSTDOCFLAGS="-D warnings"
cargo doc --manifest-path src-tauri/Cargo.toml --no-deps --document-private-items
```

### Bash (Linux / macOS)

```bash
# 1. Bangun dokumentasi API publik (Mode 1: tanpa item privat)
RUSTDOCFLAGS="-D warnings" cargo doc --manifest-path src-tauri/Cargo.toml --no-deps

# 2. Bangun dokumentasi lengkap termasuk item privat/internal (Mode 2: dengan item privat)
RUSTDOCFLAGS="-D warnings" cargo doc --manifest-path src-tauri/Cargo.toml --no-deps --document-private-items
```
