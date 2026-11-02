# 06 — Troubleshooting

Common errors and their fixes.

## Cookie Issues

### "Cookies expired" / "Session invalid"

**Cause**: Instagram/Threads session cookies expire (~30-60 days).

**Fix**:
```bash
# Re-login to Instagram in your Chrome browser
# Then re-extract:
python -m threads_poster.cli setup --extract-cookies
```

### "No Instagram cookies in file"

**Cause**: Wrong Chrome profile, or you weren't logged in when extracting.

**Fix**:
```bash
# List profiles
python -m threads_poster.cli setup --list-chrome-profiles

# Use the right profile name
python -m threads_poster.cli setup --extract-cookies --chrome-profile "Profile 1"
```

### "useragent mismatch" error from i.instagram.com

**Cause**: Using `i.instagram.com` endpoint (deprecated/strict UA check).

**Fix**: Already handled in the toolkit. We use `www.instagram.com/api/v1/users/web_profile_info/` instead.

---

## Posting Failures

### "Editor element not found"

**Cause**: Threads UI updated, selectors changed.

**Fix**:
1. Run with `--headed` flag to see what's happening
2. Inspect element in DevTools
3. Update selectors in `threads_poster/poster.py` → `post()` method, line `for sel in [...]`

### "Add to thread not found"

**Cause**: Threads changed button text or layout.

**Fix**: Check `_click_add_to_thread()` in `poster.py`. The toolkit looks for:
- "Add to thread" (English)
- "Tambahkan ke utas" (Indonesian)

Add new locale by adding to the candidates list.

### "Link not inserted after retries"

**Cause**: Clipboard paste failing (browser context issues).

**Fix**: The toolkit auto-falls-back to keyboard typing. If that also fails:
1. Check if Playwright has clipboard permissions: `context.grant_permissions(["clipboard-read", "clipboard-write"])`
2. Try alternative paste method (Meta+v on Mac, Control+v on Linux/Windows)
3. Verify URL doesn't contain special chars that get auto-corrected

### Post submitted but doesn't appear on profile

**Cause**: Threads spam-flagged the post or it's pending review.

**Fix**:
1. Wait 5-10 minutes (sometimes delayed)
2. Check email — Meta might have sent suspension notice
3. Log in via web to see flagged posts
4. Reduce posting frequency

---

## Account Issues

### "Account temporarily disabled" / "Account locked"

**Cause**: Threads detected unusual behavior. Common triggers:
- Posting too frequently (>5/day on new account)
- Same affiliate link multiple times
- Hook text too similar across posts
- Login from new device + immediate burst posting

**Fix**:
1. Stop ALL automation immediately
2. Log in manually via mobile app
3. Complete any security challenges (selfie, ID verification)
4. Wait 24-48 hours before resuming automation
5. Reduce posting frequency to 1-2/day for 1 week
6. Review dedup history — ensure no duplicates

### "Continue with Instagram" button not found

**Cause**: Threads SSO UI changed, or you're already logged in.

**Fix**: Add new button text to `_click_text()` candidates list.

---

## Database Issues

### "No UNUSED links available"

**Cause**: All your affiliate links have been used.

**Fix**:
- Generate new affiliate links from Shopee dashboard
- Add to database: `python -m threads_poster.cli db --add --category skincare --product "..." --link "..."`
- Or reset old ones: `python -m threads_poster.cli db --reset` (⚠️ allows reuse)

### Database parse errors

**Cause**: Malformed Markdown table (missing pipe, wrong row format).

**Fix**: Validate format:
```markdown
| # | Product | Link | Status | Last Used |
|---|---------|------|--------|-----------|
| 1 | Product Name | `https://s.shopee.co.id/X` | ❌ UNUSED | - |
```

Required:
- Exactly 5 columns
- Link wrapped in backticks
- Status starts with ❌ UNUSED or ✅ USED

---

## Playwright Issues

### "browser binary not found"

**Cause**: Chromium not installed.

**Fix**:
```bash
playwright install chromium
```

### "Target page, context or browser has been closed"

**Cause**: Network issue, or Threads/IG returned redirect that crashed page.

**Fix**:
1. Check internet connection
2. Verify cookies are valid (`--validate-cookies`)
3. Run with `--headed` to see what happens
4. Increase `timeout_ms` in poster constructor

### Image upload not working

**Cause**: File input not found in DOM.

**Fix**: The toolkit tries multiple selectors. If still fails:
1. Verify image file exists and is JPEG/PNG
2. Verify file size < 10 MB (Threads limit)
3. Try with `--headed` to see if image picker dialog opens
4. Post without image as fallback (text-only post still valid)

---

## Performance

### Posts take 30-60 seconds each

This is **normal**. The bottleneck is:
- 3-5s page navigation to threads.com
- 5-10s for Meta SSO login
- 5-10s for editor render
- 1-2s per character typing (intentional human-like)
- 10s post-submit verification

Don't try to speed this up — it will trigger spam detection.

### Cron job failing silently

**Cause**: Cron has minimal env vars, can't find Python or Playwright binary.

**Fix**:
```bash
# In crontab, use absolute paths:
0 1 * * * /usr/bin/python3 /full/path/to/threads_poster/cli.py post-auto --category skincare

# Or source environment:
0 1 * * * source ~/.bashrc && cd /path/to/repo && python -m threads_poster.cli post-auto --category skincare
```

---

## Getting Help

If issue not listed here:

1. Run with `--headed` to see browser behavior
2. Check `logs/cron.log` for stack traces
3. Open GitHub issue with:
   - Python version
   - OS (macOS/Linux/Windows)
   - Playwright version (`playwright --version`)
   - Full error output
   - Steps to reproduce (NO real cookies, redact account info)

**Never share cookie files or session data in public issues.**
