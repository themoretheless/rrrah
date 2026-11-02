# Deduparr API Usage Examples

This document demonstrates how to use the duplicate detection and deletion features.

## Overview

The duplicate detection and deletion process works in three stages:
1. **Scan** - Detect duplicates and score files based on quality
2. **Preview** - Review which files would be kept vs deleted
3. **Delete** - Execute deletion (with dry-run support for safety)

## API Endpoints

### 1. Scan for Duplicates

Start a scan to detect duplicates in your Plex libraries:

```bash
curl -X POST "http://localhost:3001/api/scan/start" \
  -H "Content-Type: application/json" \
  -d '{
    "library_names": ["Movies", "TV Shows"]
  }'
```

**Response:**
```json
{
  "success": true,
  "message": "Scan completed for 2 libraries",
  "duplicates_found": 15,
  "sets_created": 8,
  "sets_already_exist": 2,
  "total_sets": 10
}
```

### 2. Get Duplicate Sets

Retrieve all detected duplicate sets:

```bash
curl "http://localhost:3001/api/scan/duplicates"
```

**Optional filters:**
- `?status=pending` - Filter by status (pending, approved, rejected, processed)
- `?media_type=movie` - Filter by media type (movie, episode)
- `?limit=50` - Limit results (default: 100, max: 1000)

**Response:**
```json
[
  {
    "id": 1,
    "plex_item_id": "12345",
    "title": "The Matrix",
    "media_type": "movie",
    "found_at": "2025-11-07T10:30:00Z",
    "status": "pending",
    "space_to_reclaim": 5368709120,
    "files": [
      {
        "id": 1,
        "file_path": "/movies/Matrix.720p.mkv",
        "file_size": 2147483648,
        "score": 15000,
        "keep": false,
        "file_metadata": {
          "resolution": "720p",
          "video_codec": "h264",
          "audio_codec": "aac"
        }
      },
      {
        "id": 2,
        "file_path": "/movies/Matrix.1080p.BluRay.REMUX.mkv",
        "file_size": 26843545600,
        "score": 35000,
        "keep": true,
        "file_metadata": {
          "resolution": "1080p",
          "video_codec": "h264",
          "audio_codec": "truehd"
        }
      }
    ]
  }
]
```

### 3. Preview Deletion

Preview what would be deleted for a specific duplicate set:

```bash
curl "http://localhost:3001/api/scan/duplicates/1/preview"
```

**Response:**
```json
{
  "set_id": 1,
  "title": "The Matrix",
  "media_type": "movie",
  "status": "pending",
  "files_to_keep": [
    {
      "id": 2,
      "file_path": "/movies/Matrix.1080p.BluRay.REMUX.mkv",
      "file_size": 26843545600,
      "score": 35000,
      "metadata": {
        "resolution": "1080p",
        "video_codec": "h264",
        "audio_codec": "truehd"
      }
    }
  ],
  "files_to_delete": [
    {
      "id": 1,
      "file_path": "/movies/Matrix.720p.mkv",
      "file_size": 2147483648,
      "score": 15000,
      "metadata": {
        "resolution": "720p",
        "video_codec": "h264",
        "audio_codec": "aac"
      }
    }
  ],
  "total_files": 2,
  "files_to_delete_count": 1,
  "space_to_reclaim": 2147483648,
  "space_to_reclaim_mb": 2048.0,
  "space_to_reclaim_gb": 2.0
}
```

### 4. Delete Duplicates (Dry Run)

**SAFETY FIRST:** Always test with `dry_run: true` (default) before actual deletion:

```bash
curl -X POST "http://localhost:3001/api/scan/duplicates/1/delete" \
  -H "Content-Type: application/json" \
  -d '{
    "dry_run": true
  }'
```

**Response:**
```json
{
  "success": true,
  "message": "[DRY-RUN] Would delete 1 file(s) from duplicate set 'The Matrix'",
  "dry_run": true,
  "files_deleted": 1,
  "space_reclaimed": 2147483648,
  "errors": []
}
```

### 5. Delete Duplicates (Actual Deletion)

⚠️ **WARNING:** This will permanently delete files!

```bash
curl -X POST "http://localhost:3001/api/scan/duplicates/1/delete" \
  -H "Content-Type: application/json" \
  -d '{
    "dry_run": false
  }'
```

**Response:**
```json
{
  "success": true,
  "message": "Deleted 1 file(s) from duplicate set 'The Matrix'",
  "dry_run": false,
  "files_deleted": 1,
  "space_reclaimed": 2147483648,
  "errors": []
}
```

## How File Selection Works

The scoring engine automatically selects the best quality file to keep based on:

1. **Resolution** - Higher is better (4K > 1080p > 720p > 480p)
2. **Bitrate** - Higher is better
3. **Video Codec** - Better codecs score higher
4. **Audio Codec** - Better codecs score higher (TrueHD/DTS-HD > DTS/AC3 > AAC)
5. **File Size** - Larger files within same resolution score higher
6. **Custom Rules** - Regex patterns for filename matching (e.g., prefer "REMUX" over "WEB-DL")

### Testing File Selection

The test `test_duplicate_detection_accuracy` verifies that:
- The highest quality file is marked as `keep: true`
- All lower quality files are marked as `keep: false`
- Scores are ordered correctly (higher quality = higher score)
- Custom scoring rules are applied (e.g., REMUX boost)

**Example from test:**
```
File                                 Score   Keep    Why
-------------------------------------------- ------- ------
QualityTest.1080p.BluRay.REMUX.mkv  35000   ✓       Highest score (1080p + high bitrate + REMUX rule)
QualityTest.1080p.WEB-DL.mkv        25000   ✗       Medium quality
QualityTest.720p.HDTV.mkv           15000   ✗       Lowest quality
```

## Deletion Pipeline Stages

When deletion is executed (non-dry-run), the following stages occur:

1. **qBittorrent Removal** - Remove item (if file is in library)
   - Skipped if qBittorrent is not configured
2. **\*arr Removal** - Remove from Radarr/Sonarr
   - Skipped if Radarr/Sonarr is not configured
3. **Disk Deletion** - Delete physical file
4. **Plex Refresh** - Trigger Plex library scan to update metadata

Each stage is tracked in the database and can be rolled back if errors occur.

## Error Handling

The deletion endpoint returns errors in the response:

```json
{
  "success": false,
  "message": "Deleted 1 file(s) from duplicate set 'The Matrix'",
  "dry_run": false,
  "files_deleted": 1,
  "space_reclaimed": 2147483648,
  "errors": [
    "Failed to delete /movies/SomeFile.mkv: File not found on disk"
  ]
}
```

## Common Status Codes

- `200` - Success
- `400` - Bad request (invalid parameters, already processed, etc.)
- `404` - Duplicate set not found
- `500` - Server error

## Best Practices

1. **Always preview first** - Use the `/preview` endpoint to see what would be deleted
2. **Test with dry-run** - Use `dry_run: true` before actual deletion
3. **Review scores** - Check that the scoring logic selected the right files to keep
4. **Configure custom rules** - Add scoring rules for specific patterns you prefer (REMUX, BluRay, etc.)
5. **Monitor space reclaimed** - Check `space_to_reclaim` values match expectations
6. **Check status after deletion** - Verify duplicate set status changes to "processed"
