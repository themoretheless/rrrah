import XCTest
@testable import VisualKeyKit

final class VisualKeyKitTests: XCTestCase {

    // MARK: - Bitset256 Tests

    func testBitset256Creation() {
        let bits = Array(repeating: true, count: 128) + Array(repeating: false, count: 128)
        let bitset = Bitset256.fromBits(bits)

        for i in 0..<128 {
            XCTAssertTrue(bitset.bit(at: i), "Bit \(i) should be true")
        }
        for i in 128..<256 {
            XCTAssertFalse(bitset.bit(at: i), "Bit \(i) should be false")
        }
    }

    func testHammingDistance() {
        let all1 = Bitset256.fromBits(Array(repeating: true, count: 256))
        let all0 = Bitset256.fromBits(Array(repeating: false, count: 256))

        XCTAssertEqual(all1.hammingDistance(all0), 256, "All bits differ")
        XCTAssertEqual(all1.hammingDistance(all1), 0, "Identical bitsets")

        let half = Bitset256.fromBits(
            Array(repeating: true, count: 128) + Array(repeating: false, count: 128)
        )
        XCTAssertEqual(all1.hammingDistance(half), 128, "Half the bits differ")
    }

    // MARK: - FuzzyTemplate Tests

    func testTemplateBuilding() {
        // Create 3 samples where bit 0 is [true, true, false] → majority = true
        let sample1 = Bitset256.fromBits([true] + Array(repeating: false, count: 255))
        let sample2 = Bitset256.fromBits([true] + Array(repeating: false, count: 255))
        let sample3 = Bitset256.fromBits([false] + Array(repeating: false, count: 255))

        let (template, confidence) = FuzzyTemplate.build(from: [sample1, sample2, sample3])

        XCTAssertTrue(template.bit(at: 0), "Bit 0 should be true (2 out of 3 voted true)")
        XCTAssertEqual(confidence[0], 2, "Confidence should be 2 for bit 0")
        XCTAssertEqual(confidence[1], 0, "Confidence should be 0 for bit 1")
    }

    func testFuzzyMatching() {
        let template = Bitset256.fromBits(Array(repeating: true, count: 256))

        // Flip 10 bits
        var bits = Array(repeating: true, count: 256)
        for i in 0..<10 { bits[i] = false }
        let sample = Bitset256.fromBits(bits)

        let distance = FuzzyTemplate.match(template: template, sample: sample)
        XCTAssertEqual(distance, 10, "Should have 10 bits different")
    }

    // MARK: - BiometricPassword Tests

    func testPasswordDeterminism() {
        let deviceSecret = SecureRandom.bytes(32)
        let salt = SecureRandom.bytes(16)
        let template = Bitset256.fromBits(Array(repeating: true, count: 256))

        let seed1 = BiometricPassword.deriveSeed(
            deviceSecret: deviceSecret,
            salt: salt,
            template: template
        )
        let seed2 = BiometricPassword.deriveSeed(
            deviceSecret: deviceSecret,
            salt: salt,
            template: template
        )

        XCTAssertEqual(seed1, seed2, "Same inputs should produce same seed")

        let password1 = BiometricPassword.generate(from: seed1, policy: .recommended)
        let password2 = BiometricPassword.generate(from: seed2, policy: .recommended)

        XCTAssertEqual(password1, password2, "Same seed should produce same password")
    }

    func testPasswordPolicyCompliance() {
        let seed = SecureRandom.bytes(32)

        // Test length
        let policy1 = PasswordPolicy(
            length: 16,
            allowUppercase: true,
            allowLowercase: true,
            allowDigits: true,
            allowSpecialCharacters: true,
            mustIncludeEachClass: false
        )
        let password1 = BiometricPassword.generate(from: seed, policy: policy1)
        XCTAssertEqual(password1.count, 16, "Password should be 16 characters")

        // Test character class inclusion
        let policy2 = PasswordPolicy(
            length: 20,
            allowUppercase: true,
            allowLowercase: true,
            allowDigits: true,
            allowSpecialCharacters: true,
            mustIncludeEachClass: true
        )
        let password2 = BiometricPassword.generate(from: seed, policy: policy2)

        let hasUpper = password2.contains(where: { $0.isUppercase })
        let hasLower = password2.contains(where: { $0.isLowercase })
        let hasDigit = password2.contains(where: { $0.isNumber })
        let hasSpecial = password2.contains(where: { "!@#$%^*-_=+?".contains($0) })

        XCTAssertTrue(hasUpper, "Password should contain uppercase")
        XCTAssertTrue(hasLower, "Password should contain lowercase")
        XCTAssertTrue(hasDigit, "Password should contain digit")
        XCTAssertTrue(hasSpecial, "Password should contain special character")
    }

    func testDifferentSaltsDifferentPasswords() {
        let deviceSecret = SecureRandom.bytes(32)
        let template = Bitset256.fromBits(Array(repeating: true, count: 256))

        let salt1 = SecureRandom.bytes(16)
        let salt2 = SecureRandom.bytes(16)

        let seed1 = BiometricPassword.deriveSeed(
            deviceSecret: deviceSecret,
            salt: salt1,
            template: template
        )
        let seed2 = BiometricPassword.deriveSeed(
            deviceSecret: deviceSecret,
            salt: salt2,
            template: template
        )

        XCTAssertNotEqual(seed1, seed2, "Different salts should produce different seeds")

        let password1 = BiometricPassword.generate(from: seed1, policy: .recommended)
        let password2 = BiometricPassword.generate(from: seed2, policy: .recommended)

        XCTAssertNotEqual(password1, password2, "Different salts should produce different passwords")
    }

    // MARK: - SecureRandom Tests

    func testSecureRandomUniqueness() {
        let bytes1 = SecureRandom.bytes(32)
        let bytes2 = SecureRandom.bytes(32)

        XCTAssertNotEqual(bytes1, bytes2, "Random bytes should be different")
        XCTAssertEqual(bytes1.count, 32, "Should generate 32 bytes")
    }

    // MARK: - PasswordPolicy Tests

    func testPasswordPolicyPresets() {
        XCTAssertEqual(PasswordPolicy.recommended.length, 20)
        XCTAssertEqual(PasswordPolicy.highSecurity.length, 32)
        XCTAssertEqual(PasswordPolicy.alphanumeric.length, 16)
        XCTAssertFalse(PasswordPolicy.alphanumeric.allowSpecialCharacters)
    }
}
