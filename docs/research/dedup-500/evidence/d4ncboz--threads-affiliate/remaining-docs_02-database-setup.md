# 02 — Affiliate Database Setup

The poster reads from a Markdown-based affiliate link database. This format is human-editable, version-control friendly, and easy to track status.

## Database Location

```
data/affiliate_links.md
```

> **NOT committed to git** (in `.gitignore`). Each user maintains their own database.

## Database Structure

```markdown
# My Shopee Affiliate Link Database

## 📊 Stats
- **Total Links:** 100
- **Used:** 12/100
- **Available:** 88
- **Last Updated:** 2026-06-29

---

## 🧴 SKINCARE (40 produk)

| # | Product | Link | Status | Last Used |
|---|---------|------|--------|-----------|
| 1 | SKINTIFIC 5X Ceramide Barrier Moisturizer | `https://s.shopee.co.id/XXXXX` | ❌ UNUSED | - |
| 2 | Azarine Hydrasoothe Sunscreen SPF45 | `https://s.shopee.co.id/YYYYY` | ✅ USED (2026-06-28) — edukasi hook | 2026-06-28 |
| 3 | Somethinc AHA BHA PHA Peeling Serum | `https://s.shopee.co.id/ZZZZZ` | ❌ UNUSED | - |

---

## 🌸 PARFUM (25 produk)

| # | Product | Link | Status | Last Used |
|---|---------|------|--------|-----------|
| 1 | Fabric Mist Parfum Tahan Lama | `https://s.shopee.co.id/AAAAA` | ❌ UNUSED | - |
| 2 | HMNS Eau de Parfum Iconic | `https://s.shopee.co.id/BBBBB` | ❌ UNUSED | - |

---

## 💆 HAIRCARE (20 produk)

| # | Product | Link | Status | Last Used |
|---|---------|------|--------|-----------|
| 1 | NR Hair Serum Anti-rontok | `https://s.shopee.co.id/CCCCC` | ❌ UNUSED | - |

---

## 💄 MAKEUP (15 produk)

| # | Product | Link | Status | Last Used |
|---|---------|------|--------|-----------|
| 1 | Wardah Lip Cream Matte Long Lasting | `https://s.shopee.co.id/DDDDD` | ❌ UNUSED | - |
```

---

## Rules for Database Entries

| Field | Rule |
|-------|------|
| `#` | Sequential integer per category |
| `Product` | Full product name as it appears on Shopee (max 60 chars) |
| `Link` | Wrapped in backticks. MUST be `s.shopee.co.id/XXXXX` short link |
| `Status` | Use `❌ UNUSED` or `✅ USED (date) — note` |
| `Last Used` | ISO date `YYYY-MM-DD` or `-` if unused |

---

## Building Your Database

### Strategy 1: Bulk Import via Shopee Affiliate Dashboard

1. Login to [affiliate.shopee.co.id](https://affiliate.shopee.co.id)
2. Use "Top Performing Products" filter
3. For each product → click "Bagikan" → "Buat Tautan"
4. Copy short link
5. Paste into your database table

### Strategy 2: Manual Niche Research

1. Browse Shopee for products in your niche
2. Filter by: rating 4.5+, sold 1000+, price <Rp 200rb
3. Generate affiliate link for each
4. Add to database categorized

### Strategy 3: Trending Products

Use Shopee's **"Lagi Tren"** section in affiliate dashboard — products with high commission rate this month.

---

## Database Size Recommendation

| Posts/Day | Recommended DB Size | Why |
|-----------|---------------------|-----|
| 1/day | 30-50 links | 1 month runway before reuse |
| 2/day | 60-100 links | 1 month runway |
| 3/day | 90-150 links | 1 month runway |
| 5/day | 150+ links | 1 month runway, requires careful warm-up |

**Why 1-month rotation?** Threads algorithm flags accounts that promote same product multiple times. 30-day spacing between same product = safer.

---

## Programmatic Access

```python
from threads_poster.database import AffiliateDatabase

db = AffiliateDatabase("data/affiliate_links.md")

# Get next unused link in a category
link, product = db.next_unused("skincare")
print(f"Product: {product}")
print(f"Link: {link}")

# Mark as used after posting
db.mark_used(link, note="edukasi hook, posted at 08:00")

# Stats
print(db.stats())  
# → {"total": 100, "used": 13, "available": 87, "by_category": {...}}

# Reset all (when starting new month/batch)
db.reset_all_used()  # only call after confirming
```

---

## ✅ Checklist Before Running Automation

- [ ] Database file exists at `data/affiliate_links.md`
- [ ] At least **30 UNUSED links** across categories
- [ ] All links tested manually (open in browser, redirects to product page)
- [ ] Categories balanced (don't have 90 skincare + 1 parfum)
- [ ] Backup of database (commit to private repo or save to cloud)

Next: [03 — Cookie Extraction](./03-cookie-extraction.md)
