# Aturan Pengembangan: Backend Map Controller (`src-tauri/src/map_controller/`)

Direktori ini memuat backend command Tauri untuk **Module 3: Aerial Image Map Explorer**.

## Struktur Modul (Rust 2021)

Sesuai dengan fitur Rust edisi 2021, kita **tidak lagi menggunakan format `mod.rs`** di dalam folder.
Sebagai gantinya, _entry point_ modul ini berada di file `src-tauri/src/map_controller.rs`. Jika Anda ingin memecah kode, Anda dapat meletakkan file tersebut di dalam folder `src-tauri/src/map_controller/` dan mendeklarasikannya di `map_controller.rs`.

Struktur berkas yang disarankan untuk tahap ini:

```
map_controller/
├── commands.rs   // fungsi #[tauri::command]
├── types.rs      // AOI, ImageMarker, LayerConfig, dll — naik jadi folder types/ kalau sudah >5-6 struct
└── error.rs       // MapControllerError + CommandError (lihat bagian Error Handling)
```

## ✅ Yang Boleh (dan Disarankan) Dilakukan (Do's)

- **Manfaatkan Serde**: Gunakan `serde::Serialize` dan `serde::Deserialize` untuk mengirim/menerima struktur data kompleks (seperti poligon AOI atau daftar koordinat gambar) dengan mulus ke frontend.
- **Pecah logika kompleks**: Pisahkan fungsi komputasi spasial murni ke luar fungsi `#[tauri::command]` agar dapat diuji (unit testing) dengan lebih mudah tanpa bergantung pada konteks Tauri.
- **Validasi geometri di boundary**: Sebelum AOI/polygon dari frontend disimpan atau diteruskan ke plugin lain, validasi bahwa poligonnya tertutup, tidak self-intersecting, dan minimal 3 titik. Gunakan crate `geo` untuk operasi ini — jangan percaya geometri mentah dari frontend begitu saja, karena data ini akan menjadi input bagi semua plugin analitik (Modul 4–10).
- **Gunakan `MapControllerError` di setiap command**: lihat bagian Error Handling di bawah — jangan `.map_err(|e| e.to_string())` manual di tiap command.

## ❌ Yang Tidak Boleh Dilakukan (Don'ts)

- **DILARANG menggunakan tanda hubung (`-`) pada nama file/folder**: Di Rust, nama modul harus menggunakan garis bawah (`_`). Oleh karena itu, kita menggunakan `map_controller.rs` dan folder `map_controller/`.
- **Jangan melakukan blocking berlebih**: Jika ada proses spasial yang sangat berat (misal memuat dataset besar), hindari menahan _main thread_. Gunakan `async` command Tauri atau _spawn thread_ baru.
- **Jangan mengembalikan `Result<T, String>` polos**: gunakan tipe error terstruktur (lihat bagian Error Handling) supaya frontend bisa membedakan jenis error.
- **Jangan panggil `log::error!`/`eprintln!` tersebar di tiap command**: logging error harus terjadi di satu titik konversi (lihat bagian Error Handling), bukan berulang di setiap fungsi.
- **Jangan memanggil proses Python secara sinkron/blocking dari dalam command**: ini melanggar syarat "plugin gagal tidak boleh mematikan core app" pada `AGENTS.md`.

## Error Handling: `MapControllerError` & `CommandError`

### Kegunaan

Pola ini punya dua fungsi tersendiri, jangan tercampur:

1. **`MapControllerError`** (internal, hanya dipakai di dalam Rust) — merepresentasikan _jenis-jenis_ error yang bisa terjadi di logika map_controller (geometri tidak valid, gagal I/O, dst). Dengan `enum`, kompiler memaksa Anda menangani tiap varian secara eksplisit, dan `#[from]` membuat error dari crate lain (`std::io::Error`, `geo`) otomatis bisa dikonversi lewat `?`.
2. **`CommandError`** (boundary, dikirim ke frontend lewat Serde) — bentuk error yang aman ditampilkan ke luar Rust: cuma `code` (string pendek, cocok untuk `switch`/`match` di TS) dan `message` (pesan manusiawi). Ini juga titik tunggal untuk **logging** — jadi Anda tidak perlu memanggil fungsi log manual di tiap command, cukup lempar error dan konversi otomatis akan mencatatnya.

Kenapa dipisah dua lapis? Supaya detail internal Rust (misal stack trace `std::io::Error`) tidak pernah bocor mentah-mentah ke frontend, sambil tetap menjaga error di sisi Rust tetap type-safe dan bisa di-`match`.

### Implementasi

```rust
// map_controller/error.rs
use thiserror::Error;
use serde::Serialize;

#[derive(Error, Debug)]
pub enum MapControllerError {
    #[error("geometri tidak valid: {0}")]
    InvalidGeometry(String),

    #[error("AOI tidak ditemukan: {0}")]
    AoiNotFound(String),

    #[error("gagal I/O: {0}")]
    Io(#[from] std::io::Error),
}

impl MapControllerError {
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidGeometry(_) => "INVALID_GEOMETRY",
            Self::AoiNotFound(_) => "AOI_NOT_FOUND",
            Self::Io(_) => "IO_ERROR",
        }
    }
}

#[derive(Serialize)]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl From<MapControllerError> for CommandError {
    fn from(err: MapControllerError) -> Self {
        // titik logging tunggal — semua error map_controller lewat sini
        log::error!("[map_controller] {err}");
        CommandError {
            code: err.code().to_string(),
            message: err.to_string(),
        }
    }
}
```

### Cara Pemakaian

Di dalam `commands.rs`, cukup pakai `?` seperti biasa — konversi ke `CommandError` (beserta logging-nya) terjadi otomatis lewat `From`:

```rust
// map_controller/commands.rs
use crate::map_controller::error::{CommandError, MapControllerError};
use crate::map_controller::types::AoiFeature;

#[tauri::command]
pub async fn save_aoi(aoi: AoiFeature) -> Result<AoiFeature, CommandError> {
    validate_polygon(&aoi)
        .map_err(MapControllerError::InvalidGeometry)?; // otomatis ke-log + jadi CommandError

    // ... simpan AOI

    Ok(aoi)
}
```

Anda **tidak perlu** memanggil fungsi logging secara manual di `save_aoi` — cukup pastikan error yang dilempar bertipe `MapControllerError`, sisanya (logging + format ke frontend) sudah ditangani oleh `impl From`.

> Catatan skalabilitas: pola ini sengaja generic (`code` + `message`, bukan string bebas) supaya begitu Module 2 (Plugin System) mulai butuh error handling serupa untuk "Plugin Error Isolation", `CommandError` bisa dipindah ke `src-tauri/src/error.rs` dan dipakai bersama, tanpa mengubah kontraknya ke frontend.

## Async, Blocking, dan Batas Proses Python

- Command yang melakukan komputasi spasial berat (misalnya perhitungan luas untuk poligon besar, atau memproses banyak titik) **wajib** berupa `async fn`, dan komputasi berat di dalamnya dijalankan lewat `tokio::task::spawn_blocking` agar tidak menahan _event loop_ Tauri.
- Untuk proses Python (disebutkan di `AGENTS.md` sebagai rencana untuk AI/komputasi berat, belum terintegrasi): jalankan sebagai **Tauri sidecar / subprocess terpisah**, bukan dipanggil langsung secara sinkron dari command. Ini menjaga syarat platform stability: kegagalan proses Python tidak boleh membuat proses Tauri utama ikut mati.

## Testing

Repo belum memiliki test runner terkonfigurasi. Khusus untuk `map_controller`, terapkan aturan berikut demi memenuhi tujuan dosen soal "verifiable outcomes":

- Setiap fungsi komputasi spasial murni (perhitungan luas, perimeter, validasi polygon, dsb.) yang dipisah dari `#[tauri::command]` **wajib** memiliki unit test menggunakan `#[cfg(test)]` dan dijalankan lewat `cargo test`.
- Command Tauri itu sendiri (yang bergantung pada konteks Tauri) tidak wajib diuji langsung — cukup pastikan logika inti di baliknya sudah tertutup oleh test.

## Rekomendasi Keamanan & Registrasi Command

- Setiap kali Anda membuat fungsi `#[tauri::command]` baru, Anda **wajib** meregistrasikannya di dalam makro `invoke_handler` pada file `src-tauri/src/lib.rs`.
- Konfigurasi izin (_capabilities_) pada `capabilities/default.json` harus diperbarui jika Anda mengaktifkan skema keamanan _allowlist_ untuk command kustom Anda pada Tauri v2. Daftarkan izin secara eksplisit per command baru — jangan mengandalkan izin yang terlalu luas/bawaan.
