"""
DINOHash ONNX - Perceptual image hashing using ONNX Runtime.

Implementation of DINOHash from https://www.arxiv.org/abs/2503.11195
"""

from .hash import DINOHash

__version__ = "0.1.0"
__all__ = ["DINOHash"]
