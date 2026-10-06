"""Command-line interface for threads_poster."""
import argparse
import json
import sys
from pathlib import Path

from . import __version__
from .content_generator import ContentGenerator, CATEGORIES
from .database import AffiliateDatabase
from .dedup import DedupChecker
from .poster import ThreadsPoster


def cmd_setup(args):
    """Setup commands: cookies, profile listing."""
    from .cookie_manager import (
        extract_cookies, validate_cookies, list_chrome_profiles,
    )

    if args.list_chrome_profiles:
        profiles = list_chrome_profiles()
        if not profiles:
            print("No Chrome profiles found.")
            return
        print("Available Chrome profiles:")
        for p in profiles:
            print(f"  - {p.name}")
        return

    if args.extract_cookies:
        try:
            out = extract_cookies(
                chrome_profile=args.chrome_profile or "Default",
                output_path=args.output,
            )
            print(f"✅ Cookies extracted → {out}")
            print(f"🔐 Permission: 600 (owner read/write only)")
            # auto-validate
            ok, details = validate_cookies(out)
            if ok:
                print(f"✅ Session valid (user_id: {details.get('session_user_id', '?')})")
            else:
                print(f"⚠️  Session invalid: {details.get('error', '?')}")
        except Exception as e:
            print(f"❌ Extraction failed: {e}")
            sys.exit(1)
        return

    if args.validate_cookies:
        cookies_path = args.cookies or Path.home() / ".threads_poster/cookies/session.json"
        ok, details = validate_cookies(cookies_path)
        if ok:
            print("✅ Cookies valid")
            print(json.dumps(details, indent=2))
        else:
            print("❌ Cookies invalid")
            print(json.dumps(details, indent=2))
            sys.exit(2)
        return


def cmd_post(args):
    """Post a single thread."""
    # Resolve config
    cookies_path = Path(args.cookies or "~/.threads_poster/cookies/session.json").expanduser()
    templates_dir = Path(args.templates or "templates")
    history_path = Path(args.history or "data/post_history.json")

    # Generate content
    gen = ContentGenerator(templates_dir)
    content = gen.generate_chain(
        product=args.product,
        affiliate_link=args.link,
        category=args.category,
        hook_style=args.hook_style,
        num_posts=args.num_posts,
    )

    # Dedup check
    dedup = DedupChecker(history_path)
    ok, reason = dedup.check(
        affiliate_link=args.link,
        hook_category=content["hook_category"],
        hook_text=content["hook_text"],
        category=args.category,
    )
    if not ok and not args.force:
        print(f"❌ DEDUP REJECTED: {reason}")
        print("Use --force to override (NOT recommended)")
        sys.exit(1)

    # Post
    poster = ThreadsPoster(
        cookies_path=cookies_path,
        headless=not args.headed,
    )
    result = poster.post(
        product=args.product,
        affiliate_link=args.link,
        hook_text=content["post_1"],
        post_2=content["post_2"],
        post_3=content.get("post_3"),
        hook_category=content["hook_category"],
        category=args.category,
        image_path=args.image,
        username=args.username,
    )

    if result.success:
        print(f"✅ Posted: {result.product}")
        if result.post_url:
            print(f"   URL: {result.post_url}")

        # Record to history
        dedup.record({
            "date": result.timestamp,
            "hook_category": result.hook_category,
            "category": args.category,
            "hook_text": result.hook_text,
            "product": result.product,
            "affiliate_link": result.affiliate_link,
            "num_posts": result.num_posts,
            "status": "posted",
        })

        # Mark link as used in DB if --db specified
        if args.db:
            db = AffiliateDatabase(args.db)
            try:
                db.mark_used(args.link, note=f"{result.hook_category} hook")
                print(f"   ✅ Marked as USED in database")
            except Exception as e:
                print(f"   ⚠️  Failed to update database: {e}")
    else:
        print(f"❌ Post failed: {result.error}")
        sys.exit(1)


def cmd_post_auto(args):
    """Pick next unused link automatically + post."""
    db = AffiliateDatabase(args.db or "data/affiliate_links.md")
    try:
        link, product, detected_category = db.next_unused(category=args.category)
    except ValueError as e:
        print(f"❌ {e}")
        sys.exit(1)

    print(f"📦 Auto-picked: {product}")
    print(f"   Category: {detected_category}")
    print(f"   Link: {link}")

    # Mimic cmd_post with picked values
    args.product = product
    args.link = link
    args.category = args.category or detected_category
    cmd_post(args)


def cmd_db(args):
    """Database management commands."""
    db = AffiliateDatabase(args.db or "data/affiliate_links.md")

    if args.stats:
        stats = db.stats()
        print(json.dumps(stats, indent=2))
        return

    if args.list_unused:
        rows = db._parse_rows(db._read())
        unused = [r for r in rows if "UNUSED" in r["status"].upper()]
        if args.category:
            unused = [r for r in unused if r["category"] == args.category]
        print(f"Available links: {len(unused)}")
        for r in unused[:30]:
            print(f"  [{r['category']:10}] #{r['num']:3} {r['product'][:60]}")
        return

    if args.add:
        if not (args.product and args.link and args.category):
            print("❌ --add requires --product, --link, --category")
            sys.exit(1)
        db.add_link(args.category, args.product, args.link)
        print(f"✅ Added: [{args.category}] {args.product}")
        return

    if args.reset:
        confirm = input("⚠️  Reset ALL links to UNUSED? (yes/no): ")
        if confirm.lower() == "yes":
            db.reset_all()
            print("✅ All links reset to UNUSED")
        else:
            print("Cancelled.")
        return


def cmd_schedule(args):
    """Show schedule helper info."""
    print("Cron schedule helper. Example crontab entries:")
    print()
    print("# 3 posts per day at 08:00, 13:00, 20:00 WIB (UTC+7)")
    print("0 1 * * * cd /path/to/repo && python -m threads_poster.cli post-auto --category skincare")
    print("0 6 * * * cd /path/to/repo && python -m threads_poster.cli post-auto --category parfum")
    print("0 13 * * * cd /path/to/repo && python -m threads_poster.cli post-auto --category haircare")
    print()
    print("# Cookie refresh every 6 hours")
    print("0 */6 * * * cd /path/to/repo && python -m threads_poster.cli setup --extract-cookies")


def main():
    parser = argparse.ArgumentParser(
        prog="threads_poster.cli",
        description="Threads Affiliate Poster ID — CLI",
    )
    parser.add_argument(
        "--version", action="version", version=f"threads-poster {__version__}"
    )
    subparsers = parser.add_subparsers(dest="cmd", required=True)

    # --- setup ---
    p_setup = subparsers.add_parser("setup", help="Setup cookies + configuration")
    p_setup.add_argument("--extract-cookies", action="store_true",
                         help="Extract IG+Threads cookies from Chrome")
    p_setup.add_argument("--validate-cookies", action="store_true",
                         help="Test if existing cookies are valid")
    p_setup.add_argument("--list-chrome-profiles", action="store_true",
                         help="List available Chrome profiles")
    p_setup.add_argument("--chrome-profile", default=None,
                         help="Chrome profile name (default: 'Default')")
    p_setup.add_argument("--output", "-o", default=None, type=Path,
                         help="Where to save cookies (default: ~/.threads_poster/cookies/session.json)")
    p_setup.add_argument("--cookies", "-c", default=None, type=Path,
                         help="Path to existing cookies file for --validate")
    p_setup.set_defaults(func=cmd_setup)

    # --- post ---
    p_post = subparsers.add_parser("post", help="Post a single thread")
    p_post.add_argument("--product", "-p", required=True, help="Product name")
    p_post.add_argument("--link", "-l", required=True, help="Affiliate URL")
    p_post.add_argument("--category", required=True, choices=CATEGORIES)
    p_post.add_argument("--hook-style", default=None,
                        help="Hook style (default: random)")
    p_post.add_argument("--num-posts", type=int, default=3, choices=[2, 3])
    p_post.add_argument("--image", default=None, help="Product image path")
    p_post.add_argument("--username", default=None,
                        help="Your Threads username for post verification")
    p_post.add_argument("--cookies", default=None, help="Cookies file path")
    p_post.add_argument("--templates", default=None, help="Templates dir")
    p_post.add_argument("--history", default=None, help="Post history file")
    p_post.add_argument("--db", default=None, help="Affiliate DB to mark used")
    p_post.add_argument("--headed", action="store_true",
                        help="Run with visible browser (debug)")
    p_post.add_argument("--force", action="store_true",
                        help="Bypass dedup check (NOT recommended)")
    p_post.set_defaults(func=cmd_post)

    # --- post-auto ---
    p_auto = subparsers.add_parser("post-auto",
                                    help="Auto-pick UNUSED link from database + post")
    p_auto.add_argument("--category", default=None, choices=CATEGORIES,
                        help="Filter category (default: any UNUSED)")
    p_auto.add_argument("--hook-style", default=None)
    p_auto.add_argument("--num-posts", type=int, default=3, choices=[2, 3])
    p_auto.add_argument("--image", default=None)
    p_auto.add_argument("--username", default=None)
    p_auto.add_argument("--cookies", default=None)
    p_auto.add_argument("--templates", default=None)
    p_auto.add_argument("--history", default=None)
    p_auto.add_argument("--db", default=None)
    p_auto.add_argument("--headed", action="store_true")
    p_auto.add_argument("--force", action="store_true")
    p_auto.set_defaults(func=cmd_post_auto)

    # --- db ---
    p_db = subparsers.add_parser("db", help="Database management")
    p_db.add_argument("--db", default=None)
    p_db.add_argument("--stats", action="store_true")
    p_db.add_argument("--list-unused", action="store_true")
    p_db.add_argument("--category", default=None, choices=CATEGORIES)
    p_db.add_argument("--add", action="store_true", help="Add new link")
    p_db.add_argument("--product", default=None)
    p_db.add_argument("--link", default=None)
    p_db.add_argument("--reset", action="store_true",
                      help="Reset all links to UNUSED")
    p_db.set_defaults(func=cmd_db)

    # --- schedule ---
    p_sched = subparsers.add_parser("schedule", help="Cron schedule helper")
    p_sched.set_defaults(func=cmd_schedule)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
