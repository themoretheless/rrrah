import type { Buffer } from 'node:buffer'
import { randomUUID } from 'node:crypto'
import { createError } from 'h3'
import { Redis } from 'ioredis'
import { useCache } from '../cache'
import { useEnv } from '../env'

const REQ_KEY = 'img:hash:req'

// Delegates perceptual hashing to the worker (which runs sharp in its main
// thread), keeping the native module out of the web server. Guarantees the
// query image is hashed with the exact same algorithm as the stored thumbnails.
export async function hashViaWorker(bytes: Buffer, timeoutSec = 12): Promise<string> {
  const id = randomUUID()
  await useCache().lpush(REQ_KEY, JSON.stringify({ id, b64: bytes.toString('base64') }))

  // Dedicated connection for the blocking wait.
  const conn = new Redis(useEnv().CACHE_URL, { maxRetriesPerRequest: null })
  try {
    const res = await conn.blpop(`img:hash:done:${id}`, timeoutSec)
    if (!res) throw createError({ statusCode: 504, statusMessage: 'Image hashing timed out (is the worker running?)' })
    if (res[1] === 'ERROR') throw createError({ statusCode: 422, statusMessage: 'Could not hash that image' })
    return res[1]
  }
  finally {
    conn.disconnect()
  }
}
