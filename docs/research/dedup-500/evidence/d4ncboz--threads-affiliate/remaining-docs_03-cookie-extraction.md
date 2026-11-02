# 03 — Cookie Extraction

Threads uses Meta SSO. To authenticate the Playwright session without password-based login (which often triggers 2FA), we extract session cookies from a Chrome browser that's already logged in.

## Why Cookie-Based Authentication?

| Method | Pros | Cons |
|--------|------|------|
| Username/password | Simple | Triggers 2FA frequently, blocks "unfamiliar device" |
| OAuth token | Most secure | Threads has no public OAuth |
| **Session cookies** | Bypasses 2FA, mimics returning user | Cookies expire after 30-60 days |

We use the session cookie approach.

---

## Method A: Automated Extraction (Recommended)

```bash
# After logging into Instagram on your Chrome browser:
python -m threads_poster.cli setup --extract-cookies
```

Output:
```
✅ Found Chrome Default profile
✅ Extracted 28 cookies from .instagram.com
✅ Extracted 12 cookies from .threads.com
✅ Saved to ~/.threads_poster/cookies/session.json
✅ Validated session (logged in as: @yourusername)
```

### Specifying Chrome Profile

If you have multiple Chrome profiles:

```bash
# List available profiles
python -m threads_poster.cli setup --list-chrome-profiles

# Extract from specific profile  
python -m threads_poster.cli setup --extract-cookies --chrome-profile "Profile 2"
```

### Required Pre-State

Before running extraction:
1. Open Chrome with the desired profile
2. Login to [instagram.com](https://instagram.com) (must show feed, not login page)
3. Visit [threads.com](https://www.threads.com) (auto-SSO from IG)
4. Close Chrome (so SQLite write-lock releases)
5. Run extraction command

---

## Method B: Manual Extraction (Fallback)

If automated extraction fails (Chrome profile not detected, encrypted cookies):

### Via Chrome DevTools

1. Open Chrome → go to [instagram.com](https://instagram.com), log in
2. Open DevTools (F12) → Application tab → Cookies → `https://www.instagram.com`
3. Copy these critical cookies:
   - `sessionid` (most important)
   - `csrftoken`
   - `mid`
   - `ds_user_id`
   - `ig_did`
   - `rur`
4. Go to `https://www.threads.com` → DevTools → Cookies → `https://www.threads.com`
5. Copy:
   - `sessionid` (different from IG's!)
   - `ig_did`
   - `mid`
   - `rur`

Save as `~/.threads_poster/cookies/session.json`:

```json
{
  "instagram": {
    "sessionid": "...",
    "csrftoken": "...",
    "mid": "...",
    "ds_user_id": "...",
    "ig_did": "...",
    "rur": "..."
  },
  "threads": {
    "sessionid": "...",
    "ig_did": "...",
    "mid": "...",
    "rur": "..."
  }
}
```

---

## Cookie Validation

After extraction, validate the session works:

```bash
python -m threads_poster.cli setup --validate-cookies
```

Expected output:
```
🔍 Testing Instagram session...
✅ IG session valid (user: @yourusername, ID: 12345...)

🔍 Testing Threads session...
✅ Threads session valid (user: @yourusername)

✅ All cookies operational
```

If validation fails:

| Error | Cause | Fix |
|-------|-------|-----|
| `IG session expired (status: fail)` | Session cookies expired | Re-login in Chrome, re-extract |
| `403 Forbidden` | Account locked or 2FA required | Login manually, complete 2FA, re-extract |
| `useragent mismatch` | Using www.threads.net (deprecated) | Use www.threads.com only |
| `No cookies found` | Wrong Chrome profile | Use `--chrome-profile` flag |

---

## Cookie Refresh Strategy

Session cookies expire approximately every 30-60 days. Strategies:

### Automatic Refresh Cron

```bash
# crontab -e
0 */6 * * * cd /path/to/repo && python -m threads_poster.cli setup --extract-cookies
```

This refreshes cookies every 6 hours from your Chrome (which presumably stays logged in).

### Detection

The poster automatically detects expired cookies and exits with code `2`:

```bash
python -m threads_poster.cli post --product "..." --link "..."
# Exit code 2 = cookies expired → trigger re-login workflow
```

---

## Security

### NEVER commit cookies to git

The `.gitignore` is configured to exclude:
- `*cookies.json`
- `.threads_poster/`
- `cookies/`

### File Permissions

```bash
chmod 600 ~/.threads_poster/cookies/session.json
```

### Multi-Account Setup

Each account gets its own cookie file:

```
~/.threads_poster/cookies/
├── account_skincare.json
├── account_fashion.json
└── account_tech.json
```

Pass via `--cookie-file`:

```bash
python -m threads_poster.cli post --cookie-file ~/.threads_poster/cookies/account_skincare.json --product "..." --link "..."
```

---

## ✅ Checklist

- [ ] Chrome installed and logged into IG + Threads
- [ ] Cookies extracted to `~/.threads_poster/cookies/session.json`
- [ ] `validate-cookies` returns ✅ for both IG and Threads
- [ ] Cookie file permissions set to 600 (Unix)
- [ ] Backup of cookies in secure location

Next: [04 — Quickstart](./04-quickstart.md)
