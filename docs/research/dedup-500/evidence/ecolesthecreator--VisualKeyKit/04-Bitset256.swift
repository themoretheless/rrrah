import Foundation

/// A compact 256-bit biometric fingerprint representation.
///
/// Used to store perceptual hash signatures from visual textures. The 256 bits are packed
/// into four 64-bit words for efficient storage and Hamming distance computation.
///
/// **Properties:**
/// - Codable: Can be serialized for storage
/// - Hashable: Can be used as dictionary keys
/// - Equatable: Supports direct comparison
public struct Bitset256: Codable, Hashable, Sendable {
    /// Four 64-bit words storing the 256 bits (4 × 64 = 256)
    public var words: (UInt64, UInt64, UInt64, UInt64)

    /// Creates a bitset from four 64-bit words.
    public init(_ w0: UInt64 = 0, _ w1: UInt64 = 0, _ w2: UInt64 = 0, _ w3: UInt64 = 0) {
        self.words = (w0, w1, w2, w3)
    }

    // MARK: - Codable

    public init(from decoder: Decoder) throws {
        var container = try decoder.unkeyedContainer()
        let w0 = try container.decode(UInt64.self)
        let w1 = try container.decode(UInt64.self)
        let w2 = try container.decode(UInt64.self)
        let w3 = try container.decode(UInt64.self)
        self.words = (w0, w1, w2, w3)
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.unkeyedContainer()
        try container.encode(words.0)
        try container.encode(words.1)
        try container.encode(words.2)
        try container.encode(words.3)
    }

    // MARK: - Equatable

    public static func == (lhs: Bitset256, rhs: Bitset256) -> Bool {
        lhs.words.0 == rhs.words.0 &&
        lhs.words.1 == rhs.words.1 &&
        lhs.words.2 == rhs.words.2 &&
        lhs.words.3 == rhs.words.3
    }

    // MARK: - Hashable

    public func hash(into hasher: inout Hasher) {
        hasher.combine(words.0)
        hasher.combine(words.1)
        hasher.combine(words.2)
        hasher.combine(words.3)
    }

    // MARK: - Bit Operations

    /// Creates a bitset from an array of 256 boolean values.
    ///
    /// - Parameter bits: Array of exactly 256 boolean values (true = 1, false = 0)
    /// - Returns: Packed bitset representation
    public static func fromBits(_ bits: [Bool]) -> Bitset256 {
        precondition(bits.count == 256, "Bitset256 requires exactly 256 bits")
        var w0: UInt64 = 0, w1: UInt64 = 0, w2: UInt64 = 0, w3: UInt64 = 0
        for i in 0..<256 where bits[i] {
            let wi = i / 64, bi = i % 64
            switch wi {
            case 0: w0 |= (UInt64(1) << UInt64(bi))
            case 1: w1 |= (UInt64(1) << UInt64(bi))
            case 2: w2 |= (UInt64(1) << UInt64(bi))
            default: w3 |= (UInt64(1) << UInt64(bi))
            }
        }
        return Bitset256(w0, w1, w2, w3)
    }

    /// Gets the bit value at a specific position.
    ///
    /// - Parameter i: Bit index (0-255)
    /// - Returns: true if bit is set, false otherwise
    public func bit(at i: Int) -> Bool {
        let wi = i / 64, bi = i % 64
        switch wi {
        case 0: return ((words.0 >> UInt64(bi)) & 1) == 1
        case 1: return ((words.1 >> UInt64(bi)) & 1) == 1
        case 2: return ((words.2 >> UInt64(bi)) & 1) == 1
        default: return ((words.3 >> UInt64(bi)) & 1) == 1
        }
    }

    /// XOR operator for bitsets.
    public static func ^ (lhs: Bitset256, rhs: Bitset256) -> Bitset256 {
        .init(lhs.words.0 ^ rhs.words.0,
              lhs.words.1 ^ rhs.words.1,
              lhs.words.2 ^ rhs.words.2,
              lhs.words.3 ^ rhs.words.3)
    }

    /// Computes Hamming distance (number of differing bits) between two bitsets.
    ///
    /// This is the core matching metric for fuzzy biometric authentication.
    /// Lower Hamming distance = more similar fingerprints.
    ///
    /// **Typical distances:**
    /// - Same object, same conditions: 0-10 bits
    /// - Same object, different lighting/angle: 10-35 bits
    /// - Different objects: 100-150 bits (random = ~128)
    ///
    /// - Parameter other: Bitset to compare against
    /// - Returns: Number of differing bits (0-256)
    public func hammingDistance(_ other: Bitset256) -> Int {
        (words.0 ^ other.words.0).nonzeroBitCount +
        (words.1 ^ other.words.1).nonzeroBitCount +
        (words.2 ^ other.words.2).nonzeroBitCount +
        (words.3 ^ other.words.3).nonzeroBitCount
    }
}
