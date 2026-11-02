"""Content generation: hook templates + multi-post chain builder."""
from __future__ import annotations

import json
import random
from pathlib import Path
from typing import Optional


CATEGORIES = ["skincare", "parfum", "haircare", "makeup"]

# Map keywords to a category for auto-classification
HOOK_STYLE_HEURISTICS = {
    "edukasi": ["educational", "fact", "tip"],
    "validasi_mental": ["empathy", "support"],
    "storytelling": ["story", "personal"],
    "problem_solving": ["solution", "fix"],
    "hook_pancingan": ["hook", "trigger"],
    "transformasi": ["before", "after"],
    "social_proof": ["popular", "trending"],
    "urgency": ["limited", "now"],
    "controversy": ["controversial", "hot-take"],
}


class ContentGenerator:
    """Generate 2-3 post chains from templates with rotation."""

    def __init__(self, templates_dir: Path | str = "templates"):
        self.templates_dir = Path(templates_dir)
        self._hooks_cache: dict[str, dict] = {}
        self._post2_cache: Optional[dict] = None
        self._cta_cache: Optional[dict] = None

    def _load_hooks(self, category: str) -> dict:
        if category in self._hooks_cache:
            return self._hooks_cache[category]
        path = self.templates_dir / f"hooks_{category}.json"
        if not path.exists():
            raise FileNotFoundError(f"Hook template missing: {path}")
        data = json.loads(path.read_text())
        self._hooks_cache[category] = data
        return data

    def _load_post2(self) -> dict:
        if self._post2_cache is None:
            path = self.templates_dir / "post2_templates.json"
            self._post2_cache = json.loads(path.read_text())
        return self._post2_cache

    def _load_cta(self) -> dict:
        if self._cta_cache is None:
            path = self.templates_dir / "post3_cta_templates.json"
            self._cta_cache = json.loads(path.read_text())
        return self._cta_cache

    def available_hook_styles(self, category: str) -> list[str]:
        """Return list of hook styles available for a category."""
        hooks = self._load_hooks(category)
        return list(hooks.get("hooks", {}).keys())

    def generate_hook(
        self,
        product: str,
        category: str,
        hook_style: Optional[str] = None,
    ) -> tuple[str, str]:
        """Generate single hook text + return (text, style_used)."""
        hooks_data = self._load_hooks(category)
        hooks = hooks_data.get("hooks", {})

        if not hooks:
            raise ValueError(f"No hooks defined for category: {category}")

        if hook_style is None:
            hook_style = random.choice(list(hooks.keys()))

        if hook_style not in hooks:
            raise ValueError(
                f"Hook style '{hook_style}' not available for {category}. "
                f"Available: {list(hooks.keys())}"
            )

        template = random.choice(hooks[hook_style])
        return template.format(product=product), hook_style

    def generate_post2(self, product: str, category: str) -> str:
        """Generate middle post (review body)."""
        post2 = self._load_post2()
        templates = post2.get(category, post2.get("default", []))
        if not templates:
            return f"Yang gw suka dari {product}: kualitas konsisten, harga masuk akal."
        return random.choice(templates).format(product=product)

    def generate_cta(self, product: str, affiliate_link: str) -> str:
        """Generate closing CTA — does NOT include link (link added by poster)."""
        cta = self._load_cta()
        templates = cta.get("templates", [])
        if not templates:
            return f"Cek {product} di link berikut:\n"
        return random.choice(templates).format(product=product)

    def generate_chain(
        self,
        product: str,
        affiliate_link: str,
        category: str,
        hook_style: Optional[str] = None,
        num_posts: int = 3,
    ) -> dict:
        """
        Generate complete N-post chain.

        Returns:
            {
                "post_1": "...",
                "post_2": "...",
                "post_3": "...",
                "affiliate_link": "https://s.shopee.co.id/...",
                "product_name": "...",
                "hook_category": "edukasi",
                "hook_text": "...",
                "category": "skincare",
            }
        """
        if num_posts not in (2, 3):
            raise ValueError("num_posts must be 2 or 3 (Threads max chain limit)")

        if category not in CATEGORIES:
            raise ValueError(f"Category must be one of {CATEGORIES}")

        hook_text, hook_style_used = self.generate_hook(product, category, hook_style)

        content = {
            "post_1": hook_text,
            "affiliate_link": affiliate_link,
            "product_name": product,
            "hook_category": hook_style_used,
            "hook_text": hook_text,
            "category": category,
        }

        if num_posts == 3:
            content["post_2"] = self.generate_post2(product, category)
            content["post_3"] = self.generate_cta(product, affiliate_link)
        else:
            content["post_2"] = self.generate_cta(product, affiliate_link)

        return content
