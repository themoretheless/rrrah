"""Batch scheduling: auto-pick from database + rotate categories across the day."""
import time
from threads_poster import (
    ThreadsPoster, ContentGenerator, AffiliateDatabase, DedupChecker
)

# Schedule: 3 posts per day at 08:00, 13:00, 20:00 WIB
# Each slot picks a different category to enforce rotation
SCHEDULE = {
    "08:00": "skincare",
    "13:00": "parfum",
    "20:00": "haircare",
}


def post_for_slot(category: str) -> bool:
    db = AffiliateDatabase("data/affiliate_links.md")
    gen = ContentGenerator(templates_dir="templates")
    dedup = DedupChecker("data/post_history.json")
    poster = ThreadsPoster(cookies_path="~/.threads_poster/cookies/session.json")

    # Pick next unused link
    try:
        link, product, detected_category = db.next_unused(category=category)
    except ValueError as e:
        print(f"❌ {e}")
        return False

    # Generate content
    content = gen.generate_chain(
        product=product,
        affiliate_link=link,
        category=detected_category,
    )

    # Dedup
    ok, reason = dedup.check(
        affiliate_link=link,
        hook_category=content["hook_category"],
        hook_text=content["hook_text"],
        category=detected_category,
    )
    if not ok:
        print(f"⏭️  Skipped: {reason}")
        return False

    # Post
    result = poster.post(
        product=product,
        affiliate_link=link,
        hook_text=content["post_1"],
        post_2=content["post_2"],
        post_3=content.get("post_3"),
        hook_category=content["hook_category"],
        category=detected_category,
        username="@yourusername",
    )

    if result.success:
        dedup.record({
            "date": result.timestamp,
            "hook_category": result.hook_category,
            "hook_text": result.hook_text,
            "product": result.product,
            "affiliate_link": result.affiliate_link,
            "num_posts": result.num_posts,
            "category": detected_category,
        })
        db.mark_used(link, note=result.hook_category)
        print(f"✅ {detected_category}: {product[:40]}")
        return True
    else:
        print(f"❌ Failed: {result.error}")
        return False


def main():
    """Run the appropriate slot based on current time, or all slots."""
    import sys
    now_hhmm = time.strftime("%H:%M")
    target_slot = sys.argv[1] if len(sys.argv) > 1 else None

    if target_slot in SCHEDULE:
        post_for_slot(SCHEDULE[target_slot])
    elif target_slot == "now":
        # Run nearest slot
        nearest = min(SCHEDULE.keys(),
                      key=lambda s: abs(_to_min(s) - _to_min(now_hhmm)))
        post_for_slot(SCHEDULE[nearest])
    elif target_slot == "all":
        # Run all 3 slots back-to-back (with delays)
        for slot, category in SCHEDULE.items():
            print(f"\n--- Slot {slot} ({category}) ---")
            post_for_slot(category)
            time.sleep(60)  # 60s breathing room between slots
    else:
        print("Usage:")
        print("  python batch_schedule.py 08:00     # run specific slot")
        print("  python batch_schedule.py now       # run nearest scheduled slot")
        print("  python batch_schedule.py all       # run all 3 slots in sequence")
        print(f"\nConfigured slots: {list(SCHEDULE.keys())}")


def _to_min(hhmm: str) -> int:
    h, m = hhmm.split(":")
    return int(h) * 60 + int(m)


if __name__ == "__main__":
    main()
