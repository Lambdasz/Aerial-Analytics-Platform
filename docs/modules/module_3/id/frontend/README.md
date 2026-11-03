# Arsitektur Frontend: Modul 03 (Aerial Image Map Explorer)

Bagian ini menyediakan gambaran arsitektur implementasi frontend untuk **Modul 03 (Aerial Image Map Explorer)**.

---

## 1. Architectural Philosophy & Technology Stack

Frontend dibangun menggunakan teknologi web dan geospasial modern yang dirancang untuk pengalaman aplikasi desktop yang responsif:

| Technology                   | Role & Responsibility                            | Key Architectural Constraints                                           |
| :--------------------------- | :----------------------------------------------- | :---------------------------------------------------------------------- |
| **React 19**                 | Rendering deklaratif komponen dan reaktivitas UI | Kepatuhan ketat terhadap Functional Programming (pure render, hooks)    |
| **TypeScript (Strict Mode)** | Keamanan tipe data lintas batas IPC dan state    | `noUnusedLocals`, `noUnusedParameters`, pengecekan null ketat           |
| **Blueprint.js (v5)**        | Desain sistem aplikasi desktop dan widget        | Nuansa aplikasi desktop asli, integrasi mode gelap, modal konsisten     |
| **Leaflet & React-Leaflet**  | Mesin peta geospasial interaktif                 | Pengikatan layer kustom, proyeksi koordinat, penskalaan high-DPI        |
| **Leaflet-Geoman**           | Penggambaran geometri dan penyuntingan vektor    | Pembuatan poligon, manipulasi sisi, dan penempelan (snapping) koordinat |
| **Tauri 2 IPC**              | Jembatan antara frontend React dan backend Rust  | Seluruh perintah diarahkan melalui pembungkus bertipe (`invokeMap`)     |

---

## 2. Atomic Design Hierarchy

Seluruh komponen UI di bawah `src/modules/module-03/` secara ketat mematuhi pola **Desain Atomik**:

```text
Atoms       -> Elemen mandiri terkecil (misal, StatusBadge)
Molecules   -> Komposisi dari atom (misal, AoiCardItem, OrthoCardItem, ResultPhoto)
Organisms   -> Unit fungsional gabungan kompleks (misal, AoiToolbar, LayerManager, MapCanvas)
Pages       -> Tampilan kontainer tingkat tata letak (misal, MapPage)
```

### Component Rules

- **Molekul Stateless**: Komponen molekul tidak boleh memicu panggilan langsung ke IPC backend. Molekul harus menerima props, menampilkan data, dan memancarkan callback aksi pengguna.
- **Organisme State Terkontrol**: Organisme mengoordinasikan hook React kustom dengan molekul anak. Organisme mengelola indikator pemuatan, banner kesalahan, dan visibilitas modal.
- **Helper Murni**: Komputasi rumit (luas Shoelace, perhitungan bounding box WGS84, templating popup HTML) didelegasikan ke fungsi utilitas murni di dalam `lib/`.

---

## 3. State Management & Hook Boundaries

State dalam Modul 03 dibagi menjadi tiga tingkatan siklus hidup yang berbeda:

1. **State Domain Persisten (Didukung Backend)**:
   - Poligon Area of Interest (`useAOI`)
   - Ortomosaik raster & progres pekerjaan (`useSpatialResultLayers`, `useAOI`)
   - Layer hasil klasifikasi/deteksi (`useSpatialResultLayers`)
   - Foto drone ber-geotag (`useImageMarkers`)
   - Dikelola melalui hook kustom yang memanggil perintah Tauri IPC dan menjaga sinkronisasi state lokal.

2. **State Interaksi Peta Efemeral (Hanya Client)**:
   - Tingkat zoom aktif, koordinat pusat, dan bounding box (`MapCanvas`)
   - State mode menggambar (bentuk aktif Geoman: poligon, persegi panjang, penanda)
   - Lapisan pengukuran jarak/luas sementara (`MeasureResultCard`)
   - Pemilihan tab sidebar (AOI, Layer, Hasil, Orto)

3. **Konteks Lintas Komponen**:
   - `ImageSyncContext`: Mengoordinasikan penyorotan gambar aktif antara penanda peta interaktif dan galeri foto sidebar.

---

## 4. Standardized Tauri IPC Integration

Semua komunikasi antara frontend dan backend Rust diarahkan melalui pembungkus terpusat `invokeMap`:

```typescript
// Berlokasi di src/modules/module-03/services/error.ts
export async function invokeMap<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (error) {
    const normalized = normalizeMapError(error);
    throw normalized;
  }
}
```

Hal ini memastikan normalisasi kesalahan yang konsisten (`MapControllerError`), penerjemahan kode kesalahan otomatis, dan fallback tiruan yang aman saat dijalankan di lingkungan peramban web mandiri tanpa runtime Tauri.
