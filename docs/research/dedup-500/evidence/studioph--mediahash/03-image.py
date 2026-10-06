import hashlib
import logging
import os
from dataclasses import dataclass
from pathlib import Path
from typing import IO

import imagehash
import numpy as np

try:
    import pillow_avif  # pylint: disable=unused-import
except ImportError:
    pass
import context_utils
from imagehash import ImageHash
from PIL import Image, UnidentifiedImageError

from .types import MediaError, MediaFingerprint

LOG = logging.getLogger(__name__)

extensions = (".jpg", ".jpeg", ".png", ".webp", ".avif")

rethrow = context_utils.rethrow(UnidentifiedImageError, as_=MediaError)


@dataclass(frozen=True, kw_only=True)
class ImageInfo:
    width: int
    height: int
    size: int
    format: str


class ImageFingerprint:
    def __init__(self, ihash: ImageHash):
        self._fp = ihash

    def __sub__(self, other: "ImageFingerprint") -> int:
        return self._fp - other._fp

    def __bytes__(self) -> bytes:
        return self._fp.hash.tobytes()


@staticmethod
@rethrow
def analyze(media: IO) -> ImageInfo:
    LOG.debug("Analyzing %s", media.name)
    with Image.open(media) as img:
        media.seek(0, os.SEEK_END)
        return ImageInfo(
            size=media.tell(), width=img.width, height=img.height, format=img.format
        )


@staticmethod
@rethrow
def checksum(media: IO) -> bytes:
    LOG.debug("Calculating hash for %s", media.name)
    with Image.open(media) as img:
        hash_ = hashlib.md5(img.tobytes())
    return hash_.digest()


@staticmethod
@rethrow
def fingerprint(media: IO) -> MediaFingerprint:
    LOG.debug("Fingerprinting %s", media.name)
    with Image.open(media) as img:
        fingerprint_ = imagehash.phash(img)
    return ImageFingerprint(fingerprint_)


@staticmethod
def parse_fingerprint(__bytes: bytes) -> MediaFingerprint:
    arr = np.frombuffer(__bytes, dtype=np.uint8)
    return ImageFingerprint(ImageHash(arr))


@dataclass(frozen=True, kw_only=True)
class Image:
    name: str
    path: Path
    info: ImageInfo
    checksum: bytes
    fingerprint: ImageFingerprint