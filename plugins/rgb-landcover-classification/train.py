"""Train the Module 8 DeepLabV3+ model on paired RGB images and label masks.

Dataset layout:
  dataset/images/*.jpg
  dataset/masks/*.png  (pixel values 0..5; 0 means unclassified)

This script intentionally requires the ML dependencies and a labelled dataset;
it does not fabricate weights.
"""
from __future__ import annotations

import argparse
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--images", type=Path, required=True)
    parser.add_argument("--masks", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--backbone", choices=("resnet101", "xception"), default="resnet101")
    parser.add_argument("--epochs", type=int, default=20)
    parser.add_argument("--learning-rate", type=float, default=1e-4)
    args = parser.parse_args()

    try:
        import torch
        from PIL import Image
        from torch.utils.data import DataLoader, Dataset
        from model import build_model
    except ImportError as exc:
        raise SystemExit(f"Training dependencies are unavailable: {exc}") from exc

    class SegmentationDataset(Dataset):
        def __init__(self):
            self.images = sorted(p for p in args.images.iterdir() if p.is_file())
            self.masks = {p.stem: p for p in args.masks.iterdir() if p.is_file()}
            if not self.images:
                raise ValueError("no training images found")
            if any(p.stem not in self.masks for p in self.images):
                raise ValueError("each image needs a mask with the same filename stem")

        def __len__(self):
            return len(self.images)

        def __getitem__(self, index):
            image = torch.from_numpy(__import__("numpy").array(Image.open(self.images[index]).convert("RGB"))).permute(2, 0, 1).float() / 255
            mask = torch.from_numpy(__import__("numpy").array(Image.open(self.masks[self.images[index].stem]).convert("L"))).long()
            return image, mask

    dataset = SegmentationDataset()
    loader = DataLoader(dataset, batch_size=1, shuffle=True)
    model = build_model(args.backbone, 5)
    optimizer = torch.optim.AdamW(model.parameters(), lr=args.learning_rate)
    model.train()
    for epoch in range(args.epochs):
        loss_total = 0.0
        for images, masks in loader:
            optimizer.zero_grad()
            loss = torch.nn.functional.cross_entropy(model(images), masks, ignore_index=0)
            loss.backward()
            optimizer.step()
            loss_total += float(loss.detach())
        print(f"epoch={epoch + 1} loss={loss_total / len(loader):.6f}")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    torch.save(model.state_dict(), args.output)
    print(f"saved weights: {args.output.resolve()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
