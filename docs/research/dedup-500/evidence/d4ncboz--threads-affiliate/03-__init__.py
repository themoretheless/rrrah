"""Threads Affiliate Poster — automated Indonesian affiliate posting toolkit."""
__version__ = "0.1.0"

from .poster import ThreadsPoster, PostResult
from .content_generator import ContentGenerator
from .database import AffiliateDatabase
from .dedup import DedupChecker

__all__ = [
    "ThreadsPoster",
    "PostResult",
    "ContentGenerator",
    "AffiliateDatabase",
    "DedupChecker",
]
