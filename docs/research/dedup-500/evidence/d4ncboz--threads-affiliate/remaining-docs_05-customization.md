# 05 — Customization

## Custom Hook Templates

Add new hook styles or modify existing ones in `templates/hooks_{category}.json`.

### Template Structure

```json
{
  "_meta": {
    "category": "skincare",
    "language": "id-ID",
    "version": "1.0"
  },
  "hooks": {
    "my_custom_style": [
      "Variant 1 dengan {product}",
      "Variant 2 dengan {product}",
      "Variant 3 dengan {product}"
    ]
  }
}
```

### Placeholders

Currently supported:
- `{product}` — replaced with product name from database

To add custom placeholders, edit `threads_poster/content_generator.py` → `generate_hook()`.

### Best Practices for Hook Writing

1. **Length**: Keep hooks under 80 characters for first line visibility on mobile
2. **Emoji**: 1-2 emojis max per hook
3. **Hook formula**: Pattern Interrupt + Curiosity + Specificity
   - ❌ "Produk ini bagus" (vague)
   - ✅ "Sumpah gw beli ulang udah 3 kali. {product}" (specific + social proof)
4. **Avoid spam words**: "Promo", "Beli sekarang", "Diskon gila" (Threads filter)

---

## Custom Voice / Tone

To adjust voice (e.g. female vs male, casual vs formal):

1. Edit hook templates with new tone variants
2. Update `_meta.tone` field
3. Add tone-specific files: `hooks_skincare_male.json`, `hooks_skincare_formal.json`
4. Modify `ContentGenerator._load_hooks()` to support tone selection

---

## Custom Schedule

Edit `examples/cron_integration.sh` to change times:

```bash
# Custom schedule example: 5x/day
0 0 * * * → 07:00 WIB skincare
0 5 * * * → 12:00 WIB parfum  
0 9 * * * → 16:00 WIB haircare
0 13 * * * → 20:00 WIB makeup
0 17 * * * → 24:00 WIB skincare (rotate)
```

**Don't post more than 5/day** — Threads spam detection.

---

## Custom Categories

Default: skincare, parfum, haircare, makeup.

To add new category (e.g. "fashion"):

1. Create `templates/hooks_fashion.json` (use existing files as template)
2. Add fashion entry to `templates/post2_templates.json`:
   ```json
   "fashion": [
     "Outfit yang gw stuck pakai 3 bulan: {product}...",
     ...
   ]
   ```
3. Update `threads_poster/content_generator.py`:
   ```python
   CATEGORIES = ["skincare", "parfum", "haircare", "makeup", "fashion"]
   ```
4. Update `threads_poster/cli.py` choices list

---

## Headless vs Headed

For debugging (see what's happening):

```bash
python -m threads_poster.cli post --headed ...
```

This opens visible Chrome window. Useful when:
- Cookies expired and you need to re-login mid-session
- Editor selector changed (Threads UI updates)
- Image upload failing

In production: always use headless (default).

---

## Image Selection Strategy

Two strategies for choosing product images:

### Strategy 1: Pre-curated folder

Store real product images in `data/product_images/` named by SKU or link suffix:

```
data/product_images/
├── 4LGSge9GGN.jpg     # SKINTIFIC, last 8 chars of affiliate link
├── 9ALogn6aec.jpg     # Azarine
└── ...
```

Auto-resolution:
```python
from pathlib import Path
img_dir = Path("data/product_images")
sku = affiliate_link.split("/")[-1]  # e.g. "4LGSge9GGN"
img_path = img_dir / f"{sku}.jpg"
if img_path.exists():
    poster.post(..., image_path=str(img_path))
```

### Strategy 2: Real-time scraping

Scrape product images from Shopee at post time. Requires:
- Shopee account cookies
- Browser automation (Playwright/Selenium)
- Anti-WAF measures (Shopee uses Akamai)

See `docs/06-troubleshooting.md#shopee-scraping` for guidance.

---

## Logging

Default: stdout + stderr (capture via cron `>> logs/cron.log 2>&1`).

For structured logging, edit `threads_poster/poster.py`:

```python
import logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    filename="logs/poster.log",
)
```

---

## Multi-Account Setup

To run multiple accounts (e.g. niche-dedicated accounts):

1. Extract cookies separately per account:
   ```bash
   python -m threads_poster.cli setup --extract-cookies \
     --chrome-profile "Profile 1" \
     --output ~/.threads_poster/cookies/account_skincare.json

   python -m threads_poster.cli setup --extract-cookies \
     --chrome-profile "Profile 2" \
     --output ~/.threads_poster/cookies/account_fashion.json
   ```

2. Maintain separate databases per niche:
   ```
   data/affiliate_links_skincare.md
   data/affiliate_links_fashion.md
   ```

3. Schedule separately:
   ```bash
   # crontab
   0 1 * * * python -m threads_poster.cli post-auto \
     --cookies ~/.threads_poster/cookies/account_skincare.json \
     --db data/affiliate_links_skincare.md \
     --history data/history_skincare.json \
     --category skincare
   ```
