"""Project auto-detection — fingerprints a directory to pick the right profile."""

from .fingerprint import (
    ProjectFingerprint,
    detect_project,
    suggest_profile_for_files,
    summarize_fingerprint,
)

__all__ = [
    "ProjectFingerprint",
    "detect_project",
    "suggest_profile_for_files",
    "summarize_fingerprint",
]
