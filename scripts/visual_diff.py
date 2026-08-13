from __future__ import annotations

from dataclasses import asdict, dataclass
from pathlib import Path

from PIL import Image, ImageChops, ImageEnhance, ImageStat


@dataclass(frozen=True)
class DiffResult:
    width: int
    height: int
    changed_pixels: int
    changed_ratio: float
    mean_channel_delta: float
    max_channel_delta: int
    passed: bool

    def to_dict(self) -> dict:
        return asdict(self)


def compare_images(
    baseline_path: Path,
    candidate_path: Path,
    diff_path: Path,
    *,
    pixel_threshold: int = 16,
    max_changed_ratio: float = 0.005,
    max_mean_delta: float = 1.0,
) -> DiffResult:
    baseline = Image.open(baseline_path).convert("RGB")
    candidate = Image.open(candidate_path).convert("RGB")
    if baseline.size != candidate.size:
        raise ValueError(f"image dimensions differ: {baseline.size} != {candidate.size}")

    diff = ImageChops.difference(baseline, candidate)
    masks = [channel.point(lambda value: 255 if value > pixel_threshold else 0) for channel in diff.split()]
    threshold_mask = ImageChops.lighter(ImageChops.lighter(masks[0], masks[1]), masks[2])
    histogram = threshold_mask.histogram()
    changed = sum(histogram[1:])
    pixel_count = baseline.width * baseline.height
    statistics = ImageStat.Stat(diff)
    mean_delta = sum(statistics.mean) / 3 if pixel_count else 0.0
    max_delta = max(channel[1] for channel in diff.getextrema())
    ratio = changed / pixel_count if pixel_count else 0.0
    passed = ratio <= max_changed_ratio and mean_delta <= max_mean_delta

    visible = ImageEnhance.Brightness(diff.convert("RGB")).enhance(4.0)
    diff_path.parent.mkdir(parents=True, exist_ok=True)
    visible.save(diff_path)
    return DiffResult(
        width=baseline.width,
        height=baseline.height,
        changed_pixels=changed,
        changed_ratio=ratio,
        mean_channel_delta=mean_delta,
        max_channel_delta=max_delta,
        passed=passed,
    )
