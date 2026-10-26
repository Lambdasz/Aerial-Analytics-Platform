# CLEANUP_LOG.md

## FASE 0: Baseline & Rollback Point

- **Tanggal**: 2026-10-26
- **Branch Kerja**: `cleanup/dead-code-2026-10-26`
- **Baseline Commit**: `96fdcab31b8f1911b9a0cfe491f1bc6b69522d75`
- **Baseline Tag**: `cleanup-baseline`

### Hasil Pengujian Baseline

| Pemeriksaan             | Perintah                    | Status              | Keterangan                                                                                                  |
| ----------------------- | --------------------------- | ------------------- | ----------------------------------------------------------------------------------------------------------- |
| ESLint                  | `npm run lint`              | **PASS**            | 0 error                                                                                                     |
| Clippy                  | `npm run lint:rust`         | **PASS**            | 0 warning                                                                                                   |
| Prettier                | `npm run format:check`      | **PASS**            | All matched files use Prettier style                                                                        |
| Rustfmt                 | `npm run format:rust:check` | **PASS**            | Sesuai format rustfmt                                                                                       |
| TypeScript & Vite Build | `npm run build`             | **PASS**            | Production bundle terbangun sukses                                                                          |
| Cargo Test              | `cargo test`                | _Pre-existing Lock_ | Binary `target/debug/aerial-analytics-platform.exe` sedang berjalan aktif di dev server `npm run tauri dev` |

---

## FASE 1 & FASE 2: Inventarisasi Kandidat & Klasifikasi Risiko

| ID       | Lokasi                                                                                                                 | Kategori                   | Deskripsi & Bukti                                                                                                                                                                | Klasifikasi              |
| -------- | ---------------------------------------------------------------------------------------------------------------------- | -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------ |
| **K-01** | `map_storage/layers/ortho_8881_148640_1792951376935076.json`                                                           | A (Sisa Data Test)         | Berkas layer sisa unit test Rust `test_reconcile_trash_restored_if_layer_exists` (layer_name: _"Trash Restore Test"_). Folder citra fisik `map_storage/images/` sudah tidak ada. | **AMAN** (Fase 3A)       |
| **K-02** | `src/modules/module-03/organisms/AoiToolbar.tsx:17`                                                                    | B (Unused Type Export)     | `export type AoiDemoToolbarProps = AoiToolbarProps;` - Alias tipe demo tidak memiliki referensi impor di seluruh project.                                                        | **AMAN** (Fase 3A)       |
| **K-03** | `src/modules/module-03/services/api.ts:187-189`                                                                        | B (Dead Wrapper Function)  | `export function getSampleAnnotations(): Promise<AnnotationFeature[]> { return getSavedAnnotations(); }` - Wrapper backward-compat usang yang tidak dipanggil di mana pun.       | **AMAN** (Fase 3A)       |
| **K-04** | `src-tauri/src/modules/module_03/sp_result/reader.rs:20` & `src/modules/module-03/hooks/useSpatialResultLayers.ts:340` | A (Mock/Dummy Dev Command) | `get_dummy_spatial_layers` - Ditandai dev/testing di `lib.rs`, namun saat ini masih dipakai sebagai fallback sekunder jika `/storage/spatial_results/` kosong atau gagal dibaca. | **PERLU KEPUTUSAN USER** |

---

## FASE 3: Log Eksekusi Pembersihan

- **Langkah 1 [K-01]**: Pembersihan data sisa test `ortho_8881_148640_1792951376935076.json` ("Trash Restore Test") di `map_storage/layers/`. File telah dipastikan dibersihkan dari direktori storage.
- **Langkah 2 [K-02]**: Penghapusan unused type alias `AoiDemoToolbarProps` di `src/modules/module-03/organisms/AoiToolbar.tsx`. Verifikasi linting dan build passed.
