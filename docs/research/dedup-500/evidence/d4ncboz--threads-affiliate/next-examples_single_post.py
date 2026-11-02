"""Single-post example — most basic usage."""
from threads_poster import ThreadsPoster, ContentGenerator, DedupChecker


def main():
    # 1. Generate content from templates
    gen = ContentGenerator(templates_dir="templates")
    content = gen.generate_chain(
        product="SKINTIFIC 5X Ceramide Moisturizer",
        affiliate_link="https://s.shopee.co.id/XXXXX",
        category="skincare",
        hook_style="edukasi",
        num_posts=3,
    )

    print(f"Hook: {content['post_1']}")
    print(f"Body: {content['post_2']}")
    print(f"CTA:  {content['post_3']}")

    # 2. Dedup check
    dedup = DedupChecker("data/post_history.json")
    ok, reason = dedup.check(
        affiliate_link=content["affiliate_link"],
        hook_category=content["hook_category"],
        hook_text=content["hook_text"],
        category="skincare",
    )
    if not ok:
        print(f"Skipped: {reason}")
        return

    # 3. Post
    poster = ThreadsPoster(
        cookies_path="~/.threads_poster/cookies/session.json",
        headless=True,
    )
    result = poster.post(
        product=content["product_name"],
        affiliate_link=content["affiliate_link"],
        hook_text=content["post_1"],
        post_2=content["post_2"],
        post_3=content["post_3"],
        hook_category=content["hook_category"],
        category="skincare",
        image_path="data/product_images/skintific.jpg",  # optional
        username="@yourusername",  # for verification
    )

    if result.success:
        print(f"✅ Posted at {result.post_url}")
        dedup.record({
            "date": result.timestamp,
            "hook_category": result.hook_category,
            "hook_text": result.hook_text,
            "product": result.product,
            "affiliate_link": result.affiliate_link,
            "num_posts": result.num_posts,
            "category": "skincare",
        })
    else:
        print(f"❌ Failed: {result.error}")


if __name__ == "__main__":
    main()
