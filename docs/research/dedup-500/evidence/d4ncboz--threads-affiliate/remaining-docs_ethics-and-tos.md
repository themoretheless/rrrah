# Ethics & Terms of Service

This tool is provided for **educational and personal-use** purposes. By using it, you accept full responsibility for compliance with platform terms and local law.

## Threads / Instagram (Meta) ToS

Threads' [Terms of Use](https://help.instagram.com/581066165581870) prohibit:

> "You can't impersonate others or provide inaccurate information."  
> "You can't do anything to interfere with or impair the intended operation of the Service."  
> "You can't attempt to create accounts or access or collect information in unauthorized ways. This includes creating accounts or collecting information in an automated way without our express permission."

### Where this tool stands:

| Action | Risk Level |
|--------|-----------|
| Manual posting | ✅ Allowed |
| Scheduled posts (e.g. Buffer, Later) | 🟡 Grey area — Meta allows some scheduling tools via API but not unofficial automation |
| **This tool (Playwright + cookie auth)** | 🟡 Grey area — Mimics human action but uses session cookies, not official API |
| Botting/spamming (10+ posts/day) | 🔴 Will get account banned |

### Mitigation in this tool:

- **Rate limiting:** Built-in delays between posts (no burst posting)
- **Human-like typing:** 30ms delay per char, mimics real user
- **Rotation:** Categories + hooks rotate to avoid spam fingerprint
- **Limit:** Recommended max 3 posts/day (not 30)
- **Warm-up requirement:** New accounts must post manually first 1-2 weeks

### Account Ban Risk

Despite the mitigations, **Threads can ban your account at any time** if their algorithms classify behavior as automated. To minimize risk:

1. Use the tool sparingly (3-5 posts/day max)
2. Mix automated posts with manual posts
3. Engage manually (like, reply, follow) outside the tool
4. Never use the tool on a primary personal account — use a niche-dedicated account

---

## Shopee Affiliate Program ToS

[Shopee Affiliate Terms](https://affiliate.shopee.co.id/terms) require:

> "Affiliates must disclose their relationship with Shopee in all promotional content."  
> "Affiliates must not engage in misleading or fraudulent practices."  
> "Affiliates must comply with all applicable laws and regulations."

### Mandatory Disclosures

Include in your posts or bio:

- `#ShopeeAffiliate`
- `#PromosiShopee` 
- Or explicit text: "Aku dapet komisi kalau lo beli lewat link ini"

### Prohibited Content

- ❌ Fake reviews (claiming you used a product you haven't)
- ❌ False before/after claims
- ❌ Health claims for non-medical products (e.g. "cures acne")
- ❌ Misleading prices ("Rp 50rb" when actual is Rp 100rb)
- ❌ Reselling Shopee products at higher price via your own page

### Tax Obligations

In Indonesia, affiliate income is taxable:
- Earnings <Rp 60 juta/year → personal income tax bracket (PPh 21)
- Earnings >Rp 4.8 miliar/year → must register as PKP
- Track your earnings monthly, report in annual SPT

---

## Indonesian Law

### UU PDP (Personal Data Protection Law, 2022)

- Don't scrape user data from Threads for marketing
- Don't store other users' personal info without consent
- Your affiliate database should only contain product/link info, not user data

### UU ITE (Electronic Information Law)

- Defamation in posts → criminal liability (Pasal 27)
- False testimonials → fraud (Pasal 28)
- Use real product photos (not stolen from another influencer)

### KOMINFO AI Disclosure (2025+)

If you use AI-generated content (images, videos, voice):
- Add `#AIGenerated` or `Konten dihasilkan AI`
- Avoid impersonating real people
- Don't claim AI avatars as your own face

---

## Responsible Usage Checklist

Before each batch of posts, verify:

- [ ] Account is at least 2 weeks old, with manual posts
- [ ] You're not posting more than 3/day
- [ ] Each post has unique hook (not copy-paste)
- [ ] Product claims are truthful (you'd recommend it to a friend)
- [ ] Affiliate disclosure is in bio AND in at least 1 post weekly
- [ ] Image used is real product (not AI-generated as "real")
- [ ] Hashtags include `#ShopeeAffiliate` or equivalent

---

## What This Tool Does NOT Do

To stay on the ethical side:

- ❌ Does NOT scrape user data
- ❌ Does NOT auto-follow/unfollow others
- ❌ Does NOT auto-DM users
- ❌ Does NOT bypass 2FA (uses cookies from your authenticated session)
- ❌ Does NOT create fake accounts
- ❌ Does NOT use proxies to evade rate limits

It **only** posts content that you (the human) configured to your own account.

---

## Liability Disclaimer

THE AUTHORS PROVIDE THIS SOFTWARE "AS IS" WITH NO WARRANTY OR GUARANTEE.

- Users assume all risk of account suspension by Meta/Threads
- Users assume all risk of affiliate program termination by Shopee
- Users are responsible for tax obligations on affiliate income
- Users are responsible for compliance with local laws
- The authors are not affiliated with Meta, Threads, Instagram, or Shopee

By using this software, you acknowledge that you have read and understood these terms.

---

## Reporting Abuse

If you discover this tool being used for:
- Mass spam
- Coordinated inauthentic behavior  
- Fraudulent affiliate schemes
- Harassment

Please open an issue on GitHub with details. The authors reserve the right to refuse PRs or support to users misusing the tool.
