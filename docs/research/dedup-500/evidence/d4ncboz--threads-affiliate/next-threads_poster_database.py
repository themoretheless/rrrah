"""Affiliate link database — Markdown-based, human-editable."""
from __future__ import annotations

import re
from datetime import datetime
from pathlib import Path
from typing import Optional


class AffiliateDatabase:
    """Read/write affiliate link database from Markdown file."""

    def __init__(self, path: Path | str = "data/affiliate_links.md"):
        self.path = Path(path)
        self.path.parent.mkdir(parents=True, exist_ok=True)

    def _read(self) -> str:
        if not self.path.exists():
            self._init_empty()
        return self.path.read_text()

    def _init_empty(self):
        """Create empty database scaffold."""
        scaffold = """# My Shopee Affiliate Link Database

## 📊 Stats
- **Total Links:** 0
- **Used:** 0
- **Available:** 0
- **Last Updated:** {today}

---

## 🧴 SKINCARE

| # | Product | Link | Status | Last Used |
|---|---------|------|--------|-----------|

---

## 🌸 PARFUM

| # | Product | Link | Status | Last Used |
|---|---------|------|--------|-----------|

---

## 💆 HAIRCARE

| # | Product | Link | Status | Last Used |
|---|---------|------|--------|-----------|

---

## 💄 MAKEUP

| # | Product | Link | Status | Last Used |
|---|---------|------|--------|-----------|
""".format(today=datetime.now().strftime("%Y-%m-%d"))
        self.path.write_text(scaffold)

    def _parse_rows(self, text: str) -> list[dict]:
        """Parse all table rows from Markdown DB.

        Returns:
            [{"category": "skincare", "num": 1, "product": "...",
              "link": "...", "status": "UNUSED", "last_used": "-"}, ...]
        """
        rows = []
        current_cat = None
        cat_pattern = re.compile(r"^##\s+(?:[^\w]*\s+)?([A-Z]+)\s*", re.MULTILINE)
        row_pattern = re.compile(
            r"^\|\|?\s*(\d+)\s*\|\s*([^|]+?)\s*\|\s*`([^`]+)`\s*\|\s*([^|]+?)\s*\|\s*([^|]+?)\s*\|"
        )

        for line in text.split("\n"):
            cat_match = cat_pattern.match(line.strip())
            if cat_match:
                current_cat = cat_match.group(1).lower()
                continue

            row_match = row_pattern.match(line)
            if row_match and current_cat:
                num, product, link, status, last_used = row_match.groups()
                rows.append({
                    "category": current_cat,
                    "num": int(num),
                    "product": product.strip(),
                    "link": link.strip(),
                    "status": status.strip(),
                    "last_used": last_used.strip(),
                })
        return rows

    def next_unused(self, category: Optional[str] = None) -> tuple[str, str, str]:
        """
        Get the next unused link.

        Args:
            category: filter by category, or None for any.

        Returns:
            (link, product_name, category)

        Raises:
            ValueError if no unused links available.
        """
        rows = self._parse_rows(self._read())
        unused = [r for r in rows if "UNUSED" in r["status"].upper()]
        if category:
            unused = [r for r in unused if r["category"] == category.lower()]

        if not unused:
            raise ValueError(
                f"No UNUSED links available"
                + (f" in category '{category}'" if category else "")
            )

        # Pick FIRST unused (oldest in list)
        pick = unused[0]
        return pick["link"], pick["product"], pick["category"]

    def mark_used(self, link: str, note: str = ""):
        """Mark a link as USED with today's date and optional note."""
        text = self._read()
        today = datetime.now().strftime("%Y-%m-%d")
        status_new = f"✅ USED ({today})"
        if note:
            status_new += f" — {note}"

        # Find row with this link, replace status + last_used
        # Pattern: capture before status, status itself, last_used
        pattern = re.compile(
            r"(\|\|?\s*\d+\s*\|\s*[^|]+?\s*\|\s*`" + re.escape(link) + r"`\s*\|\s*)([^|]+?)(\s*\|\s*)([^|]+?)(\s*\|)"
        )

        def repl(m):
            return f"{m.group(1)}{status_new}{m.group(3)}{today}{m.group(5)}"

        new_text, count = pattern.subn(repl, text)
        if count == 0:
            raise ValueError(f"Link not found in database: {link}")

        self.path.write_text(new_text)

    def reset_all(self):
        """Mark all links as UNUSED again (for new batch/cycle)."""
        text = self._read()
        # Reset status column
        text = re.sub(
            r"(\|\|?\s*\d+\s*\|\s*[^|]+?\s*\|\s*`[^`]+`\s*\|\s*)([^|]+?)(\s*\|\s*)([^|]+?)(\s*\|)",
            r"\1❌ UNUSED\3-\5",
            text,
        )
        self.path.write_text(text)

    def add_link(
        self,
        category: str,
        product: str,
        link: str,
    ):
        """Add a new link to the database under specified category."""
        text = self._read()
        category_upper = category.upper()

        # Find the category section
        cat_pattern = re.compile(
            rf"(##\s+(?:[^\w]*\s+)?{category_upper}[^\n]*\n+\|[^\n]+\n\|[^\n]+\n)",
            re.IGNORECASE,
        )
        match = cat_pattern.search(text)
        if not match:
            raise ValueError(f"Category '{category}' not found in database")

        # Count existing rows under this category
        rows = self._parse_rows(text)
        cat_rows = [r for r in rows if r["category"] == category.lower()]
        next_num = max([r["num"] for r in cat_rows], default=0) + 1

        new_row = f"| {next_num} | {product} | `{link}` | ❌ UNUSED | - |\n"

        # Insert after header rows
        insert_pos = match.end()
        # Find the end of existing rows
        rest = text[insert_pos:]
        # Skip existing rows
        existing = re.match(r"(\|[^\n]+\n)*", rest)
        if existing:
            insert_pos += existing.end()

        new_text = text[:insert_pos] + new_row + text[insert_pos:]
        self.path.write_text(new_text)

    def stats(self) -> dict:
        """Aggregate statistics."""
        rows = self._parse_rows(self._read())
        from collections import Counter

        total = len(rows)
        used = sum(1 for r in rows if "USED" in r["status"].upper() and "UNUSED" not in r["status"].upper())
        unused = total - used

        return {
            "total": total,
            "used": used,
            "available": unused,
            "by_category": dict(Counter(r["category"] for r in rows)),
            "available_by_category": dict(
                Counter(r["category"] for r in rows if "UNUSED" in r["status"].upper())
            ),
        }
