# Pelatihan detector `deepforest` (Modul 7)

Alat untuk mengambil foto drone dari dataset UAV-VisLoc, membuat label tajuk, lalu
melakukan fine-tuning detector `deepforest`. Folder ini tidak dipasang oleh platform:
plugin hanya butuh checkpoint hasilnya.

## Kenapa DeepForest, bukan DeepLabV3+

UAV-VisLoc adalah dataset **lokalisasi visual** (foto drone + GPS + peta satelit).
Dataset ini **tidak punya label pohon**, jadi model apa pun harus diberi label dulu
sebelum bisa dilatih.

|                   | DeepForest (RetinaNet)                                | DeepLabV3+ (ResNet101/Xception)                                          |
| :---------------- | :---------------------------------------------------- | :----------------------------------------------------------------------- |
| Tanpa pelatihan   | bisa, bobot rilis sudah dilatih untuk tajuk pohon RGB | tidak, butuh checkpoint hasil latihan sendiri                            |
| Label yang dibuat | kotak per tajuk (cepat, bisa mulai dari hasil model)  | mask per piksel (lambat, harus digambar dari nol)                        |
| Keluaran          | kotak + skor per pohon, sesuai kontrak Modul 7        | mask kanopi, masih harus dipecah jadi pohon (tajuk yang menempel gabung) |

Detector `deeplabv3plus` tetap ada di registry, untuk dipakai bersama Modul 8 kalau tim
punya mask berlabel.

## Persiapan

Dari root repo, dengan venv di **luar** repo (lihat README plugin):

```powershell
py -3.10 -m venv $HOME\.venvs\aerial-m7-train
& $HOME\.venvs\aerial-m7-train\Scripts\Activate.ps1
pip install -r plugins\tree_detection\training\requirements.txt
```

Punya GPU NVIDIA? Pasang torch versi CUDA dulu dari <https://pytorch.org>, baru file di atas.

## 1. Ambil sebagian dataset (tidak perlu 22 GB)

Link Nextcloud dibaca lewat endpoint WebDAV publik, jadi hanya file yang dipilih yang
diunduh. Peta satelit (`satelliteXX.tif`, sekitar 15 GB dari 22 GB) dilewati; seluruh
foto drone (6.774 foto) sekitar 8,7 GB.

```powershell
python plugins\tree_detection\training\fetch_visloc.py --out $HOME\datasets\uav-visloc --per-flight 20
```

`--per-flight 20` mengambil 20 foto yang tersebar merata di tiap penerbangan (11 penerbangan,
sekitar 220 foto, ±300 MB). Foto berurutan saling tumpang tindih, jadi foto yang tersebar
memberi lebih banyak variasi daripada 20 foto pertama. Jalankan lagi untuk melanjutkan
unduhan yang terputus. `--flights 01 03` membatasi ke penerbangan tertentu.

Melatih langsung dari link tanpa mengunduh tidak disarankan, karena setiap epoch akan
mengunduh ulang semua foto.

## 2. Buat label awal

```powershell
python plugins\tree_detection\training\train_deepforest.py prelabel `
  --images $HOME\datasets\uav-visloc --labels $HOME\datasets\uav-visloc-labels --gsd-cm 5
```

Setiap foto diperkecil ke resolusi latih model (10 cm/px), lalu dipotong menjadi tile
400×400. Kotak hasil model rilis ditulis sebagai Pascal VOC XML di samping tiap tile.

`--gsd-cm` adalah resolusi foto. UAV-VisLoc tidak menyertakan data kamera. Angka 5 cm/px
diperoleh dengan mencoba langsung pada penerbangan 01: dengan nilai ini kotak pas dengan
tajuk, sedangkan tanpa penyekalaan kotaknya hanya setengah tajuk. Penerbangan yang lebih
tinggi (misalnya 11) bisa diberi nilai lebih besar dengan menjalankan `prelabel` terpisah
per penerbangan.

## 3. Koreksi label (bagian manual)

Buka folder label di [LabelImg](https://github.com/HumanSignal/labelImg) (`pip install labelImg`,
format Pascal VOC), atau impor sebagai Pascal VOC di CVAT / Label Studio. Lalu:

- hapus kotak yang bukan pohon (atap hijau, rumput, bayangan);
- tambahkan tajuk yang terlewat;
- sesuaikan kotak supaya pas dengan tepi tajuk;
- tile tanpa pohon dibiarkan tanpa kotak (dipakai sebagai contoh negatif).

Sekitar 150–300 tile yang sudah dikoreksi cukup untuk fine-tuning awal. Akurasi model
dibatasi oleh kualitas tahap ini.

## 4. Fine-tuning (penyetelan lanjut)

```powershell
python plugins\tree_detection\training\train_deepforest.py train `
  --labels $HOME\datasets\uav-visloc-labels --out $HOME\models\deepforest-visloc.pt --epochs 10
```

Semua tile dari satu foto masuk ke split yang sama (80% latih, 20% validasi), supaya
validasi tidak melihat potongan foto yang sudah dipakai untuk latihan. Skrip mencetak
`box_precision`, `box_recall`, dan `map_50` pada data validasi untuk model rilis dan
model hasil fine-tuning. Pakai checkpoint hanya kalau angkanya naik.

## 5. Pakai di plugin

Isi parameter plugin dengan `detector = deepforest`, `model_path = <path .pt>`, dan
`gsd_cm` sesuai foto. Kalau `model_path` dikosongkan, plugin memakai model rilis.

Checkpoint tidak di-commit ke repo karena ukurannya ±250 MB. Simpan di penyimpanan tim,
misalnya Nextcloud yang sama.
