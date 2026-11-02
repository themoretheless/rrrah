"""Bucket-feasible MinHash/LSH deduplication for text corpora."""

from .minhash import (
    BucketStage,
    FilterStage,
    MinhashConfig,
    PruneStage,
    SelectionStage,
    SignatureStage,
)
from .signatures import HashConfig

__version__ = "0.1.0"
