"""DeepLabV3+ model factory for Module 8.

Weights are intentionally external: a segmentation architecture without trained
weights must not be presented as a working classifier.
"""
from __future__ import annotations


def build_model(backbone: str, num_classes: int):
    if backbone not in {"resnet101", "xception"}:
        raise ValueError("backbone must be 'resnet101' or 'xception'")
    try:
        import torch.nn as nn
        import torchvision.models as tv_models
    except ImportError as exc:
        raise RuntimeError("PyTorch and torchvision are required for inference") from exc

    class ConvBNReLU(nn.Sequential):
        def __init__(self, in_channels: int, out_channels: int, kernel: int = 3, dilation: int = 1):
            padding = dilation if kernel == 3 else 0
            super().__init__(
                nn.Conv2d(in_channels, out_channels, kernel, padding=padding, dilation=dilation, bias=False),
                nn.BatchNorm2d(out_channels),
                nn.ReLU(inplace=True),
            )

    class ASPP(nn.Module):
        def __init__(self, channels: int, output: int = 256):
            super().__init__()
            self.blocks = nn.ModuleList([
                ConvBNReLU(channels, output, 1),
                ConvBNReLU(channels, output, 3, 6),
                ConvBNReLU(channels, output, 3, 12),
                ConvBNReLU(channels, output, 3, 18),
            ])
            self.pool_project = ConvBNReLU(channels, output, 1)
            self.project = ConvBNReLU(output * 5, output, 1)

        def forward(self, x):
            import torch
            size = x.shape[-2:]
            pooled = torch.nn.functional.adaptive_avg_pool2d(x, 1)
            pooled = torch.nn.functional.interpolate(pooled, size=size, mode="bilinear", align_corners=False)
            pooled = self.pool_project(pooled)
            branches = [block(x) for block in self.blocks]
            return self.project(torch.cat(branches + [pooled], dim=1))

    class DeepLabV3Plus(nn.Module):
        def __init__(self):
            super().__init__()
            if backbone == "resnet101":
                encoder = tv_models.resnet101(weights=None)
                self.low = nn.Sequential(*list(encoder.children())[:5])
                self.high = nn.Sequential(*list(encoder.children())[5:-2])
                high_channels, low_channels = 2048, 256
            else:
                try:
                    import timm
                    encoder = timm.create_model("xception41", pretrained=False, features_only=True)
                except ImportError as exc:
                    raise RuntimeError("timm is required for the xception backbone") from exc
                self.low = encoder
                self.high = None
                high_channels, low_channels = 2048, 256
            self.aspp = ASPP(high_channels)
            self.low_project = ConvBNReLU(low_channels, 48, 1)
            self.decoder = nn.Sequential(ConvBNReLU(304, 256), ConvBNReLU(256, 256))
            self.classifier = nn.Conv2d(256, num_classes, 1)

        def forward(self, x):
            import torch
            import torch.nn.functional as F
            if backbone == "resnet101":
                low = self.low(x)
                high = self.high(low)
            else:
                features = self.low(x)
                low, high = features[1], features[-1]
            high = self.aspp(high)
            high = F.interpolate(high, size=low.shape[-2:], mode="bilinear", align_corners=False)
            out = self.decoder(torch.cat([high, self.low_project(low)], dim=1))
            return F.interpolate(self.classifier(out), size=x.shape[-2:], mode="bilinear", align_corners=False)

    return DeepLabV3Plus()
