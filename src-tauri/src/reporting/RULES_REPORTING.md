# Aturan Pengembangan: Backend Reporting (`src-tauri/src/reporting/`)

Direktori ini memuat backend command Tauri untuk **Module 11: Report Generation
+ Data Export** (bagian dari Aerial Analytics Dashboard & Reporting).

Dokumen ini mengikuti pola yang sama dengan
`src-tauri/src/map_controller/RULES_MAP_CONTROLLER.md` (Module 3), disesuaikan
untuk kebutuhan khusus Module 11.

## Struktur Modul (Rust 2021)

Sesuai Rust edisi 2021, tidak memakai format `mod.rs`. Entry point ada di
`src-tauri/src/reporting.rs`; submodul dipecah di dalam folder
`src-tauri/src/reporting/`:

```
reporting/
├── types.rs         // entry, deklarasi submodul types/
├── types/
│   ├── input.rs      // kontrak SEMENTARA dari Module 9 (M9.6, M9.7) & Module 10 (M10.8)
│   └── output.rs      // model Report + Export milik Module 11 sendiri
├── error.rs          // ReportingError + CommandError
├── aggregator.rs      // fungsi MURNI — gabungkan data jadi model laporan/ekspor
├── report.rs          // efek samping — render laporan (PDF) ke disk
├── export.rs          // efek samping — tulis berkas ekspor (CSV/Excel/GeoJSON) ke disk
└── commands.rs         // #[tauri::command] — orkestrasi: ambil data → aggregator → report/export
```

## Prinsip Utama: Pemisahan Pure vs Side-Effect

Ini aturan paling penting di modul ini (selaras dengan tuntutan dosen soal
*functional thinking*):

- **`aggregator.rs` wajib 100% murni**: tidak ada I/O, tidak `async`, tidak
  memanggil command Tauri lain. Input dan output hanya berupa nilai/struct.
  Karena murni, fungsi di sini **wajib diuji lewat `#[cfg(test)]`**.
- **`report.rs` dan `export.rs` berisi efek samping** (tulis berkas ke disk).
  Fungsi di sini menerima model yang sudah jadi (`ReportDocument`,
  `ExportTable`) — **tidak boleh** menggabungkan/menyusun data sendiri, itu
  tugas `aggregator.rs`.
- **`commands.rs` mengorkestrasi**, tidak berisi logika penggabungan data
  sendiri: ambil data dari Module 9/10 → panggil `aggregator.rs` (murni) →
  panggil `report.rs`/`export.rs` (efek samping).

Kalau ragu taruh suatu fungsi di mana: kalau dia butuh `&Path`/tulis berkas/
`async`, dia bukan di `aggregator.rs`.

## ⚠️ Sumber Data: Module 11 Tidak Mengubah Kode Modul Lain

Module 11 **hanya mengonsumsi** fungsi publik Module 9 (`M9.6`, `M9.7`) dan
Module 10 (`get_temporal_change_result()` — `M10.8`). Module 11 **tidak
mengekspos fungsi publik untuk modul lain** — dia modul terakhir yang
menampilkan data.

Karena Module 9 dan Module 10 belum punya kode Rust di repo ini, kontrak
data mereka untuk sementara **direplikasi secara lokal** di
`types/input.rs`, ditandai jelas sebagai tipe sementara. Ini artinya:

- **Jangan pernah** menyalin/mengedit file di `src-tauri/src/plugin_manager/`
  atau folder Module 9/10 (begitu ada) untuk "menyesuaikan" kontrak.
- Begitu Module 9/Module 10 mem-publish tipe resmi mereka, tipe di
  `types/input.rs` dihapus dan diganti `use` langsung ke tipe asli —
  koordinasikan lewat PR terpisah dengan pemilik modul terkait, jangan
  diam-diam mengganti kontrak.

## ✅ Yang Boleh (dan Disarankan) Dilakukan (Do's)

- Manfaatkan Serde (`Serialize`/`Deserialize`) untuk semua tipe di `types/`,
  sama seperti modul lain.
- Command yang menulis berkas ke disk **wajib** `async fn` (lihat bagian
  Async & Blocking).
- Gunakan `ReportingError` + `CommandError` (lihat `error.rs`) di setiap
  command — jangan `.map_err(|e| e.to_string())` manual.

## ❌ Yang Tidak Boleh Dilakukan (Don'ts)

- **DILARANG** tanda hubung (`-`) di nama file/folder — pakai garis bawah.
- **Jangan** menaruh logika penggabungan data (menyandingkan ranking dengan
  summary, memetakan hasil temporal, dst) di `commands.rs` atau di
  `report.rs`/`export.rs` — itu harus di `aggregator.rs` supaya bisa diuji
  tanpa konteks Tauri.
- **Jangan** mengembalikan `Result<T, String>` polos — pakai `CommandError`.
- **Jangan** mengubah file Module 9/Module 10 untuk "membetulkan" kontrak —
  lihat bagian sumber data di atas.

## Error Handling: `ReportingError` & `CommandError`

Pola sama seperti `map_controller` (lihat `RULES_MAP_CONTROLLER.md`), dua
lapis: `ReportingError` (internal, `enum` lewat `thiserror`) dan
`CommandError` (boundary ke frontend, `code` + `message`). Implementasi ada di
`error.rs` — pakai `?` seperti biasa di `commands.rs`, konversi ke
`CommandError` otomatis lewat `impl From`.

## Async, Blocking, dan Batas Proses Python

- Command yang menulis berkas (PDF/Excel/CSV berukuran besar) **wajib**
  `async fn`; kalau prosesnya berat/lama, jalankan lewat
  `tokio::task::spawn_blocking` agar tidak menahan event loop Tauri — sama
  seperti aturan di `map_controller`.
- Reporting **tidak** memanggil proses Python secara langsung. Kalau nanti
  butuh data yang berasal dari plugin Python, itu masuk lewat kontrak standar
  Module 2 (`ExecutionResult`), bukan panggilan langsung.

## Testing

- Setiap fungsi di `aggregator.rs` **wajib** punya unit test (`#[cfg(test)]`,
  `cargo test`) begitu diimplementasikan — ini fungsi paling gampang diuji di
  seluruh modul karena tidak ada I/O.
- Fungsi di `report.rs`/`export.rs`/`commands.rs` (bergantung pada I/O/konteks
  Tauri) tidak wajib diuji langsung — cukup pastikan logika di baliknya sudah
  tertutup test lewat `aggregator.rs`.

## Registrasi Command

- Command baru **wajib** didaftarkan di `invoke_handler!` pada
  `src-tauri/src/lib.rs`, dan izinnya ditambahkan secara eksplisit di
  `capabilities/default.json` — **jangan** dilakukan dulu selama isi command
  masih `todo!()` (lihat status di `reporting.rs`), supaya command yang pasti
  panic tidak bisa terpanggil dari frontend secara tidak sengaja.
