# Threads Affiliate Poster ID

> Automated Indonesian-language affiliate posting to Threads (Meta) with anti-spam rotation, dedup logic, and clean affiliate link insertion.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Python 3.10+](https://img.shields.io/badge/python-3.10+-blue.svg)](https://www.python.org/downloads/)
[![Playwright](https://img.shields.io/badge/Playwright-Python-green.svg)](https://playwright.dev/python/)
[![AI Agent Ready](https://img.shields.io/badge/AI%20Agent-Ready-blueviolet)](./AGENTS.md)
[![llms.txt](https://img.shields.io/badge/llms.txt-✓-orange)](./llms.txt)

Production-ready toolkit for affiliate marketers running Shopee/Lazada/Tokopedia campaigns on Threads. Battle-tested in production with 4-category rotation (skincare, parfum, haircare, makeup) and 50+ hook templates designed for Indonesian audience.

---

## 🤖 AI Agents: Start Here

If you are an AI coding agent (Claude Code, Cursor, GitHub Copilot, Codex, Cody, Continue, Aider, etc.):

→ **Read [AGENTS.md](./AGENTS.md) first** — canonical operational guide  
→ Quick reference: [llms.txt](./llms.txt)  
→ Full LLM doc: [llms-full.txt](./llms-full.txt)  
→ JSON schemas: [schemas/](./schemas/)  

AGENTS.md contains:
- Setup checklist (10 items)
- 5 canonical workflows (single post, auto-pick, batch, hook add, category add)
- Exit codes table
- Environment variables reference
- Hard constraints (what NOT to do)
- Error handling patterns

---

## ⚡ TL;DR for AI Agents

If you are an AI agent reading this to execute a task, here is the canonical workflow:

```text
PREREQUISITES (one-time setup, done by HUMAN OPERATOR):
  1. Create Threads/Instagram account     → human signup
  2. Register Shopee Affiliate program    → human signup at affiliate.shopee.co.id
  3. Create affiliate link database       → see docs/database-setup.md
  4. Extract IG/Threads session cookies   → see docs/cookie-extraction.md

EXECUTION FLOW (automatable per post):
  1. Read affiliate database              → select UNUSED link
  2. Pick category + hook style           → enforce rotation (no repeat in last 2 posts)
  3. Generate 2-3 post chain content      → fill template with product name
  4. Validate dedup                       → check link history + hook similarity
  5. Run poster.run_post(content)         → headless Playwright posts to Threads
  6. Mark link as USED in database        → persist history
  7. Sync database copies                 → if using multi-skill setup

RUN SINGLE POST:
  python -m threads_poster.cli post \
    --product "SKINTIFIC 5X Ceramide" \
    --link "https://s.shopee.co.id/XXXXX" \
    --category skincare \
    --hook-style edukasi

BATCH SCHEDULE (cron-compatible):
  python -m threads_poster.cli schedule --times "08:00,13:00,20:00"
```

---

## 🎯 What This Solves

Posting affiliate content to Threads manually 3x/day = 90+ posts/month. Each post needs:
- Unique hook angle (avoid spam flag)
- Real product image (not AI-generated, looks fake)
- Clean affiliate link insertion (no broken URLs)
- Category rotation (skincare → parfum → haircare → makeup)
- Dedup tracking (no link reuse, no hook repetition)

This toolkit automates ALL of that with proven anti-spam patterns.

---

## 🏗️ Architecture

```
threads-affiliate/
├── threads_poster/              Core library
│   ├── poster.py                Playwright automation (login → compose → submit)
│   ├── content_generator.py     Hook templates + 3-post chain builder
│   ├── dedup.py                 Link history + hook similarity check
│   ├── database.py              Affiliate link database (Markdown table)
│   ├── scheduler.py             Cron-compatible scheduler
│   └── cli.py                   Command-line interface
│
├── templates/
│   ├── hooks_skincare.json      9 hook styles × 3 variants each
│   ├── hooks_parfum.json
│   ├── hooks_haircare.json
│   ├── hooks_makeup.json
│   └── post2_templates.json     Middle post (review body) templates
│
├── docs/
│   ├── 01-account-setup.md      Threads + Shopee Affiliate signup
│   ├── 02-database-setup.md     Affiliate link database structure
│   ├── 03-cookie-extraction.md  Extract IG/Threads cookies
│   ├── 04-quickstart.md         First post in 5 minutes
│   ├── 05-customization.md      Custom hooks, voice, schedule
│   ├── 06-troubleshooting.md    Common errors + fixes
│   └── ethics-and-tos.md        Important warnings
│
├── examples/
│   ├── single_post.py
│   ├── batch_schedule.py
│   └── cron_integration.sh
│
└── tests/
```

---

## 📋 Prerequisites (Setup Steps for Human)

### Step 1 — Create Threads/Instagram Account

Threads is owned by Meta. You sign up via Instagram:

1. Download **Instagram** app (Android/iOS) or go to [instagram.com](https://instagram.com)
2. Sign up with email + phone + username
3. Verify email + phone
4. Open [threads.com](https://www.threads.com) → "Continue with Instagram" (Meta SSO auto-creates Threads profile)
5. Set Threads username (can match IG handle)
6. Post 2-3 manual posts first (warm up account, prevents new-account spam flag)

**⚠️ Important:**
- Use a real, human-looking profile (not bot-style with stock avatar)
- Wait 24-48 hours after first manual posts before running automation
- Don't post more than 3-5 times/day in first week

### Step 2 — Register Shopee Affiliate

The most accessible affiliate program in Indonesia:

1. Go to [affiliate.shopee.co.id](https://affiliate.shopee.co.id)
2. Sign in with your Shopee account (same as buyer)
3. Complete profile + KYC (KTP photo, NPWP optional)
4. Wait approval (24-48 hours)
5. After approved → Dashboard shows your unique affiliate links
6. Generate `s.shopee.co.id/XXXXX` short links per product

**Commission structure (as of 2026):**
- 3-15% per sale depending on category
- Beauty/skincare ~8-12% (highest category)
- Cookie window 7 days

### Step 3 — Build Affiliate Link Database

Create `data/affiliate_links.md` following the template in `docs/02-database-setup.md`. Structure:

```markdown
# Affiliate Link Database

## SKINCARE
| # | Product | Link | Status | Last Used |
|---|---------|------|--------|-----------|
| 1 | SKINTIFIC 5X Ceramide | `https://s.shopee.co.id/XXXXX` | ❌ UNUSED | - |
| 2 | Azarine Hydrasoothe SPF45 | `https://s.shopee.co.id/YYYYY` | ❌ UNUSED | - |
```

Aim for **50-100 links** across 4 categories before starting automation. More links = longer rotation runway.

### Step 4 — Extract Session Cookies

Threads uses Instagram session cookies (Meta SSO). Extract them via `browser_cookie3`:

```bash
python -m threads_poster.cli setup --extract-cookies --chrome-profile "Profile 1"
```

This will:
- Locate Chrome's Cookies SQLite database
- Extract Instagram + Threads domain cookies
- Save to `~/.threads_poster/cookies/` (NOT in repo, NEVER committed)
- Validate session with test API call

See [docs/03-cookie-extraction.md](docs/03-cookie-extraction.md) for manual extraction methods.

---

## 🚀 Quick Start

After completing all 4 prerequisites:

```bash
# Install
git clone https://github.com/d4ncboz/threads-affiliate
cd threads-affiliate
pip install -r requirements.txt
playwright install chromium

# Run a single post
python -m threads_poster.cli post \
  --product "SKINTIFIC 5X Ceramide Moisturizer" \
  --link "https://s.shopee.co.id/XXXXX" \
  --category skincare \
  --hook-style edukasi \
  --image /path/to/product.jpg

# Or schedule batch
python -m threads_poster.cli schedule --config schedule.json
```

---

## 🛡️ Anti-Spam Features

Built-in protection that production-tested through 1000+ posts:

| Feature | Implementation |
|---------|----------------|
| **Category rotation** | Enforce no-repeat in last 2 posts (db tracks last 2 categories) |
| **Hook style rotation** | 9 styles per category (edukasi, validasi, storytelling, problem-solving, hook-pancingan, transformasi, social-proof, urgency, controversy) |
| **Hook similarity check** | Cosine-style word overlap with last 5 posts, reject if >60% |
| **Link dedup** | Never repeat same affiliate link |
| **Human typing pattern** | 30ms delay per char, randomized between posts |
| **Clipboard paste for URL** | Cleaner link insertion than keyboard typing |
| **Image attachment** | Always uses REAL product image (no AI-gen for review credibility) |
| **3-post chain limit** | Threads native limit, prevents over-aggressive posting |

---

## 📅 Schedule Recommendation

Threads algorithm rewards consistency, penalizes burst posting. Recommended cadence:

```
Indonesian time zones (WIB):
  08:00 — Morning commute (high engagement)
  13:00 — Lunch break
  20:00 — Evening prime time

→ 3 posts/day × 4 categories = 12 unique product mentions/day
→ Each link used once = 50 links last ~4 days, 100 links ~8 days
```

Cron setup in `examples/cron_integration.sh`.

---

## ⚖️ Ethics & ToS Disclaimer

**Read [docs/ethics-and-tos.md](docs/ethics-and-tos.md) before running.**

Key principles:
- Don't post >5/day on a fresh account (Threads will flag)
- Don't reuse exact same hook text (spam detector)
- Only promote products you'd actually recommend
- Disclose affiliate relationship (`#ad`, `#affiliate`, `#produkpilihan`)
- Respect Threads ToS — automation use case is grey-area, posting frequency must look human

This tool is provided for educational and personal-use purposes. Use responsibly.

---

## 🤝 Contributing

PRs welcome for:
- New hook templates (other categories: fashion, gadget, food, baby)
- Better dedup algorithms
- Support for other affiliate platforms (Lazada, TikTok Shop, Tokopedia)
- Translations (English README, other SEA languages)

See [CONTRIBUTING.md](CONTRIBUTING.md).

---

## 📜 License

MIT License. See [LICENSE](LICENSE).

---

## 🙏 Acknowledgments

- Playwright team for reliable browser automation
- `browser_cookie3` for clean session extraction
- Indonesian affiliate community for hook style inspiration
