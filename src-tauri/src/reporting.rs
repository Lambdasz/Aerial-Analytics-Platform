//! # Module 11: Aerial Analytics Dashboard & Reporting — Report + Export
//!
//! Backend untuk dua fitur Module 11: **Report Generation** dan **Data Export**.
//!
//! Modul ini **tidak mengekspos fungsi publik untuk dipakai modul lain**: Module 11
//! adalah titik akhir (leaf) dari alur data platform — dia mengonsumsi hasil dari
//! modul analitik lain lalu menyajikannya sebagai laporan/berkas ekspor, tidak ada
//! modul lain yang perlu memanggil balik kode di sini.
//!
//! ## Sumber data (kontrak yang sudah disepakati)
//!
//! | Sumber | Fungsi/keluaran | Dipakai untuk |
//! |---|---|---|
//! | Module 9.6 — Plot Ranking | daftar plot terurut + peringkat | bagian perbandingan plot di laporan |
//! | Module 9.7 — Plot Summary | ringkasan JSON terstruktur per plot | bagian ringkasan analitik per plot |
//! | Module 10.8 — Change Visualization, lewat `get_temporal_change_result()` | data perubahan antar observation period | bagian ringkasan temporal (M11.5) |
//!
//! Ketiga fungsi sumber itu **milik Module 9 dan Module 10**, belum ada di repo ini.
//! Supaya modul ini tetap bisa dikompilasi dan didokumentasikan sekarang, tipe-tipe
//! kontraknya sengaja **didefinisikan ulang secara lokal** sebagai representasi
//! sementara di [`types::input`] — lihat catatan penting di modul tersebut.
//!
//! ## Struktur
//!
//! - [`types`] — kontrak data: tipe input (sementara, dari M9/M10) dan tipe output
//!   (model Report + Export milik Module 11 sendiri).
//! - [`error`] — `ReportingError` (internal) dan `CommandError` (batas ke frontend).
//! - [`aggregator`] — **fungsi murni**: menggabungkan data dari M9.6/M9.7/M10.8
//!   menjadi model laporan siap-render. Tidak ada I/O, mudah di-unit-test.
//! - [`report`] — **fungsi bersisi efek samping**: merender model laporan menjadi
//!   berkas (PDF).
//! - [`export`] — **fungsi bersisi efek samping**: menulis data ke berkas ekspor
//!   (Excel/CSV/GeoJSON).
//! - [`commands`] — batas Tauri (`#[tauri::command]`): mengorkestrasi alur
//!   ambil-data → agregasi murni → render/ekspor bersisi efek samping.
//!
//! Lihat `RULES_REPORTING.md` di folder ini untuk aturan pengembangan lengkap
//! (pemisahan pure/side-effect, error handling, async/blocking, testing).
//!
//! ## Status
//!
//! Seluruh fungsi di bawah modul ini masih berupa **kerangka kosong**
//! (`todo!()`/`unimplemented!()`) — signature dan rustdoc sudah final berdasarkan
//! kontrak input/output yang disepakati, implementasi menyusul. Command di sini
//! **belum didaftarkan** di `invoke_handler!` pada `lib.rs`; itu langkah terakhir
//! begitu implementasi nyata siap.
#![allow(dead_code)] // TODO(M11): hapus setelah commands.rs benar-benar memanggil fungsi di bawah ini.

pub mod aggregator;
pub mod commands;
pub mod error;
pub mod export;
pub mod report;
pub mod types;
