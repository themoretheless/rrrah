# YouTube Deep-Search

**Re-find a video you lost in your own YouTube library** — even one you liked or
saved years ago and only half-remember. You connect your YouTube account with
**read-only** access, the app builds a private, local copy of all your playlists,
likes, channels and subscriptions, and then you search it with as many or as few
clues as you can remember: a few scrambled words from the title, a rough view
count, the channel's avatar, roughly how long the video was, a comment you left,
etc...

It runs entirely on your own machine. It never changes, deletes, likes, or
comments on anything in your account.

---

## Why it exists

YouTube's own search is poor at finding *your* old videos, and the "Liked
videos" list becomes an endless wall after a while. Worse, videos get deleted —
and once they're gone, even the title disappears. This tool keeps a durable local
copy of everything it sees (including a saved thumbnail and channel avatar), so
even if YouTube loses a video, **you still have enough to recognise it and hunt
it down elsewhere.**

---

## Connecting your YouTube account

- The app asks Google for **read** access to your YouTube data. Almost everything
  uses the strict read-only scope (`youtube.readonly`), which **cannot** modify
  anything. Reading **your own comments** is the one exception: YouTube has no
  read-only scope for comments, so the app must also request `youtube.force-ssl`.
  Google's consent screen describes that scope as letting an app edit or delete
  your videos and comments — but **this app only ever reads; it never writes,
  edits, or deletes anything.**
- **What it can see:** your playlists (public, unlisted *and* private), your
  Liked videos, your subscriptions, and your own comments.
- **What it can't see:** Watch Later and Watch History (YouTube no longer exposes
  these to any app), and other people's private data.
- You stay in control: a **Disconnect** button removes the stored access at any
  time. Your local catalog stays so you can keep searching.

Everything sits behind a single app password you choose, so nobody else on your
network can open it.

---

## How searching works

Open the app, unlock it, and you'll see a **Filters** panel beside a live results
grid. Results update as you type — combine any filters you like; a video must
match **all** of the ones you set. Leave the rest blank.

### Text — title, description, channel name
Type any **fragments** you remember, in **any order**. Matching is
case-insensitive, accent-insensitive, and **partial**: `mau` finds *Maudin
Malin*, `guy fam` finds *Family Guy*. Every fragment you type must appear
somewhere.

### Published date (with a "not sure" cursor)
Pick a date or a range. Not sure exactly when? Set **± days** to widen the window
in both directions, so being a few days (or months) off still finds it.

### Numbers, with tolerance — views, likes, comments, subscribers
Remember roughly how many views it had? Enter a number and a **tolerance %** —
e.g. `600000` ± `10%` matches anything from 540k to 660k. The same works for
likes, comment count, and the channel's subscriber count.
*(Subscriber counts are rounded by YouTube to 3 significant figures, so the app
automatically widens that filter to match — don't expect exact-to-the-digit.)*

### Duration
Enter the length as `HH:MM:SS` (e.g. `12:45` for twelve minutes forty-five, or
`1:30:00` for a stream), plus an optional tolerance %.

### Match by image — thumbnail or channel avatar
Have a copy of the thumbnail, or know the creator's avatar? **Paste a URL or
upload an image.** The app finds the closest matches among your saved thumbnails
/ avatars using a perceptual hash, so a different size or quality still matches.
The **strictness** slider controls how close a match must be (lower = stricter).

### Flags — subscribed, saved
- **Subscribed to channel:** only videos from channels you follow (or don't).
- **Saved in a playlist:** only videos you've put in one of your own playlists
  (a great way to shrink the haystack — *liked* alone doesn't count as saved).

### My comments
Remember commenting on it? Type a fragment of what you wrote (one per line), pick
**all / any**, and press **Scan comments**. The app checks YouTube for *your own*
comments on the videos that match your other filters, caches what it finds, and
narrows the results to videos you commented on. (Scanning is the most
quota-intensive action, so it runs only when you ask and only on the current
candidates.)

### Two helpful toggles
- **Include items with hidden/unknown numbers** (on by default): keeps videos
  whose creator hid view/like counts, rather than silently dropping them.
- **Include videos deleted from YouTube:** show the videos YouTube has lost — you
  still have their title, channel, stats and saved thumbnail.

---

## Your local copy = peace of mind

The first time you connect, the app downloads everything it can (this takes a
while and pauses automatically if it hits YouTube's daily limit, resuming the
next day). After that, searching is instant because it all runs against your
local copy — and it refreshes itself nightly.

Crucially, it also **saves the highest-quality thumbnail and the channel avatar**
for everything. If a video later gets deleted, it's flagged **deleted** but stays
fully searchable, with its saved image — so you can still recognise it and look
for a re-upload elsewhere.

---

## Limitations

YouTube's API simply doesn't expose some things, so this app is upfront about
them:

- **No dislikes / like-ratio.** YouTube made dislike counts private in 2021;
  they're not available for other people's videos. These filters don't exist.
- **"Member of a channel" isn't available** to a viewer, so the closest filter is
  **subscribed**.
- **Liked videos go back ~5,000 items.** YouTube only lets apps page through your
  most recent likes, not your entire lifetime of them.
- **"Published" date, not upload date.** YouTube exposes only the publish time.
- The **first crawl takes time and uses your daily quota** — it pauses and
  resumes on its own.

---

## Run it yourself

Everything is self-hosted with Docker. You'll need
[Docker](https://docs.docker.com/get-docker/) and a free Google Cloud project.

### 1. Get YouTube API credentials

1. Go to the [Google Cloud Console](https://console.cloud.google.com/) and create
   a project.
2. **APIs & Services → Library →** enable **YouTube Data API v3**.
3. **APIs & Services → Credentials → Create credentials → OAuth client ID →**
   *Web application*.
4. Add an **Authorized redirect URI**: `http://localhost:3000/api/auth/callback`
5. Copy the **Client ID** and **Client secret**.
   *(While your app is in "Testing", add your Google account as a test user.)*

### 2. Configure secrets

Secrets live in `secrets/`, split by concern. Copy each example and fill it in:

```bash
cp secrets/database/database.env.example secrets/database/database.env
cp secrets/cache/cache.env.example       secrets/cache/cache.env
cp secrets/storage/storage.env.example   secrets/storage/storage.env
cp secrets/app/app.env.example           secrets/app/app.env
```

In `secrets/app/app.env`, set your `GOOGLE_CLIENT_ID` / `GOOGLE_CLIENT_SECRET`,
choose a strong `APP_ENCRYPTION_KEY` (32+ characters) and an `APP_PASSWORD`, and
make sure the `DATABASE_URL` / `CACHE_URL` / `STORAGE_URL` credentials match the
other three files. (The `*_URL` host names — `database`, `cache`, `storage` —
match the compose service names; leave them as-is.)

### 3. Start it

```bash
docker compose up -d
```

Open **http://localhost:3000**, unlock with your `APP_PASSWORD`, click **Connect
YouTube**, approve the read-only access, then **Sync now**. The "Local mirror"
panel shows progress; you can start searching as soon as videos appear.

---

## Tech stack

Nuxt 4 (Vue + Nitro) · PostgreSQL (with `pg_trgm`, full-text search and
`pgvector`) · Valkey · SeaweedFS (S3-compatible) · Drizzle ORM · TypeScript
throughout. A standalone worker handles ingestion and image hashing so the web
app stays responsive.

## License & privacy

Open-source and self-hosted: your data never leaves your machine. No account, no
tracking, no third-party services beyond YouTube's own API.
