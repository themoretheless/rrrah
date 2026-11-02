# AGENTS.md — Instructions for AI Coding Agents

> **For:** Claude Code, Cursor, GitHub Copilot, Codex, Cody, Continue, Aider, and any AI coding agent.

This document is the **canonical operational guide** for AI agents working with this repository. Read this BEFORE running any task. Treat each section as authoritative; the README is for humans.

---

## ⚡ Quick Identity

| Property | Value |
|----------|-------|
| Project | `threads-affiliate` |
| Domain | Indonesian affiliate marketing automation for Threads (Meta) |
| Language | Python 3.10+ |
| Browser engine | Playwright (Chromium) |
| Auth method | Session cookies (Instagram + Threads via Meta SSO) |
| License | MIT |

---

## 🎯 What This Project Does

Posts 2-3 post chains to Threads automatically, using affiliate links from a Markdown database, with anti-spam rotation and dedup logic. Battle-tested for skincare/parfum/haircare/makeup affiliate categories in Indonesian language.

**Production use case:** Posting 3x/day across 4 categories without manual intervention, while avoiding Threads' spam detection.

---

## 🔐 Hard Constraints — Read First

### Things AI Agents MUST NOT Do

| Constraint | Why |
|------------|-----|
| ❌ NEVER commit `*.cookies.json`, `~/.threads_poster/`, `data/affiliate_links.md`, `data/post_history.json` | Contains session secrets + personal data |
| ❌ NEVER post >5 times/day from a single account | Triggers Threads spam ban |
| ❌ NEVER reuse the same affiliate link (dedup will reject) | Spam detection |
| ❌ NEVER hardcode credentials into Python files | Use `~/.threads_poster/cookies/session.json` |
| ❌ NEVER auto-follow / auto-DM / scrape other users | Violates Threads ToS hard |
| ❌ NEVER modify `posts[i]` content in production without running `dedup.check()` first | Will repost similar content → spam flag |

### Things AI Agents SHOULD Do

| Action | When |
|--------|------|
| ✅ Always run `dedup.check()` before `poster.post()` | Prevents duplicate posting |
| ✅ Always validate cookies before launching Playwright | Saves time on broken sessions |
| ✅ Always mark link USED after successful post | Maintains database integrity |
| ✅ Use exit codes (see below) for status communication | Allows cron monitoring |
| ✅ Log to `logs/cron.log` for batch runs | Audit trail |

---

## 📋 Setup Checklist

When a human asks you to "set up" this project, complete this checklist:

```
[ ] 1. Python 3.10+ installed                 → `python --version`
[ ] 2. Dependencies installed                 → `pip install -r requirements.txt`
[ ] 3. Playwright browsers installed          → `playwright install chromium`
[ ] 4. User has Instagram + Threads account   → ask, then refer to docs/01-account-setup.md
[ ] 5. User has Shopee Affiliate approval     → ask, then refer to docs/01-account-setup.md
[ ] 6. Cookies extracted to disk              → run `python -m threads_poster.cli setup --extract-cookies`
[ ] 7. Cookies validated                      → run `python -m threads_poster.cli setup --validate-cookies`
[ ] 8. Affiliate database populated (≥30 links) → ask user or refer to docs/02-database-setup.md
[ ] 9. First test post (in --headed mode)     → `python -m threads_poster.cli post-auto --category skincare --headed`
[ ] 10. Cron configured                       → see examples/cron_integration.sh
```

**If any step fails, STOP and report which step + the specific error. Do NOT proceed.**

---

## 🚀 Canonical Workflows

### Workflow A: Single Post

```bash
# Use when: user says "post about [product]"
python -m threads_poster.cli post \
  --product "PRODUCT_NAME" \
  --link "https://s.shopee.co.id/XXXXX" \
  --category {skincare|parfum|haircare|makeup} \
  --hook-style {edukasi|validasi_mental|storytelling|problem_solving|hook_pancingan|transformasi|social_proof|urgency|controversy} \
  --image /path/to/image.jpg  # optional but recommended
```

**Pre-conditions check:**
1. Cookies file exists at `~/.threads_poster/cookies/session.json`
2. Image file exists if `--image` provided
3. Affiliate link starts with `https://s.shopee.co.id/`

**Post-conditions check:**
1. Exit code 0 = success
2. `data/post_history.json` has new entry with today's timestamp
3. If `--db` was used, link is marked USED in database

### Workflow B: Auto-Pick from Database

```bash
# Use when: user says "post next product in [category]"
python -m threads_poster.cli post-auto \
  --category skincare \
  --db data/affiliate_links.md \
  --hook-style edukasi  # optional, random if omitted
```

**Pre-conditions check:**
1. `data/affiliate_links.md` exists and has ≥1 UNUSED entry in target category
2. Cookies valid

### Workflow C: Scheduled Batch

```bash
# Use when: user says "schedule posting" or "set up cron"
# Show: cat examples/cron_integration.sh
# Customize times/categories based on user's timezone (WIB = UTC+7)
```

### Workflow D: Adding New Hook Style

User wants to add a new hook style "controversy_v2":

```bash
# 1. Edit templates/hooks_{category}.json
# 2. Add new key under "hooks":
#    "controversy_v2": ["Variant 1 {product}", "Variant 2 {product}", ...]
# 3. Restart any running scheduler
# No code changes needed
```

### Workflow E: Adding New Category

User wants to add "fashion" category:

```python
# 1. Create templates/hooks_fashion.json (copy structure from hooks_skincare.json)
# 2. Add to templates/post2_templates.json:
#    "fashion": ["Outfit review template 1 {product}", ...]
# 3. Update threads_poster/content_generator.py:
#    CATEGORIES = ["skincare", "parfum", "haircare", "makeup", "fashion"]
# 4. Update threads_poster/cli.py:
#    Find `choices=CATEGORIES` — already references the constant, no change needed
# 5. Run tests: pytest tests/
```

---

## 📂 File Map for AI Agents

When user references a concept, here's where to find it:

| Concept | File |
|---------|------|
| Hook templates | `templates/hooks_{category}.json` |
| Mid-post review templates | `templates/post2_templates.json` |
| CTA templates | `templates/post3_cta_templates.json` |
| Hook generation logic | `threads_poster/content_generator.py` |
| Dedup rules | `threads_poster/dedup.py` |
| Database operations | `threads_poster/database.py` |
| Playwright posting logic | `threads_poster/poster.py` |
| Cookie extraction | `threads_poster/cookie_manager.py` |
| CLI commands | `threads_poster/cli.py` |
| Cookie storage | `~/.threads_poster/cookies/session.json` |
| Database | `data/affiliate_links.md` |
| Post history | `data/post_history.json` |
| Logs | `logs/cron.log` |

---

## 🚨 Exit Codes

The CLI uses standardized exit codes. Cron monitors should check these:

| Code | Meaning | Action |
|------|---------|--------|
| 0 | Success | Continue |
| 1 | Generic error / exception | Check logs, retry once |
| 2 | Cookies expired or invalid | Run `setup --extract-cookies` to refresh |
| 3 | Dedup rejected (link/hook reused) | Skip; try different category or hook |
| 4 | No UNUSED links available in DB | Add more links via `db --add` |
| 5 | Playwright/network failure | Retry with backoff; check internet |
| 6 | Image file not found | Continue without image OR fix path |
| 7 | Account locked / suspended | STOP all automation; manual intervention |

**For Python callers:**
```python
result = poster.post(...)
if not result.success:
    if "cookies" in result.error.lower():
        sys.exit(2)
    elif "dedup" in result.error.lower():
        sys.exit(3)
    # etc.
```

---

## 🌍 Environment Variables

The toolkit honors these env vars (override defaults):

| Variable | Default | Purpose |
|----------|---------|---------|
| `THREADS_COOKIES_PATH` | `~/.threads_poster/cookies/session.json` | Cookie file location |
| `THREADS_DB_PATH` | `data/affiliate_links.md` | Database file path |
| `THREADS_HISTORY_PATH` | `data/post_history.json` | Post history file |
| `THREADS_TEMPLATES_DIR` | `templates/` | Template directory |
| `THREADS_HEADLESS` | `true` | `false` for visible browser (debug) |
| `THREADS_USERNAME` | (none) | For post verification (e.g. `@yourname`) |
| `THREADS_TIMEOUT_MS` | `30000` | Playwright operation timeout |
| `THREADS_LOG_LEVEL` | `INFO` | `DEBUG`/`INFO`/`WARNING`/`ERROR` |
| `THREADS_IMAGE_DIR` | `data/product_images/` | Image lookup directory |

---

## 🧪 Testing

Before any PR/commit, run:

```bash
# Unit tests (no Playwright needed, fast)
pytest tests/ -v

# All tests should pass without --skip flags
# Coverage target: >70% for threads_poster/*.py
```

**When asked to add a feature, you MUST also add tests.**

Tests go in `tests/test_*.py`. Use pytest fixtures for templates_dir (see existing test_core.py).

---

## 📐 Code Style

- **Python:** PEP 8, max 100 cols
- **Type hints:** Required for public functions
- **Docstrings:** Google style, required for classes + public methods
- **Imports:** stdlib → third-party → local, alphabetical within each group
- **String formatting:** f-strings preferred over `.format()` or `%`
- **Path handling:** Use `pathlib.Path`, not `os.path`
- **JSON:** Always `indent=2`, `ensure_ascii=False` for ID text

---

## 🔄 Common Tasks Quick Reference

```bash
# Refresh cookies (run after Chrome re-login)
python -m threads_poster.cli setup --extract-cookies

# Check database stats
python -m threads_poster.cli db --stats

# List unused links by category
python -m threads_poster.cli db --list-unused --category skincare

# Add new link to database
python -m threads_poster.cli db --add \
  --category skincare \
  --product "Product Name" \
  --link "https://s.shopee.co.id/XXXXX"

# Reset all links to UNUSED (start new month)
python -m threads_poster.cli db --reset
```

---

## ⚠️ Error Handling Patterns

When you encounter these errors during execution, here's the canonical fix:

### "Cookies expired"
```bash
python -m threads_poster.cli setup --extract-cookies
# Then retry the original command
```

### "No UNUSED links available"
Either:
1. Tell the user to add more links (provide command above)
2. Or suggest `db --reset` if it's a new posting cycle

### "Editor element not found"
This means Threads UI changed. Steps:
1. Run with `--headed` flag to see what's happening
2. Inspect DOM in DevTools
3. Update selectors in `poster.py` → look for `_click_text()` and editor search

### "Account locked"
STOP. Tell the user:
1. Manually log in to Instagram + Threads via mobile app
2. Complete any security challenges (selfie, ID upload)
3. Wait 24-48 hours
4. Resume with reduced frequency (1 post/day for 1 week)

---

## 🎓 Indonesian Language Conventions

Hook templates use specific Indonesian gen-Z casual register:

- **Pronouns:** `gw`/`gue` (1st person), `lo` (2nd person) — NOT `aku`/`kamu`
- **Emojis:** Max 2 per hook, placed at sentence end
- **Length:** Hook ≤ 80 chars (mobile first-line visibility)
- **Tone:** Confident but not aggressive, conversational not formal
- **Forbidden words:** "promo", "diskon gila", "beli sekarang" (triggers Threads spam filter)

When generating new hooks, follow this pattern. When user asks for translations to formal Indonesian, output them in a separate `templates/hooks_*_formal.json` file.

---

## 🆘 When to Escalate to Human

You SHOULD NOT proceed and MUST ask the human when:

1. Cookies fail to extract (Chrome profile not found, browser_cookie3 errors)
2. Threads UI shows captcha/2FA prompt during posting
3. Account shows "Action Blocked" or "Suspended" notice
4. User wants to post >5 times/day (against project guidelines)
5. User asks to bypass dedup with `--force` flag (warn first)
6. User wants to add scraping of external user data (out of scope)
7. Database has <5 UNUSED links (warn before draining)

---

## 📚 Additional Documentation

For deeper context, read in order:
1. [README.md](./README.md) — User-facing intro
2. [docs/01-account-setup.md](./docs/01-account-setup.md) — Human signup steps
3. [docs/02-database-setup.md](./docs/02-database-setup.md) — Database structure
4. [docs/03-cookie-extraction.md](./docs/03-cookie-extraction.md) — Auth flow
5. [docs/04-quickstart.md](./docs/04-quickstart.md) — First post tutorial
6. [docs/05-customization.md](./docs/05-customization.md) — Hooks, voice, schedule
7. [docs/06-troubleshooting.md](./docs/06-troubleshooting.md) — Error patterns
8. [docs/ethics-and-tos.md](./docs/ethics-and-tos.md) — Legal/ethics

---

## 🤝 Contributing as an AI Agent

If you're an AI agent making changes:

1. **Read the relevant doc(s)** before editing
2. **Run tests** before committing
3. **Update tests** if you change behavior
4. **Update this AGENTS.md** if you change workflows/exit codes/env vars
5. **Write commit messages** in conventional commit format:
   - `feat:` new feature
   - `fix:` bug fix
   - `docs:` documentation
   - `refactor:` no behavior change
   - `test:` test only
6. **Never commit secrets** (use `.gitignore`)

---

**Repository:** https://github.com/d4ncboz/threads-affiliate  
**Issues:** https://github.com/d4ncboz/threads-affiliate/issues  
**License:** MIT
