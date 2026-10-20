# Sample Data Module 3: Aerial Image Map Explorer

## 1. Provenance & Hak Cipta Citra Udara

- **Sumber Data Citra**: [OpenAerialMap (OAM)](https://map.openaerialmap.org/#/600fcc9055481e0005e312a1)
- **OAM Image ID**: `600fcc9055481e0005e312a1`
- **Lokasi Geografis**: Kariangau, Balikpapan, Kalimantan Timur, Indonesia
- **Sistem Koordinat (CRS)**: EPSG:4326 (WGS84)
- **Bounding Box Real**:
  - `west (min lon)`: `116.855973140003`
  - `south (min lat)`: `-1.2243110280554836`
  - `east (max lon)`: `116.863202751651`
  - `north (max lat)`: `-1.2186297002884836`
  - `center`: `[-1.221470, 116.859588]` (Zoom 16)
- **Lisensi**: Creative Commons Attribution 4.0 International (CC-BY 4.0)
- **Status Transparansi**: `nodata: None`. Format JPEG (`drone_ortho.jpg`, ~1.2 MB) digunakan untuk kompresi bundle efisien tanpa kehilangan detail visual.

## 2. Status Data Vektor & Metadata

- **`drone_ortho.jpg`**: Data raster riil hasil fotogrametri drone OpenAerialMap.
- **`markers.json`**: Data sintetis deterministik (200 titik) yang dibangkitkan oleh skrip `scripts/generate_sample_markers.py` dengan seed tetap (`42`) di dalam batas bounding box Kariangau.
- **`layers.json`**: Konfigurasi layer awal peta yang selaras dengan state frontend `INITIAL_LAYERS`.
- **`aoi.json`**: Contoh poligon Area of Interest (AOI) valid yang berada di dalam bounds Kariangau.
- **`annotations.json`**: Contoh catatan anotasi spasial point yang berada di dalam bounds Kariangau.
- **`spatial_results.json`**: Kumpulan 6 layer hasil analisis spasial (deteksi pohon, vegetasi ExG, tutupan lahan, inspeksi jalur, dan raster overlay).

## 3. Verifikasi Integritas

Setiap file di dalam direktori ini diverifikasi integritasnya melalui hash SHA-256 yang tercatat di `manifest.json`.
Skrip regenerasi marker dapat dijalankan ulang melalui:
```bash
python scripts/generate_sample_markers.py
```
