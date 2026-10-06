# DINOHash ONNX

Python implementation of DINOHash using ONNX Runtime, from https://www.arxiv.org/abs/2503.11195

This is a Python port of the Node.js implementation, providing perceptual image hashing using ONNX models.

## Installation

```bash
uv add dinohash-onnx
```

## Usage

```python
from dinohash_onnx import DINOHash
import os

hash = DINOHash()
hash_hex = hash.hash(image_path)
print(f"Hex: {hash_hex}")
```

## Example

See `examples/example.py` for a complete working example.

## License

MIT

## Citation

If you use this library in your research, please cite:

```bibtex
@article{dinohash2025,
  title={DINOHash: Perceptual Image Hashing with Vision Transformers},
  journal={arXiv preprint arXiv:2503.11195},
  year={2025}
}
```
