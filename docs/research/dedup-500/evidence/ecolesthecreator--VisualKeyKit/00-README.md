# VisualKeyKit

**Transform physical objects into cryptographic password generators.**

VisualKeyKit is a Swift package that implements fuzzy biometric authentication using perceptual image hashing. Turn everyday objects (watch faces, wallets, book covers) into password generators with zero storage of passwords or images.

## Features

- 🔐 **Zero-knowledge security** - No passwords, images, or service names stored
- 🎯 **Fuzzy matching** - Tolerates lighting, angle, and focus variations
- 🔑 **Deterministic generation** - Same object → same password (enables recovery)
- 🛡️ **Device-bound** - Passwords tied to device secret (never transmitted)
- ⚡ **Pure Swift** - No external dependencies, uses Apple's Accelerate & CryptoKit

## Installation

### Swift Package Manager

Add to your `Package.swift`:

```swift
dependencies: [
    .package(url: "https://github.com/ecolesthecreator/VisualKeyKit.git", from: "1.0.0")
]
```

Or in Xcode: **File → Add Packages → Enter repository URL**

## Quick Start

```swift
import VisualKeyKit

// 1. Enrollment: Capture 3-5 samples of the same object
let samples = [
    VisualKey.computeSignature(from: image1),
    VisualKey.computeSignature(from: image2),
    VisualKey.computeSignature(from: image3)
]

let (template, confidence) = FuzzyTemplate.build(from: samples)

// Store these (NOT the images!)
let salt = SecureRandom.bytes(16)
let deviceSecret = SecureRandom.bytes(32)  // Store in keychain

// 2. Password Generation: Re-capture the same object
let newSample = VisualKey.computeSignature(from: liveImage)
let distance = FuzzyTemplate.match(template: template, sample: newSample)

if distance <= 35 {  // Recommended threshold
    let seed = BiometricPassword.deriveSeed(
        deviceSecret: deviceSecret,
        salt: salt,
        template: template
    )
    let password = BiometricPassword.generate(from: seed, policy: .recommended)
    print("Password: \(password)")  // e.g., "Kx7!mP3nQ@zL9wRt2Vs"
}
```

## Core Components

### 1. VisualKey - Perceptual Hashing

Converts images into 256-bit fingerprints that are stable across capture variations:

```swift
let signature = VisualKey.computeSignature(from: cgImage)
// Returns: Bitset256 (256-bit fingerprint)
```

**Algorithm**: Combines 4 complementary hashes (64 bits each)
- **aHash**: Overall brightness distribution
- **dHashH**: Horizontal gradients (texture patterns)
- **dHashV**: Vertical gradients (texture patterns)
- **pHash**: Frequency domain via DCT (scale/rotation resistant)

### 2. FuzzyTemplate - Template Building & Matching

Creates stable templates from multiple noisy samples:

```swift
let (template, confidence) = FuzzyTemplate.build(from: samples)
let distance = FuzzyTemplate.match(template: enrolled, sample: captured)
```

**Typical Hamming distances:**
- 0-20 bits: Same object, same conditions
- 20-35 bits: Same object, different lighting/angle ✅ **Recommended threshold**
- 35-50 bits: Loose matching (higher false accept rate)
- 100+ bits: Different objects (random chance ≈ 128)

### 3. BiometricPassword - Seed Derivation & Password Generation

Derives cryptographic seeds and generates deterministic passwords:

```swift
let seed = BiometricPassword.deriveSeed(
    deviceSecret: deviceSecret,
    salt: salt,
    template: template
)

let password = BiometricPassword.generate(from: seed, policy: .recommended)
```

**Password Policies:**
- `.recommended` - 20 chars, all classes, must include each
- `.highSecurity` - 32 chars, all classes, must include each
- `.alphanumeric` - 16 chars, letters + digits only
- Custom policies via `PasswordPolicy` initializer

## Security Model

### What VisualKeyKit Provides

✅ **Cryptographic primitives** - Perceptual hashing, fuzzy matching, key derivation
✅ **Deterministic generation** - Same inputs → same password
✅ **No secret storage** - Only stores templates (not passwords/images)
✅ **Device binding** - Requires device secret for seed derivation

### What Your App Must Provide

🔐 **Keychain storage** - Secure storage for device secret
📱 **Biometric gates** - Face ID/Touch ID before viewing enrollments
📷 **Camera integration** - Capture visual keys as CGImage
🎨 **Enrollment UX** - Guide users through multi-sample capture
🗑️ **Panic delete** - Emergency wipe of all enrollments

### Threat Model

**Protects Against:**
- Device seizure (no passwords stored, biometric auth required)
- Remote attacks (offline, no network surface)
- Forensic analysis (encrypted storage + panic delete)
- Coercion ("I don't know my passwords" is truthful)

**Does NOT Protect Against:**
- Device unlock + biometric coercion + physical object access
- Supply chain attacks (malicious OS/hardware)
- Social engineering for device transfer

## Algorithm Details

### Perceptual Hashing Pipeline

1. **Preprocessing** (32×32 grayscale normalization)
   - High-quality downsampling via vImage (Accelerate)
   - BT.601 luma conversion: `Y = 0.299R + 0.587G + 0.114B`
   - Gamma compression (γ=0.9) to enhance texture
   - Contrast normalization (zero mean, bounded variance)

2. **Hash Computation** (4 algorithms × 64 bits = 256 bits)
   - **aHash**: Downsample to 8×8, threshold by mean
   - **dHashH**: Downsample to 9×8, compare horizontal neighbors
   - **dHashV**: Downsample to 8×9, compare vertical neighbors
   - **pHash**: 32×32 DCT (Type II), extract 8×8 low-freq, threshold by median

3. **Template Building** (majority voting)
   - For each bit position, count votes from N samples
   - Template bit = 1 if ≥50% of samples voted 1, else 0
   - Confidence = number of samples that agreed on each bit

### Password Generation

1. **Seed Derivation**: `HMAC-SHA256(deviceSecret, salt || template)`
2. **Stream Expansion**: `HKDF-SHA256(seed, info="pw-stream")`
3. **Character Sampling**: Rejection sampling (no modulo bias)
4. **Shuffling**: Fisher-Yates with deterministic PRNG

## Performance

- **Signature computation**: ~5-10ms on iPhone 12 (32×32 DCT)
- **Template building**: ~1ms for 5 samples
- **Password generation**: <1ms for 32-char password
- **Memory usage**: <1MB peak (image processing)

## Testing

```bash
swift test
```

Tests include:
- Perceptual hash stability (same object → low distance)
- Perceptual hash uniqueness (different objects → high distance)
- Template fuzzy matching with configurable thresholds
- Password determinism (same seed → same password)
- Password entropy and character distribution

## Roadmap

- [ ] Rotation normalization (PCA-based alignment)
- [ ] Illumination invariance improvements
- [ ] Benchmarks against academic perceptual hashing libraries
- [ ] C API for cross-platform usage
- [ ] WASM build for web integration

## License

**Dual licensing** (TBD):
- **GPL v3** for open source projects
- **Commercial license** for proprietary use (contact for pricing)

## Credits

Perceptual hashing algorithms based on:
- **avgHash/aHash**: [Looks Like It](https://www.hackerfactor.com/blog/index.php?/archives/432-Looks-Like-It.html) by Dr. Neal Krawetz
- **dHash**: [Kind of Like That](http://www.hackerfactor.com/blog/index.php?/archives/529-Kind-of-Like-That.html) by Dr. Neal Krawetz
- **pHash**: [The pHash Library](https://www.phash.org/)

## Contributing

Contributions welcome! Please:
1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## Disclaimer

VisualKeyKit is a cryptographic toolkit, not a complete password manager. It provides the primitives for biometric password generation but **does not replace** security best practices:

- Use alongside traditional password managers (1Password, Bitwarden)
- Enable device encryption and biometric locks
- Understand the trade-off: object loss = password loss
- Test your enrollments before relying on them for critical accounts

**Use at your own risk. No warranties provided.**

---

**Made with ❤️ for privacy-focused users, journalists, and security researchers.**
