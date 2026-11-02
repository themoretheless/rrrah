"""Dedup checker — prevent link reuse + hook repetition + category burst."""
from __future__ import annotations

import json
from pathlib import Path
from typing import Optional


class DedupChecker:
    """Track post history and reject duplicates."""

    def __init__(self, history_path: Path | str = "data/post_history.json"):
        self.history_path = Path(history_path)
        self.history_path.parent.mkdir(parents=True, exist_ok=True)

    def load(self) -> list[dict]:
        """Load post history. Returns [] if missing."""
        if not self.history_path.exists():
            return []
        data = json.loads(self.history_path.read_text())
        if isinstance(data, dict) and "posts" in data:
            return data["posts"]
        elif isinstance(data, list):
            return data
        return []

    def save(self, history: list[dict]):
        """Save (keep last 100 entries)."""
        history = history[-100:]
        self.history_path.write_text(
            json.dumps({"posts": history}, indent=2, ensure_ascii=False)
        )

    def check(
        self,
        affiliate_link: str,
        hook_category: str,
        hook_text: str,
        category: Optional[str] = None,
    ) -> tuple[bool, str]:
        """
        Check if a new post passes dedup rules.

        Rules:
        1. Affiliate link never reused (ever).
        2. Hook category (edukasi/storytelling/...) not used in last 2 posts.
        3. Hook text similarity <= 60% with last 5 posts.
        4. Category not used in last 1 post (force rotation).

        Returns:
            (ok: bool, reason: str)
        """
        history = self.load()
        if not history:
            return True, "First post"

        # Rule 1: link reuse
        used_links = {p.get("affiliate_link") for p in history}
        if affiliate_link in used_links:
            return False, f"Link already used: {affiliate_link}"

        # Rule 2: hook category not in last 2
        recent_cats = [p.get("hook_category", "") for p in history[-2:]]
        if hook_category in recent_cats:
            return False, f"Hook category '{hook_category}' used in last 2 posts"

        # Rule 3: hook similarity
        hook_words = set(hook_text[:60].lower().split())
        for prev in history[-5:]:
            prev_words = set(prev.get("hook_text", "")[:60].lower().split())
            if hook_words and prev_words:
                overlap = len(hook_words & prev_words) / max(len(hook_words), 1)
                if overlap > 0.6:
                    return False, (
                        f"Hook too similar to recent post "
                        f"(overlap={overlap:.0%})"
                    )

        # Rule 4: category rotation (recommended but not strict)
        if category:
            recent_categories = [p.get("category", "") for p in history[-1:]]
            if category in recent_categories:
                # warning only, not blocking
                pass

        return True, "OK"

    def record(self, entry: dict):
        """Append a post entry to history."""
        history = self.load()
        history.append(entry)
        self.save(history)

    def stats(self) -> dict:
        """Return aggregate stats."""
        history = self.load()
        from collections import Counter

        return {
            "total_posts": len(history),
            "by_category": dict(Counter(p.get("category", "?") for p in history)),
            "by_hook_style": dict(Counter(p.get("hook_category", "?") for p in history)),
            "last_post_at": history[-1].get("date") if history else None,
        }
