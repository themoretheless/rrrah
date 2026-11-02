"""Tests for content generator + dedup logic (no Playwright needed)."""
import json
import tempfile
from pathlib import Path

import pytest

from threads_poster import ContentGenerator, DedupChecker, AffiliateDatabase


@pytest.fixture
def templates_dir(tmp_path):
    """Create minimal test templates."""
    (tmp_path / "hooks_skincare.json").write_text(json.dumps({
        "_meta": {"category": "skincare"},
        "hooks": {
            "edukasi": ["Test edukasi {product}"],
            "storytelling": ["Test story {product}"],
        }
    }))
    (tmp_path / "post2_templates.json").write_text(json.dumps({
        "skincare": ["Test review {product}"],
        "default": ["Default review"],
    }))
    (tmp_path / "post3_cta_templates.json").write_text(json.dumps({
        "templates": ["Cek {product} disini:\n"],
    }))
    return tmp_path


def test_generate_hook(templates_dir):
    gen = ContentGenerator(templates_dir)
    hook, style = gen.generate_hook("Product X", "skincare", "edukasi")
    assert "Product X" in hook
    assert style == "edukasi"


def test_generate_chain_3_posts(templates_dir):
    gen = ContentGenerator(templates_dir)
    chain = gen.generate_chain(
        product="Product Y",
        affiliate_link="https://s.shopee.co.id/ABC",
        category="skincare",
        hook_style="edukasi",
        num_posts=3,
    )
    assert "post_1" in chain
    assert "post_2" in chain
    assert "post_3" in chain
    assert "Product Y" in chain["post_1"]
    assert chain["hook_category"] == "edukasi"


def test_dedup_link_reuse(tmp_path):
    history_path = tmp_path / "history.json"
    dedup = DedupChecker(history_path)

    # First post: OK
    ok, _ = dedup.check("https://s.shopee.co.id/X", "edukasi", "Some hook", "skincare")
    assert ok

    # Record it
    dedup.record({
        "date": "2026-06-29T08:00",
        "hook_category": "edukasi",
        "hook_text": "Some hook",
        "affiliate_link": "https://s.shopee.co.id/X",
    })

    # Reuse same link: REJECTED
    ok, reason = dedup.check("https://s.shopee.co.id/X", "validasi", "Different hook", "parfum")
    assert not ok
    assert "already used" in reason.lower()


def test_dedup_category_burst(tmp_path):
    history_path = tmp_path / "history.json"
    dedup = DedupChecker(history_path)

    # Use 'edukasi' twice
    for i in range(2):
        dedup.record({
            "date": f"2026-06-29T0{i+8}:00",
            "hook_category": "edukasi",
            "hook_text": f"Hook number {i}",
            "affiliate_link": f"https://s.shopee.co.id/A{i}",
        })

    # Third try with edukasi: rejected
    ok, reason = dedup.check(
        "https://s.shopee.co.id/NEW",
        "edukasi",
        "Yet another hook",
    )
    assert not ok
    assert "last 2 posts" in reason


def test_database_parse_unused(tmp_path):
    db_path = tmp_path / "links.md"
    db_path.write_text("""# DB
## SKINCARE

| # | Product | Link | Status | Last Used |
|---|---------|------|--------|-----------|
| 1 | Product A | `https://s.shopee.co.id/A` | ❌ UNUSED | - |
| 2 | Product B | `https://s.shopee.co.id/B` | ✅ USED (2026-06-28) | 2026-06-28 |
| 3 | Product C | `https://s.shopee.co.id/C` | ❌ UNUSED | - |
""")
    db = AffiliateDatabase(db_path)
    link, product, cat = db.next_unused("skincare")
    assert link == "https://s.shopee.co.id/A"
    assert product == "Product A"
    assert cat == "skincare"


def test_database_mark_used(tmp_path):
    db_path = tmp_path / "links.md"
    db_path.write_text("""# DB
## SKINCARE

| # | Product | Link | Status | Last Used |
|---|---------|------|--------|-----------|
| 1 | Product A | `https://s.shopee.co.id/A` | ❌ UNUSED | - |
""")
    db = AffiliateDatabase(db_path)
    db.mark_used("https://s.shopee.co.id/A", note="edukasi hook")

    text = db_path.read_text()
    assert "USED" in text
    assert "UNUSED" not in text
    assert "edukasi hook" in text
