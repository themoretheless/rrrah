"""Playwright-based Threads poster — handles login, compose, submit."""
from __future__ import annotations

import json
import re
import time
from dataclasses import dataclass, asdict
from datetime import datetime
from pathlib import Path
from typing import Optional


@dataclass
class PostResult:
    """Result of a post attempt."""
    success: bool
    product: str = ""
    affiliate_link: str = ""
    hook_category: str = ""
    hook_text: str = ""
    num_posts: int = 0
    post_url: str = ""
    error: str = ""
    timestamp: str = ""

    def to_dict(self) -> dict:
        return asdict(self)


class ThreadsPoster:
    """Post 2-3 post chains to Threads via Playwright + cookie auth."""

    DEFAULT_VIEWPORT = {"width": 1280, "height": 800}

    def __init__(
        self,
        cookies_path: Path | str,
        headless: bool = True,
        viewport: Optional[dict] = None,
        timeout_ms: int = 30000,
    ):
        self.cookies_path = Path(cookies_path).expanduser()
        self.headless = headless
        self.viewport = viewport or self.DEFAULT_VIEWPORT
        self.timeout_ms = timeout_ms

    def _load_cookies(self) -> tuple[dict, dict]:
        """Load IG + Threads session cookies from disk.

        Expected format:
            {
                "instagram": {"sessionid": "...", "csrftoken": "...", ...},
                "threads": {"sessionid": "...", ...}
            }
        """
        if not self.cookies_path.exists():
            raise FileNotFoundError(
                f"Cookies not found at {self.cookies_path}. "
                "Run `python -m threads_poster.cli setup --extract-cookies` first."
            )

        raw = json.loads(self.cookies_path.read_text())
        ig_cookies = raw.get("instagram", {})
        threads_cookies = raw.get("threads", {})

        # Sanitize control chars
        ig_clean = {
            k: re.sub(r"[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]", "", str(v))
            for k, v in ig_cookies.items()
        }
        threads_clean = {
            k: re.sub(r"[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]", "", str(v))
            for k, v in threads_cookies.items()
        }

        return ig_clean, threads_clean

    def _build_playwright_cookies(
        self,
        ig_cookies: dict,
        threads_cookies: dict,
    ) -> list[dict]:
        """Convert flat dict cookies to Playwright cookie format."""
        all_cookies = []
        httponly_names = {"sessionid", "ig_did", "datr", "mid", "rur", "ig_nrcb"}

        # Instagram cookies → .instagram.com
        for name, value in ig_cookies.items():
            all_cookies.append({
                "name": name, "value": value,
                "domain": ".instagram.com", "path": "/",
                "httpOnly": name in httponly_names,
                "secure": True, "sameSite": "Lax",
            })

        # Threads cookies → .threads.com + .threads.net
        for name, value in threads_cookies.items():
            for domain in [".threads.com", ".threads.net"]:
                all_cookies.append({
                    "name": name, "value": value,
                    "domain": domain, "path": "/",
                    "httpOnly": name in {"sessionid", "mid", "rur"},
                    "secure": True, "sameSite": "Lax",
                })

        return all_cookies

    def _click_text(self, page, candidates: list[str]) -> bool:
        """Click first element matching any text in candidates."""
        js = """
        (candidates) => {
            for (const el of document.querySelectorAll('div[role="button"], button, span, a')) {
                const txt = el.textContent.trim();
                if (candidates.includes(txt)) {
                    const rect = el.getBoundingClientRect();
                    if (rect.width > 0 && rect.height > 0 && rect.height < 100) {
                        el.click();
                        return "clicked: " + txt;
                    }
                }
            }
            return null;
        }
        """
        result = page.evaluate(js, candidates)
        return result is not None

    def _click_add_to_thread(self, page) -> bool:
        """Click 'Add to thread' button to add another post in chain."""
        js = """
        () => {
            for (const el of document.querySelectorAll('span')) {
                const txt = el.textContent.trim();
                if (txt === "Add to thread" || txt === "Tambahkan ke utas") {
                    const rect = el.getBoundingClientRect();
                    if (rect.width > 0 && rect.height > 0 && rect.y > 0) {
                        el.click();
                        return true;
                    }
                }
            }
            return false;
        }
        """
        return page.evaluate(js)

    def _paste_text(self, page, text: str):
        """Paste text via clipboard (cleaner than keyboard.type for URLs)."""
        page.evaluate(
            """
            async (text) => {
                try {
                    await navigator.clipboard.writeText(text);
                } catch(e) {
                    const ta = document.createElement('textarea');
                    ta.value = text;
                    document.body.appendChild(ta);
                    ta.select();
                    document.execCommand('copy');
                    document.body.removeChild(ta);
                }
            }
            """,
            text,
        )
        time.sleep(0.3)
        page.keyboard.press("Meta+v")  # macOS. On Linux/Win, switch to Control+v
        time.sleep(1)

    def _type_text(self, page, text: str, delay: int = 30):
        """Type text handling newlines as Enter."""
        lines = text.split("\n")
        for i, line in enumerate(lines):
            page.keyboard.type(line, delay=delay)
            if i < len(lines) - 1:
                page.keyboard.press("Enter")
                time.sleep(0.5)

    def post(
        self,
        product: str,
        affiliate_link: str,
        hook_text: str,
        post_2: str,
        post_3: Optional[str] = None,
        hook_category: str = "",
        category: str = "",
        image_path: Optional[str] = None,
        username: Optional[str] = None,
    ) -> PostResult:
        """
        Post a 2-3 chain thread.

        Args:
            product: Product name
            affiliate_link: Affiliate URL (will be inserted in last post)
            hook_text: Post 1 content (hook)
            post_2: Post 2 content (review body)
            post_3: Post 3 content (CTA + link). If None, link goes to post_2.
            hook_category: e.g. "edukasi" for tracking
            category: e.g. "skincare" for tracking
            image_path: Optional product image attached to post 1
            username: Your Threads username (for verification)

        Returns:
            PostResult
        """
        from playwright.sync_api import sync_playwright

        posts = [hook_text, post_2]
        if post_3:
            posts.append(post_3)
        num_posts = len(posts)

        if num_posts < 2 or num_posts > 3:
            return PostResult(
                success=False, product=product, affiliate_link=affiliate_link,
                error=f"Threads supports 2-3 posts only (got {num_posts})",
                timestamp=datetime.now().isoformat(),
            )

        ig_cookies, threads_cookies = self._load_cookies()
        cookies = self._build_playwright_cookies(ig_cookies, threads_cookies)

        with sync_playwright() as p:
            browser = p.chromium.launch(headless=self.headless)
            context = browser.new_context(viewport=self.viewport)
            context.grant_permissions(["clipboard-read", "clipboard-write"])
            context.add_cookies(cookies)

            page = context.new_page()

            try:
                # 1. Navigate to Instagram (warm session)
                page.goto("https://www.instagram.com/",
                         wait_until="domcontentloaded", timeout=self.timeout_ms)
                time.sleep(3)

                # 2. Trigger Meta SSO via Threads login
                page.goto("https://www.threads.com/login",
                         wait_until="domcontentloaded", timeout=self.timeout_ms)
                time.sleep(5)
                self._click_text(page, [
                    "Continue with Instagram",
                    "Lanjutkan dengan Instagram",
                ])
                time.sleep(10)

                # 3. Navigate to feed
                page.goto("https://www.threads.com/",
                         wait_until="domcontentloaded", timeout=self.timeout_ms)
                time.sleep(5)

                # 4. Click "New thread" / "Buat"
                self._click_text(page, [
                    "New thread", "Utas baru", "Buat", "Create",
                ])
                time.sleep(5)

                # 5. Wait for editor
                editor = None
                for attempt in range(7):
                    for sel in ['[contenteditable="true"]',
                                '[data-lexical-editor="true"]',
                                'div[role="textbox"]']:
                        editors = page.locator(sel)
                        if editors.count() > 0:
                            editor = editors.first
                            break
                    if editor:
                        break
                    time.sleep(2)

                if not editor:
                    return PostResult(
                        success=False, product=product, affiliate_link=affiliate_link,
                        error="Editor element not found after 14s",
                        timestamp=datetime.now().isoformat(),
                    )

                # 6. Type each post in chain
                for i in range(num_posts):
                    if i > 0:
                        # Add to thread for posts 2+
                        self._click_add_to_thread(page)
                        time.sleep(3)
                        editors = page.locator('[contenteditable="true"]')
                        if i >= editors.count():
                            return PostResult(
                                success=False, product=product, affiliate_link=affiliate_link,
                                error=f"Editor {i} doesn't exist (only {editors.count()})",
                                timestamp=datetime.now().isoformat(),
                            )
                        editors.nth(i).click()
                        time.sleep(1)
                    else:
                        editor.click()
                        time.sleep(1)

                    post_text = posts[i]

                    # Last post: include link via clipboard paste
                    if i == num_posts - 1:
                        if post_text:
                            self._type_text(page, post_text)
                            time.sleep(1)
                            page.keyboard.press("Enter")
                            time.sleep(0.5)
                        self._paste_text(page, affiliate_link)
                        time.sleep(3)
                    else:
                        self._type_text(page, post_text)

                    time.sleep(1)

                # 7. Verify link is in last editor
                editors = page.locator('[contenteditable="true"]')
                last_text = editors.nth(num_posts - 1).inner_text()
                if affiliate_link not in last_text:
                    # Retry paste
                    editors.nth(num_posts - 1).click()
                    time.sleep(1)
                    page.keyboard.press("End")
                    page.keyboard.press("Enter")
                    time.sleep(0.5)
                    self._paste_text(page, affiliate_link)
                    time.sleep(2)
                    last_text = editors.nth(num_posts - 1).inner_text()

                if affiliate_link not in last_text:
                    return PostResult(
                        success=False, product=product, affiliate_link=affiliate_link,
                        error="Affiliate link not inserted after retries",
                        timestamp=datetime.now().isoformat(),
                    )

                # 8. Optional: attach image to first post
                if image_path and Path(image_path).exists():
                    try:
                        file_input = page.locator(
                            'input[type="file"][accept*="image"]'
                        )
                        if file_input.count() > 0:
                            file_input.first.set_input_files(image_path)
                            time.sleep(5)
                    except Exception:
                        # Continue without image rather than fail post
                        pass

                # 9. Submit
                submit_clicked = False
                for label in ["Post", "Kirim"]:
                    try:
                        btn = page.locator(f'div[role="button"]:has-text("{label}")').last
                        if btn.count() > 0:
                            btn.click(force=True)
                            submit_clicked = True
                            break
                    except Exception:
                        continue

                if not submit_clicked:
                    return PostResult(
                        success=False, product=product, affiliate_link=affiliate_link,
                        error="Submit button (Post/Kirim) not clickable",
                        timestamp=datetime.now().isoformat(),
                    )

                time.sleep(10)

                # 10. Verify on profile
                post_url = ""
                if username:
                    profile_url = f"https://www.threads.com/{username.lstrip('@')}"
                    page.goto(profile_url, wait_until="domcontentloaded",
                             timeout=self.timeout_ms)
                    time.sleep(5)
                    page_text = page.content()
                    if hook_text[:30] in page_text or product[:20] in page_text:
                        post_url = profile_url

            except Exception as e:
                return PostResult(
                    success=False, product=product, affiliate_link=affiliate_link,
                    error=f"Exception during post: {type(e).__name__}: {e}",
                    timestamp=datetime.now().isoformat(),
                )
            finally:
                browser.close()

        return PostResult(
            success=True,
            product=product,
            affiliate_link=affiliate_link,
            hook_category=hook_category,
            hook_text=hook_text[:80],
            num_posts=num_posts,
            post_url=post_url,
            timestamp=datetime.now().isoformat(),
        )
