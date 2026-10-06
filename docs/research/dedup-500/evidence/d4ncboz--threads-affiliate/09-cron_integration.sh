#!/bin/bash
# Cron integration for threads-affiliate
# Copy these lines to crontab via: crontab -e
#
# All times in server local timezone. Adjust if you want WIB (UTC+7).
# If server is UTC, subtract 7 hours from WIB schedule:
#   08:00 WIB → 01:00 UTC
#   13:00 WIB → 06:00 UTC
#   20:00 WIB → 13:00 UTC

REPO_PATH="/path/to/threads-affiliate"

# === Posting schedule (3x/day, different categories) ===

# 08:00 WIB → 01:00 UTC — skincare
0 1 * * * cd "$REPO_PATH" && python -m threads_poster.cli post-auto --category skincare >> logs/cron.log 2>&1

# 13:00 WIB → 06:00 UTC — parfum
0 6 * * * cd "$REPO_PATH" && python -m threads_poster.cli post-auto --category parfum >> logs/cron.log 2>&1

# 20:00 WIB → 13:00 UTC — haircare
0 13 * * * cd "$REPO_PATH" && python -m threads_poster.cli post-auto --category haircare >> logs/cron.log 2>&1


# === Cookie refresh (every 6 hours) ===
# Keeps session.json fresh from your Chrome (assumes Chrome stays logged in)

0 */6 * * * cd "$REPO_PATH" && python -m threads_poster.cli setup --extract-cookies >> logs/cron.log 2>&1


# === Database stats (daily report at 23:00 WIB / 16:00 UTC) ===
# Helps monitor when DB runs low on UNUSED links

0 16 * * * cd "$REPO_PATH" && python -m threads_poster.cli db --stats >> logs/db_stats.log 2>&1


# === Setup notes ===
#
# 1. Create logs directory before activating cron:
#      mkdir -p $REPO_PATH/logs
#
# 2. Set timezone in your Python environment (optional):
#      export TZ="Asia/Jakarta"
#
# 3. Verify cron is running:
#      crontab -l
#
# 4. Monitor:
#      tail -f $REPO_PATH/logs/cron.log
