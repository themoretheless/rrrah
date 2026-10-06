import Foundation
import CryptoKit

/// Deterministic password generation from biometric templates.
///
/// Converts cryptographic seeds (derived from visual keys) into random-looking passwords
/// that conform to user-specified requirements. The same inputs always produce the same
/// password, enabling "password recovery" by re-capturing the same visual key.
///
/// **Security Properties:**
/// - Deterministic: Same seed + policy → same password (critical for recovery)
/// - High entropy: Passwords appear random with uniform character distribution
/// - One-way: Cannot recover seed from password (uses HKDF key derivation)
/// - Device-bound: Requires device secret + biometric template
public enum BiometricPassword {
    /// Derives a cryptographic seed from a biometric template using HMAC-SHA256.
    ///
    /// This is the key derivation function that converts a fuzzy biometric template into
    /// a deterministic cryptographic seed. The same inputs always produce the same seed.
    ///
    /// - Parameters:
    ///   - deviceSecret: 32-byte device-specific secret (stored in keychain, never leaves device)
    ///   - salt: Random salt generated during enrollment (stored with enrollment data)
    ///   - template: The 256-bit biometric template from enrollment or matching
    /// - Returns: 32-byte cryptographic seed suitable for password generation
    ///
    /// **Security Properties:**
    /// - Deterministic: Same inputs always produce same output
    /// - One-way: Cannot recover template or device secret from seed
    /// - Device-bound: Seed requires both the template AND the device secret
    /// - Salted: Different enrollments of same object produce different seeds
    ///
    /// **Format:** HMAC-SHA256(deviceSecret, salt || template)
    public static func deriveSeed(
        deviceSecret: Data,
        salt: Data,
        template: Bitset256
    ) -> Data {
        var body = Data()
        body.append(salt)
        withUnsafeBytes(of: template.words) { body.append(contentsOf: $0) }

        let key = SymmetricKey(data: deviceSecret)
        let mac = HMAC<SHA256>.authenticationCode(for: body, using: key)
        return Data(mac)
    }

    /// Generates a deterministic password from a cryptographic seed.
    ///
    /// Uses HKDF-SHA256 to expand the seed into a deterministic pseudorandom stream,
    /// then applies rejection sampling to ensure uniform character distribution.
    ///
    /// - Parameters:
    ///   - seed: 32-byte cryptographic seed from `deriveSeed()`
    ///   - policy: Password policy specifying length and allowed character classes
    /// - Returns: Generated password string conforming to policy
    ///
    /// **Algorithm:**
    /// 1. Use HKDF-SHA256 to expand seed into deterministic pseudorandom byte stream
    /// 2. If mustIncludeEachClass, reserve first N positions for one char from each class
    /// 3. Fill remaining positions with rejection sampling from full alphabet
    /// 4. Fisher-Yates shuffle to mix required characters throughout password
    ///
    /// **Rejection sampling:**
    /// To avoid modulo bias, we only accept random bytes that fall in a "fair range".
    /// For alphabet size M, fair range is [0, (256/M)*M). Bytes outside this range
    /// are rejected and the next byte is tried. This ensures truly uniform distribution.
    ///
    /// **Example:**
    /// ```swift
    /// let seed = BiometricPassword.deriveSeed(
    ///     deviceSecret: secret,
    ///     salt: enrollment.salt,
    ///     template: enrollment.template
    /// )
    /// let password = BiometricPassword.generate(from: seed, policy: .recommended)
    /// // password: "7Kx!mP3nQ@zL9wRt" (deterministic, appears random)
    /// ```
    public static func generate(from seed: Data, policy: PasswordPolicy) -> String {
        precondition(policy.length > 0, "Password length must be > 0")

        var classes = [[Character]]()
        let lowers = Array("abcdefghijklmnopqrstuvwxyz")
        let uppers = Array("ABCDEFGHIJKLMNOPQRSTUVWXYZ")
        let digits = Array("0123456789")
        let specials = Array("!@#$%^*-_=+?")

        if policy.allowLowercase { classes.append(lowers) }
        if policy.allowUppercase { classes.append(uppers) }
        if policy.allowDigits { classes.append(digits) }
        if policy.allowSpecialCharacters { classes.append(specials) }

        let alphabet = classes.flatMap { $0 }
        precondition(!alphabet.isEmpty, "At least one character class must be enabled")

        // Deterministic pseudorandom stream via HKDF-SHA256
        let okm = HKDF<SHA256>.deriveKey(
            inputKeyMaterial: SymmetricKey(data: seed),
            info: Data("pw-stream".utf8),
            outputByteCount: max(policy.length * 4, 64)
        )

        var bytes = Data()
        okm.withUnsafeBytes { ptr in
            bytes.append(contentsOf: ptr)
        }

        /// Selects the next character from a character set using rejection sampling.
        func nextChar(_ idx: inout Int, from set: [Character]) -> Character {
            let m = set.count
            let bound = (256 / m) * m  // Fair range: largest multiple of m below 256

            while idx < bytes.count {
                let b = Int(bytes[idx]); idx += 1
                if b < bound { return set[b % m] }
            }

            // Exhausted byte stream (rare) - extend deterministically
            let digest = SHA256.hash(data: bytes)
            bytes.append(contentsOf: digest)
            return nextChar(&idx, from: set)
        }

        var i = 0
        var out = Array(repeating: Character("•"), count: policy.length)

        // Step 1: If required, fill first N positions with one char from each class
        if policy.mustIncludeEachClass {
            for pos in 0..<min(classes.count, policy.length) {
                out[pos] = nextChar(&i, from: classes[pos])
            }
        }

        // Step 2: Fill remaining positions from full alphabet
        for pos in 0..<policy.length where out[pos] == "•" {
            out[pos] = nextChar(&i, from: alphabet)
        }

        // Step 3: Deterministic Fisher-Yates shuffle
        var j = 0
        for k in stride(from: out.count - 1, through: 1, by: -1) {
            let b = Int(bytes[j]); j = (j + 1) % bytes.count
            out.swapAt(k, b % (k + 1))
        }

        return String(out)
    }
}
