import { and, eq, inArray, isNull, sql } from 'drizzle-orm'
import { videos } from '../database/schema'
import { useDb } from '../lib/db'
import { scanVideoComments } from '../lib/ingest/comments'
import { searchVideos } from '../lib/search/buildQuery'
import { FilterSpecSchema } from '../lib/search/filterSpec'
import { QuotaParkedError } from '../lib/youtube/errors'
import { getConnectionStatus } from '../lib/youtube/oauth'

// Hybrid comment scan: take the candidate set from the OTHER filters and scan a
// bounded number of not-yet-scanned videos for the user's own comments. The UI
// then re-runs the search (with ownComments) against the freshly-cached data.
const MAX_SCAN = 20

export default defineEventHandler(async (event) => {
  const spec = await readValidatedBody(event, body => FilterSpecSchema.parse(body))

  const status = await getConnectionStatus()
  if (!status.connected || !status.channelId) {
    throw createError({ statusCode: 409, statusMessage: 'YouTube account not connected' })
  }

  // Candidates narrowed by every OTHER filter.
  const { results } = await searchVideos({ ...spec, ownComments: undefined, limit: 200, offset: 0 })
  const candidateIds = results.map(r => r.id)
  if (candidateIds.length === 0) {
    return { candidates: 0, scanned: 0, found: 0, remaining: 0, parked: false }
  }

  const db = useDb()
  const unscanned = await db.select({ id: videos.id }).from(videos)
    .where(and(inArray(videos.id, candidateIds), isNull(videos.commentsScannedAt), eq(videos.commentsDisabled, false)))
    .limit(MAX_SCAN)

  let scanned = 0
  let found = 0
  let parked = false
  for (const row of unscanned) {
    try {
      found += await scanVideoComments(row.id, status.channelId)
      scanned++
    }
    catch (error) {
      if (error instanceof QuotaParkedError) {
        parked = true
        break
      }
      throw error
    }
  }

  const [remaining] = await db.select({ n: sql<number>`count(*)::int` }).from(videos)
    .where(and(inArray(videos.id, candidateIds), isNull(videos.commentsScannedAt), eq(videos.commentsDisabled, false)))

  return { candidates: candidateIds.length, scanned, found, remaining: remaining?.n ?? 0, parked }
})
