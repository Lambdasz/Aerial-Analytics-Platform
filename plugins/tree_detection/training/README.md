# Training the `deepforest` detector (Module 7)

Tools to fetch drone photos from the UAV-VisLoc dataset, create crown labels, and then fine-tune
the `deepforest` detector. This folder is not installed by the platform: the plugin only needs
the resulting checkpoint.

## Why DeepForest, not DeepLabV3+

UAV-VisLoc is a **visual localization** dataset (drone photos + GPS + satellite maps).
It has **no tree labels**, so any model has to be labelled first before it can be trained.

|                  | DeepForest (RetinaNet)                                  | DeepLabV3+ (ResNet101/Xception)                                       |
| :--------------- | :------------------------------------------------------ | :-------------------------------------------------------------------- |
| Without training | works, released weights are trained for RGB tree crowns | no, needs a checkpoint you trained yourself                           |
| Labels to create | one box per crown (fast, can start from model output)   | per-pixel masks (slow, must be drawn from scratch)                    |
| Output           | box + score per tree, matching the Module 7 contract    | canopy mask, still has to be split into trees (touching crowns merge) |

The `deeplabv3plus` detector stays in the registry, to be used together with Module 8 if the team
has labelled masks.

## Setup

From the repo root, with the venv **outside** the repo (see the plugin README):

```powershell
py -3.10 -m venv $HOME\.venvs\aerial-m7-train
& $HOME\.venvs\aerial-m7-train\Scripts\Activate.ps1
pip install -r plugins\tree_detection\training\requirements.txt
```

Have an NVIDIA GPU? Install the CUDA build of torch from <https://pytorch.org> first, then the
file above.

## 1. Fetch part of the dataset (no need for all 22 GB)

The Nextcloud link is read through the public WebDAV endpoint, so only the selected files are
downloaded. Satellite maps (`satelliteXX.tif`, about 15 GB of the 22 GB) are skipped; all drone
photos (6,774 photos) come to about 8.7 GB.

```powershell
python plugins\tree_detection\training\fetch_visloc.py --out $HOME\datasets\uav-visloc --per-flight 20
```

`--per-flight 20` takes 20 photos spread evenly across each flight (11 flights, about 220 photos,
~300 MB). Consecutive photos overlap each other, so spread-out photos give more variety than the
first 20 photos. Run it again to resume an interrupted download. `--flights 01 03` limits it to
specific flights.

Training straight from the link without downloading is not recommended, because every epoch
would download all the photos again.

## 2. Create initial labels

```powershell
python plugins\tree_detection\training\train_deepforest.py prelabel `
  --images $HOME\datasets\uav-visloc --labels $HOME\datasets\uav-visloc-labels --gsd-cm 5
```

Each photo is downscaled to the model's training resolution (10 cm/px) and then cut into
400×400 tiles. The released model's boxes are written as Pascal VOC XML next to each tile.

`--gsd-cm` is the photo resolution. UAV-VisLoc does not include camera data. The value 5 cm/px
was found by trying it directly on flight 01: with this value the boxes fit the crowns, whereas
without rescaling the boxes cover only half a crown. Higher flights (e.g. 11) can be given a
larger value by running `prelabel` separately per flight.

## 3. Correct the labels (the manual part)

Open the label folder in [LabelImg](https://github.com/HumanSignal/labelImg) (`pip install labelImg`,
Pascal VOC format), or import it as Pascal VOC into CVAT / Label Studio. Then:

- delete boxes that are not trees (green roofs, grass, shadows);
- add crowns that were missed;
- adjust boxes so they fit the crown edges;
- leave tiles without trees empty (they serve as negative examples).

About 150–300 corrected tiles are enough for an initial fine-tune. Model accuracy is limited by
the quality of this step.

## 4. Fine-tuning

```powershell
python plugins\tree_detection\training\train_deepforest.py train `
  --labels $HOME\datasets\uav-visloc-labels --out $HOME\models\deepforest-visloc.pt --epochs 10
```

All tiles from one photo go into the same split (80% training, 20% validation), so validation
never sees pieces of a photo already used for training. The script prints `box_precision`,
`box_recall`, and `map_50` on the validation data for both the released model and the fine-tuned
model. Use the checkpoint only if the numbers go up.

## 5. Use it in the plugin

Set the plugin parameters to `detector = deepforest`, `model_path = <path to .pt>`, and a `gsd_cm`
that matches the photos. If `model_path` is left empty, the plugin uses the released model.

The checkpoint is not committed to the repo because it is about 250 MB. Store it in the team's
storage, for example the same Nextcloud.
