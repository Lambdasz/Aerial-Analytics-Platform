# Log Implementasi - Person 1 (Map Core & Image System)

**Modul 3: Aerial Image Map Explorer**

Dokumen ini merekam jejak arsitektural dan implementasi teknis untuk fitur-fitur yang dikerjakan oleh Orang 1 menggunakan React, Leaflet, dan Blueprint.js.

## 👤 Rasionalisasi Peran & Beban Kerja

**Orang 1: Map Core & Image System (Fokus: UI Dasar & Interaksi Peta)**

Orang pertama memegang fondasi peta dan semua hal yang berkaitan dengan menampilkan lokasi foto.
Fitur yang dipegang:

- Fitur 1: Interactive Map Explorer
- Fitur 2: Image Location Marker
- Fitur 3: Image Preview & Popup
- Fitur 7: Map ↔ Image Interaction
- Fitur 8: Map View Control

**Kenapa ini adil?**
Meskipun jumlah fiturnya terlihat banyak (5 fitur), kelima fitur ini sangat berkaitan erat dan saling menumpuk. Setelah peta (Fitur 1) berhasil dibuat, membuat marker (Fitur 2 & 3) dan mengontrol pergeseran layarnya (Fitur 7 & 8) adalah satu alur kerja bawaan dari library Leaflet. Beban kerja utamanya ada pada sinkronisasi klik antara peta dan antarmuka galeri foto.

---

## _**Implementing Feature 1 (Interactive Map Explorer)**_

**1. Manajemen Dependensi (NPM Packages):**

- **Sudah Terinstall Bawaan**: `leaflet`, `react-leaflet`, `@types/leaflet`, `@blueprintjs/core`.

**2. Arsitektur Basemap Switcher:**

- Mengembangkan kanvas peta utama menggunakan `<MapContainer>` dan mematikan kontrol zoom _default_ (`zoomControl={false}`) untuk digantikan dengan kontrol kustom di sprint berikutnya.
- Menyediakan 3 opsi _Basemap_ untuk konteks analisis spasial yang berbeda:
  1. **OpenStreetMap (OSM)**: Untuk orientasi jalan dan tata letak kota (Default).
  2. **Esri Satellite**: Untuk komparasi citra dunia nyata.
  3. **CartoDB Positron**: Untuk _high-contrast_ penonjolan titik data (berwarna abu-abu pucat).
- **Keamanan & Konfigurasi API**: Mengimplementasikan injeksi variabel lingkungan (`VITE_CARTO_API_KEY`) untuk mengakomodasi regulasi terbaru CartoDB (Agustus 2026) yang mewajibkan API Key gratis untuk menghilangkan _watermark_.

**3. Penyesuaian Layout Ekosistem Desktop:**

- Membersihkan CSS _boilerplate_ bawaan Tauri.
- Memaksa wadah aplikasi untuk menempati `100vw` dan `100vh` dengan `overflow: hidden` demi mencegah _scrolling_ halaman web ganda pada aplikasi desktop.

**Located in (Peta Perubahan Berkas):**

- `src/map/components/MapCanvas.tsx` **[NEW]** — _Komponen inti pembuat kanvas peta dan tombol Blueprint.js untuk Basemap Switcher_.
- `src/map/components/MapCanvas.css` **[NEW]** — _Styling flexbox absolut untuk peta_.
- `src/main.tsx` **[MODIFIED]** — _Import global CSS untuk Leaflet dan Blueprint_.
- `src/App.tsx` & `src/App.css` **[MODIFIED]** — _Pembersihan boilerplate dan setup Flexbox container_.

---

## _**Implementing Feature 2 (Image Location Marker) & Feature 3 (Image Preview Popup)**_

**1. Manajemen Dependensi (NPM Packages):**

- **NPM Ter-install**: `react-leaflet-cluster`, `leaflet.markercluster`, dan `@types/leaflet.markercluster`.
- **Justifikasi**: Wajib dipasang untuk mencekal _browser freeze_ (lag) saat Leaflet dipaksa merender lebih dari 500+ titik koordinat secara bersamaan. Pustaka ini memecah tumpukan titik koordinat menjadi satu angka "Cluster" yang membesar saat peta di-zoom.

**2. Kontrak Data Spasial & Dummy Generator:**

- Membuat _interface_ TypeScript yang solid (`DroneImageMetadata`) sebagai "sumber kebenaran tunggal" antara Peta dan Backend Rust (Tauri).
- Meracik _Custom Hook_ murni (`useImageMarkers.ts`) untuk men-generate ratusan data koordinat acak dan telemetri (termasuk _Altitude AGL_ dan _Heading_) agar pengujian rendering UI bisa dilakukan meskipun modul backend belum siap.

**3. Kustomisasi Simbologi Marker Spasial (SVG):**

- Meninggalkan penanda _default_ biru dari Leaflet.
- Membangun `L.divIcon` yang menyuntikkan vektor SVG berbentuk panah/drone secara dinamis. Vektor tersebut memutar (_rotate_) dirinya sendiri berdasar derajat arah hadap kamera drone (`heading_deg`).

**4. UI Pratinjau Telemetri (Glassmorphism):**

- Membuat _Popup Card_ menggunakan murni CSS _backdrop-filter_ tanpa bantuan framework Tailwind CSS, demi menjaga keterikatan visual dengan Blueprint.js.
- Popup menyajikan _thumbnail_ gambar (lazy-loaded), koordinat desimal, dan informasi ketinggian.

**Located in (Peta Perubahan Berkas):**

- `src/map/types/marker.ts` **[NEW]** — _Kontrak tipe data metadata gambar drone_.
- `src/map/components/useImageMarkers.ts` **[NEW]** — _Generator data dummy statis_.
- `src/map/components/DroneImageMarker.tsx` **[NEW]** — _Komponen SVG marker kustom dan logika orientasi arah_.
- `src/map/components/ImagePopupCard.tsx` & `.css` **[NEW]** — _Kartu informasi dengan efek tembus pandang (glassmorphism)_.
- `src/map/components/MapCanvas.tsx` **[MODIFIED]** — _Membungkus iterasi marker ke dalam `<MarkerClusterGroup>`_.

---

## _**Implementing Feature 7 (Map ↔ Image Interaction - Two Way Sync)**_

**1. Manajemen Dependensi (NPM Packages):**

- **NPM Ter-install**: Tidak ada paket tambahan untuk fitur ini (0 dependencies).
- **Justifikasi**: Eksekusi instalasi `zustand` sengaja dibatalkan untuk meminimalisir pembengkakan dependensi pihak ketiga. Solusi sepenuhnya diracik murni menggunakan **React Context API** (`createContext`, `useContext`, `useState`).

**2. Arsitektur State Management:**

- Fitur ini memperkenalkan _Global State Provider_ (`<ImageSyncProvider>`) yang membungkus komponen peta dan galeri agar keduanya merujuk pada satu _Single Source of Truth_.
- **Pencegahan Race Condition / Infinite Loop:** _State_ tidak hanya menyimpan ID gambar (`activeImageId`), tetapi juga menyimpan `interactionSource` (`'map'` atau `'gallery'`). Ini mencegah "Efek Ping-Pong" di mana Peta memerintahkan Galeri untuk bergeser, lalu Galeri balik memerintahkan Peta untuk bergeser secara terus-menerus hingga aplikasi _crash_.

**3. Pemisahan Concern & Functional Principles (Alur A):**

- Mematuhi prinsip FP untuk memisahkan _side-effect_ dari visual UI, maka fungsi pergerakan Leaflet (`map.flyTo`) **tidak** diletakkan di dalam tombol Sidebar.
- Sebagai gantinya, dibuat komponen pengamat kasat mata bernama `<MapSyncHandler />` yang hidup di dalam ekosistem peta. Komponen murni ini bertugas "mendengarkan" perubahan di _Global State_, lalu menggerakkan koordinat peta menggunakan referensi instansiasi Leaflet bawaan (`useMap()`).

**4. Interaksi UI Programatis (Alur B & Fitur Lanjutan):**

- **Di sisi Peta (`DroneImageMarker`)**: Menggunakan `useRef<L.Marker>` untuk mengakses _object_ murni Leaflet. Ketika _state_ berubah dari Galeri, Marker tidak hanya berubah ikon SVG-nya menjadi oranye bersinar secara reaktif, tetapi kode juga memanggil `markerRef.current?.openPopup()` untuk membuka kartu popup secara programatis.
- **Di sisi Galeri (`ImageGallerySidebar`)**: Memanfaatkan DOM API bawaan browser `scrollIntoView({ behavior: 'smooth', block: 'center' })` agar _scrollbar_ panel secara otomatis menggulir mulus dan memusatkan elemen _thumbnail_ yang sesuai ketika sebuah titik diklik di atas peta.

**Located in (Peta Perubahan Berkas):**

- `src/map/store/ImageSyncContext.tsx` **[NEW]** — _Fondasi Context API dan Custom Hook_.
- `src/map/components/MapSyncHandler.tsx` **[NEW]** — _Jembatan efek samping (side-effect) animasi `flyTo`_.
- `src/map/components/ImageGallerySidebar.tsx` & `.css` **[NEW]** — _UI Panel Navigasi Galeri Blueprint.js dan logika auto-scroll_.
- `src/map/components/DroneImageMarker.tsx` **[MODIFIED]** — _Inject event listener klik dan programatis popup_.
- `src/map/components/MapCanvas.tsx` **[MODIFIED]** — _Penyisipan handler sinkronisasi ke dalam kanvas Leaflet_.
- `src/App.tsx` & `src/App.css` **[MODIFIED]** — _Pembungkusan aplikasi dengan Provider dan implementasi layout Flexbox terbelah (Sidebar vs Kanvas)_.

---

## _**Implementing Feature 8 (Map View Control)**_

**1. Manajemen Dependensi (NPM Packages):**

- **NPM Ter-install**: Tidak ada dependensi baru (0 dependencies).
- **Justifikasi**: Skala peta menggunakan komponen bawaan `react-leaflet`, dan fitur mode layar penuh (_fullscreen_) menggunakan murni **HTML5 Web API native**. Hal ini menjaga aplikasi tetap _lightweight_.

**2. Pemisahan Concern (Control Bar Overlay):**

- Membuat komponen independen `<MapViewControlBar />` yang mengambang (melayang) di atas peta menggunakan CSS `absolute` dan `z-index`.
- Memanfaatkan _hook_ bawaan `useMap()` dari react-leaflet agar komponen ini dapat memerintahkan Leaflet engine secara langsung.

**3. Algoritma Perhitungan Batas Ekstrem (FitBounds):**

- Alih-alih membuat pan/zoom statis, tombol _FitBounds_ dibuat dinamis dengan mengiterasi seluruh titik data drone (_markers_).
- Sistem menciptakan batas _envelope_ spasial menggunakan `L.latLngBounds()` lalu memerintahkan kamera (`map.fitBounds`) untuk terbang secara animasi membingkai seluruh area proyek dengan _padding_ 40px agar terlihat lega dan estetis.

**4. Interaksi Fullscreen Native & Sinkronisasi State Zoom:**

- **Zoom Indicator**: Menggunakan `useEffect` murni yang bereaksi terhadap _event_ `zoomend` bawaan Leaflet. Teks level zoom akan terupdate secara seketika (_real-time_) setiap kali roda _mouse_ diputar.
- **Fullscreen API**: Menjalankan rutin imperatif native `document.documentElement.requestFullscreen()` yang dibungkus dengan `try-catch` yang kokoh, serta mendeteksi jika sedang dalam mode layar penuh untuk menjalankan `document.exitFullscreen()`.

**Located in (Peta Perubahan Berkas):**

- `src/map/components/MapViewControlBar.tsx` & `.css` **[NEW]** — _Komponen panel navigasi melayang (Zoom In/Out, Indikator, FitBounds, Reset North, Fullscreen)_.
- `src/map/components/MapCanvas.tsx` **[MODIFIED]** — _Penghapusan zoom bawaan Leaflet dan penyisipan `<MapViewControlBar />` serta `<ScaleControl />`_.

---

## 🛠️ Penyesuaian Integrasi Akhir (Strict Linting & Security Hooks)

Sebagai akibat dari integrasi kode kita ke dalam _branch_ landasan repositori terbaru yang menerapkan pengawasan kode otomatis (_Husky pre-commit hooks_), beberapa penyesuaian teknis mikroskopis diterapkan agar pekerjaan Person 1 lolos standar kualitas industri:

1. **Strict TypeScript Promises (`MapViewControlBar.tsx`):**
   - Penambahan operator `void` secara eksplisit (`void document.exitFullscreen()`) untuk menandakan bahwa kita sadar mengabaikan _floating promise_ sesuai tuntutan _rules_ `@typescript-eslint/no-floating-promises`.
   - Pengetatan validasi _error handling_ pada fungsi _Fullscreen_. Variabel `catch` yang secara bawaan bertipe `any` diubah menjadi blok pengecekan kokoh `err instanceof Error` untuk menghindari teguran ketat dari `@typescript-eslint/no-unsafe-member-access`.
2. **Vite Fast Refresh Rules (`ImageSyncContext.tsx`):**
   - Menambahkan pelindung linter `/* eslint-disable react-refresh/only-export-components */`. Hal ini dilakukan karena _file state_ mengekspor _Provider_ dan _Custom Hook_ secara bersamaan. Secara arsitektur _Context_, ini sangat lazim dan aman, namun linter bawaan Vite memperingatkannya.
3. **Automated Styling (Prettier):**
   - Seluruh baris kode yang ditulis telah dilewatkan pada perintah `npm run format`, sehingga indentasi, penggunaan tanda kutip (_quotes_), dan struktur spasi 100% konsisten dengan standar _Prettier_ milik tim _frontend_ lainnya.
