import { describe, expect, it } from 'vitest'
import { decryptSecret, encryptSecret, safeEqual } from '../server/lib/crypto'

describe('crypto', () => {
  it('round-trips a secret through encrypt/decrypt', () => {
    const secret = '1//refresh-token-value-with-symbols_-.~'
    const packed = encryptSecret(secret)
    expect(packed).not.toContain(secret)
    expect(decryptSecret(packed)).toBe(secret)
  })

  it('produces a different ciphertext each time (random nonce)', () => {
    expect(encryptSecret('same')).not.toBe(encryptSecret('same'))
  })

  it('fails to decrypt tampered ciphertext (GCM auth tag)', () => {
    const packed = encryptSecret('secret')
    const tampered = `${packed.slice(0, -2)}AA`
    expect(() => decryptSecret(tampered)).toThrow()
  })

  it('safeEqual compares constant-time by value', () => {
    expect(safeEqual('abc', 'abc')).toBe(true)
    expect(safeEqual('abc', 'abd')).toBe(false)
    expect(safeEqual('abc', 'abcd')).toBe(false)
  })
})
