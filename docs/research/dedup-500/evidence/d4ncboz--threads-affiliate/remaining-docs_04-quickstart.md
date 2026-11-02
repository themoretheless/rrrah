# 04 — Quickstart

End-to-end guide to post your first affiliate thread.

## Prerequisites

Before starting, ensure:

- [x] Completed [01 - Account Setup](./01-account-setup.md)
- [x] Completed [02 - Database Setup](./02-database-setup.md)
- [x] Completed [03 - Cookie Extraction](./03-cookie-extraction.md)

## Installation

```bash
# Clone repo
git clone https://github.com/d4ncboz/threads-affiliate
cd threads-affiliate

# Install dependencies
pip install -r requirements.txt
playwright install chromium

# Verify installation
python -m threads_poster.cli --version
# → threads-poster v0.1.0
```

## Configuration

Create `config.json` in repo root:

```json
{
  "cookies_path": "~/.threads_poster/cookies/session.json",
  "database_path": "data/affiliate_links.md",
  "history_path": "data/post_history.json",
  "templates_dir": "templates/",
  "default_username": "@yourusername",
  "default_categories": ["skincare", "parfum", "haircare", "makeup"],
  "default_image_dir": "data/product_images/",
  "headless": true,
  "viewport": {"width": 1280, "height": 800}
}
```

## Your First Post

### Option 1: Single Post (CLI)

```bash
python -m threads_poster.cli post \
  --product "SKINTIFIC 5X Ceramide Moisturizer" \
  --link "https://s.shopee.co.id/XXXXX" \
  --category skincare \
  --hook-style edukasi \
  --image data/product_images/skintific.jpg
```

Expected output:
```
[01:00] Loading content templates...
[01:00] Selected hook: "Skincare hidden gem yang bakal lo sesalin..."
[01:00] Building 3-post chain...
[01:01] Loading session cookies...
[01:01] Launching Playwright headless...
[01:02] Navigating to threads.com/login
[01:03] Meta SSO login success: @yourusername
[01:03] Composing post 1/3...
[01:05] Composing post 2/3...
[01:07] Composing post 3/3 (with link)...
[01:09] Posted! Verified at /your_username/post/12345
[01:09] Marked link as USED in database
[01:09] Updated post_history.json
✅ Done
```

### Option 2: Auto-pick from Database

Let the tool pick the next UNUSED link automatically:

```bash
python -m threads_poster.cli post-auto \
  --category skincare \
  --hook-style edukasi
```

Tool will:
1. Read `data/affiliate_links.md`
2. Find next UNUSED in skincare category
3. Apply edukasi hook template
4. Post + mark as used

### Option 3: Programmatic

```python
from threads_poster import ThreadsPoster, AffiliateDatabase

# Load database
db = AffiliateDatabase("data/affiliate_links.md")
link, product = db.next_unused("skincare")

# Create poster
poster = ThreadsPoster(
    cookies_path="~/.threads_poster/cookies/session.json",
    headless=True
)

# Post
result = poster.post(
    product=product,
    affiliate_link=link,
    category="skincare",
    hook_style="edukasi",
    image_path="data/product_images/skintific.jpg"
)

if result.success:
    db.mark_used(link, note=f"{result.hook_category} hook")
    print(f"✅ Posted: {result.post_url}")
else:
    print(f"❌ Failed: {result.error}")
```

## Verifying the Post

After posting, verify:

1. Open your Threads profile in browser
2. Latest post should appear within 30 seconds
3. Click into the post — verify all 3 posts in thread are visible
4. Verify affiliate link is clickable (not broken)
5. Click your own affiliate link — should redirect to Shopee product

## Common First-Run Issues

### "Cookies expired"

```bash
python -m threads_poster.cli setup --extract-cookies
```

### "Database empty"

You haven't added links yet. Go to [02 - Database Setup](./02-database-setup.md) and add at least 5 links.

### "Image not found"

Either provide `--image` flag, or set up auto-image-pick in config. Without image, post will be text-only (lower engagement but valid).

### "Hook category already used in last 2 posts"

The dedup rejected your post. Choose different `--hook-style`:

```bash
# Available styles per category
edukasi | validasi_mental | storytelling | problem_solving | hook_pancingan | transformasi | social_proof | urgency | controversy
```

## Next Steps

Once your first post is live:

- Schedule recurring posts: [05 - Customization](./05-customization.md)
- Tune hooks for your niche: [05 - Customization](./05-customization.md)
- Set up cron: see `examples/cron_integration.sh`
- Monitor performance: track which hooks get most engagement, double down on those
