"""Hash configuration and binary helpers for the existing signature format.

The hash mapping and binary search are retained from the extracted implementation;
see NOTICE for provenance. Character shingling remains in the MinHash stage.
"""

import hashlib
import os
import struct
from dataclasses import dataclass
from functools import cache
from typing import Callable, Literal

import numpy as np
import xxhash
from fsspec.spec import AbstractBufferedFile


@dataclass
class TextNormConfig:
    """Legacy configuration fields; character shingling does not consult them."""

    lowercase: bool = True
    norm_whitespace: bool = True
    remove_punctuation: bool = True
    norm_unicode_diacritics: bool = True
    norm_numbers: bool = True
    norm_weekdays: bool = False
    norm_monthnames: bool = False


@dataclass(frozen=True)
class HashConfig:
    precision: Literal[32, 64] = 64
    hash_fc: Literal["sha1", "xxhash"] = "xxhash"

    @property
    def np_dtype(self):
        return np.uint32 if self.precision == 32 else np.uint64

    @property
    def np_descr(self):
        return np.dtype(self.np_dtype).descr[0][1]

    @property
    def struct_format(self):
        return "I" if self.precision == 32 else "Q"

    @property
    def max(self):
        return np.iinfo(self.np_dtype).max

    @property
    def min(self):
        return np.iinfo(self.np_dtype).min

    def __str__(self):
        return f"HashConfig(precision={self.precision}, hash_fc={self.hash_fc})"


def sha1_hash32(data: str):
    return struct.unpack("<I", hashlib.sha1(data.encode("utf-8")).digest()[:4])[0]


def sha1_hash64(data: str):
    return struct.unpack("<Q", hashlib.sha1(data.encode("utf-8")).digest()[:8])[0]


def create_hash_func(config: HashConfig) -> Callable[[str], int]:
    if config.hash_fc == "sha1":
        return sha1_hash32 if config.precision == 32 else sha1_hash64
    if config.hash_fc == "xxhash":
        hash_bytes = xxhash.xxh32_intdigest if config.precision == 32 else xxhash.xxh64_intdigest

        def hash_text(text: str) -> int:
            return hash_bytes(text.encode("utf-8"))

        return hash_text
    raise ValueError(f"Unknown {config.hash_fc=}")


def read_tuples_from_file(file, *formats, lines_to_buffer: int = 5):
    if lines_to_buffer != -1 and lines_to_buffer < 1:
        raise ValueError("lines_to_buffer must be >= 1 or -1 (for unlimited)")
    reader = struct.Struct("<" + "".join(formats))
    while True:
        chunk = file.read(lines_to_buffer * reader.size if lines_to_buffer != -1 else -1)
        if not chunk:
            break
        yield from reader.iter_unpack(chunk)


def seek_to_start(f: AbstractBufferedFile, start_hash: int, line_format: str, hash_format: str):
    if start_hash == 0:
        return
    line_size = struct.calcsize(line_format)
    nr_lines = f.size // line_size

    @cache
    def read_line_start(line):
        assert 0 <= line < nr_lines
        f.seek(line * line_size, os.SEEK_SET)
        return struct.unpack(hash_format, f.read(struct.calcsize(hash_format)))[0]

    if read_line_start(0) >= start_hash:
        f.seek(0, os.SEEK_SET)
        return
    if read_line_start(nr_lines - 1) < start_hash:
        f.seek(0, os.SEEK_END)
        return

    start_line, hi = 0, nr_lines
    while start_line < hi:
        mid = (start_line + hi) // 2
        if read_line_start(mid) < start_hash:
            start_line = mid + 1
        else:
            hi = mid
    if start_line > nr_lines:
        raise ValueError
    if (prev_hash := read_line_start(start_line - 1)) >= start_hash:
        raise ValueError(f"Wrong bsearch start line: {prev_hash=} >= {start_hash=}")
    f.seek(start_line * line_size, os.SEEK_SET)
