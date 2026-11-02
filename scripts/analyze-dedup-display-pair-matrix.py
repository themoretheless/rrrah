"""Diagnostic only: compare diagonal and full affine RGB fits on held-out grid points."""
import hashlib
import json
import sys
from pathlib import Path

import numpy as np


def analyze(path):
    data = np.loadtxt(path, delimiter=",", skiprows=1, ndmin=2)
    if data.shape[1] != 8 or not np.isfinite(data).all():
        raise ValueError("expected finite x,y and two RGB triples")
    source, target = data[:, 2:5], data[:, 5:8]
    in_range = ((source >= 0) & (source <= 1) & (target >= 0) & (target <= 1)).all(axis=1)
    training = ((data[:, 0].astype(np.int64) // 32 + data[:, 1].astype(np.int64) // 32) % 2) == 0
    fit_mask = training & in_range
    if fit_mask.sum() < 100:
        raise ValueError("insufficient training points")
    results = []
    for direction, (a, b) in enumerate([(source, target), (target, source)]):
        for model in ["diagonal_affine", "full_affine", "white_anchored_diagonal"]:
            design = np.column_stack([a, np.ones(len(a))])
            if model == "full_affine":
                coefficients, _, rank, singular = np.linalg.lstsq(design[fit_mask], b[fit_mask], rcond=None)
                if rank != 4:
                    raise ValueError("rank-deficient full color fit")
                condition = float(singular[0] / singular[-1])
            elif model == "white_anchored_diagonal":
                coefficients = np.zeros((4, 3))
                for channel in range(3):
                    x = a[fit_mask, channel] - 1
                    y = b[fit_mask, channel] - 1
                    denominator = float(x @ x)
                    if denominator <= 1e-12:
                        raise ValueError("degenerate white-anchored fit")
                    gain = float(x @ y) / denominator
                    if not .2 <= gain <= 5:
                        raise ValueError("white-anchored gain outside diagnostic bounds")
                    coefficients[channel, channel] = gain
                    coefficients[3, channel] = 1 - gain
                condition = None
            else:
                coefficients = np.zeros((4, 3))
                for channel in range(3):
                    coef, _, rank, _ = np.linalg.lstsq(design[fit_mask][:, [channel, 3]], b[fit_mask, channel], rcond=None)
                    if rank != 2:
                        raise ValueError("rank-deficient channel fit")
                    coefficients[[channel, 3], channel] = coef
                condition = None
            predicted = np.clip(design @ coefficients, 0, 1)
            error = np.max(np.abs(predicted - np.clip(b, 0, 1)), axis=1)
            groups = {}
            for name, mask in {
                "training_in_range": fit_mask,
                "heldout_in_range": ~training & in_range,
                "heldout_out_of_range": ~training & ~in_range,
                "heldout_all": ~training,
                "heldout_native_negative": ~training & (source < 0).any(axis=1),
                "heldout_native_above_one_without_negative": ~training & (source > 1).any(axis=1) & ~(source < 0).any(axis=1),
                "heldout_target_near_white": ~training & (target >= .99).any(axis=1),
                "heldout_native_high_target_near_white": ~training & (source > 1).any(axis=1) & (target >= .99).any(axis=1),
            }.items():
                n = int(mask.sum())
                groups[name] = {"points": n, "agreement": float(np.mean(error[mask] <= .03)) if n else None}
            results.append({"direction": direction, "model": model, "coefficients": coefficients.tolist(), "design_condition": condition, "groups": groups})
    return {"input": str(path), "sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "points": len(data), "results": results}


if __name__ == "__main__":
    print(json.dumps({
        "scope": "Diagnostic unconstrained fits on sparse opaque aligned grid. Checkerboard holdout is within-image, not independent-camera qualification. All out-of-range holdout points retained; no acceptance/policy change.",
        "numpy_version": np.__version__,
        "pairs": [analyze(Path(p)) for p in sys.argv[1:]],
    }, indent=2, allow_nan=False))
