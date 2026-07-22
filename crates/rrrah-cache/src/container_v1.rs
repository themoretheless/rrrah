use std::io::{Read, Write};

use thiserror::Error;

use crate::ArtifactKey;

pub const OBJECT_HEADER_V1_BYTES: usize = 136;
pub const MAX_OBJECT_DESCRIPTOR_BYTES: u32 = 64 * 1024;
pub const MAX_OBJECT_PAYLOAD_BYTES: u64 = 1 << 30;

const MAGIC: [u8; 8] = *b"RRRAHOBJ";
const OBJECT_HEADER_V1_BYTES_U16: u16 = 136;
const ENVELOPE_VERSION: u16 = 1;
const ENVELOPE_FLAGS: u32 = 0;
const PAYLOAD_DIGEST_CONTEXT: &str = "rrrah.cache.object-payload.v1";
const ENVELOPE_DIGEST_CONTEXT: &str = "rrrah.cache.object-envelope.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PayloadSchema {
    id: u32,
    version: u16,
}

/// Complete typed locator of one immutable cache object.
///
/// Schema identity is part of the physical address so a representation change
/// cannot silently reuse bytes written by another codec version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectLocator {
    schema: PayloadSchema,
    artifact_key: ArtifactKey,
}

/// Checksum of the exact stored payload bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PayloadDigest([u8; 32]);

impl PayloadDigest {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl ObjectLocator {
    pub const fn new(schema: PayloadSchema, artifact_key: ArtifactKey) -> Self {
        Self { schema, artifact_key }
    }

    pub const fn schema(self) -> PayloadSchema {
        self.schema
    }

    pub const fn artifact_key(self) -> ArtifactKey {
        self.artifact_key
    }
}

impl PayloadSchema {
    pub const fn new(id: u32, version: u16) -> Result<Self, PayloadSchemaError> {
        if id == 0 || version == 0 {
            return Err(PayloadSchemaError);
        }
        Ok(Self { id, version })
    }

    pub const fn id(self) -> u32 {
        self.id
    }

    pub const fn version(self) -> u16 {
        self.version
    }

    pub(crate) const fn from_static(id: u32, version: u16) -> Self {
        Self { id, version }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContainerHeaderV1 {
    schema: PayloadSchema,
    descriptor_bytes: u32,
    payload_bytes: u64,
    artifact_key: ArtifactKey,
    payload_digest: PayloadDigest,
    envelope_digest: [u8; 32],
}

impl ContainerHeaderV1 {
    /// Validates the self-describing prefix before a reader commits to the
    /// complete V1 header size. This preserves unknown future envelopes as an
    /// unsupported outcome instead of misclassifying them as truncated V1.
    pub fn parse_preamble(bytes: &[u8; 12]) -> Result<(), ContainerError> {
        if bytes[..8] != MAGIC {
            return Err(ContainerError::BadMagic);
        }
        let version = u16::from_le_bytes([bytes[8], bytes[9]]);
        if version != ENVELOPE_VERSION {
            return Err(ContainerError::UnsupportedEnvelopeVersion(version));
        }
        let header_bytes = u16::from_le_bytes([bytes[10], bytes[11]]);
        if usize::from(header_bytes) != OBJECT_HEADER_V1_BYTES {
            return Err(ContainerError::InvalidHeaderBytes(header_bytes));
        }
        Ok(())
    }

    pub fn new(
        locator: ObjectLocator,
        descriptor: &[u8],
        payload_bytes: u64,
        payload_digest: PayloadDigest,
    ) -> Result<Self, ContainerError> {
        let descriptor_bytes = validate_lengths(descriptor.len(), payload_bytes)?;
        let mut header = Self {
            schema: locator.schema,
            descriptor_bytes,
            payload_bytes,
            artifact_key: locator.artifact_key,
            payload_digest,
            envelope_digest: [0; 32],
        };
        let prefix = header.encode_prefix();
        header.envelope_digest = envelope_digest(&prefix, descriptor);
        Ok(header)
    }

    pub fn parse_header(
        bytes: &[u8; OBJECT_HEADER_V1_BYTES],
        file_bytes: u64,
        expected_locator: ObjectLocator,
    ) -> Result<Self, ContainerError> {
        let mut preamble = [0_u8; 12];
        preamble.copy_from_slice(&bytes[..12]);
        Self::parse_preamble(&preamble)?;
        let flags = read_u32(bytes, 12);
        if flags != ENVELOPE_FLAGS {
            return Err(ContainerError::UnsupportedEnvelopeFlags(flags));
        }
        let schema = PayloadSchema::new(read_u32(bytes, 16), read_u16(bytes, 20))
            .map_err(|_| ContainerError::ZeroPayloadSchema)?;
        if schema != expected_locator.schema {
            return Err(ContainerError::PayloadSchemaMismatch {
                expected_id: expected_locator.schema.id,
                expected_version: expected_locator.schema.version,
                actual_id: schema.id,
                actual_version: schema.version,
            });
        }
        if bytes[22..24] != [0; 2] || bytes[28..32] != [0; 4] {
            return Err(ContainerError::NonZeroReserved);
        }
        let descriptor_bytes = read_u32(bytes, 24);
        let payload_bytes = read_u64(bytes, 32);
        validate_lengths(
            usize::try_from(descriptor_bytes).map_err(|_| ContainerError::LengthOverflow)?,
            payload_bytes,
        )?;
        let expected_file_bytes = expected_file_bytes(descriptor_bytes, payload_bytes)?;
        if file_bytes != expected_file_bytes {
            return Err(ContainerError::FileLengthMismatch {
                expected: expected_file_bytes,
                actual: file_bytes,
            });
        }
        let artifact_key = ArtifactKey::from_bytes(copy_32(bytes, 40));
        if artifact_key != expected_locator.artifact_key {
            return Err(ContainerError::ArtifactKeyMismatch);
        }
        Ok(Self {
            schema,
            descriptor_bytes,
            payload_bytes,
            artifact_key,
            payload_digest: PayloadDigest::from_bytes(copy_32(bytes, 72)),
            envelope_digest: copy_32(bytes, 104),
        })
    }

    pub fn encode(self) -> [u8; OBJECT_HEADER_V1_BYTES] {
        let mut bytes = self.encode_prefix();
        bytes[104..136].copy_from_slice(&self.envelope_digest);
        bytes
    }

    pub fn verify_descriptor(self, descriptor: &[u8]) -> Result<(), ContainerError> {
        if descriptor.len()
            != usize::try_from(self.descriptor_bytes).map_err(|_| ContainerError::LengthOverflow)?
        {
            return Err(ContainerError::DescriptorLengthMismatch);
        }
        let prefix = self.encode_prefix();
        if envelope_digest(&prefix, descriptor) != self.envelope_digest {
            return Err(ContainerError::EnvelopeChecksumMismatch);
        }
        Ok(())
    }

    pub fn verify_payload(self, payload: &[u8]) -> Result<(), ContainerError> {
        self.verify_payload_digest(
            u64::try_from(payload.len()).map_err(|_| ContainerError::LengthOverflow)?,
            object_payload_digest(payload),
        )
    }

    pub fn verify_payload_digest(
        self,
        payload_bytes: u64,
        payload_digest: PayloadDigest,
    ) -> Result<(), ContainerError> {
        if payload_bytes != self.payload_bytes {
            return Err(ContainerError::PayloadLengthMismatch);
        }
        if payload_digest != self.payload_digest {
            return Err(ContainerError::PayloadChecksumMismatch);
        }
        Ok(())
    }

    pub const fn schema(self) -> PayloadSchema {
        self.schema
    }

    pub const fn descriptor_bytes(self) -> u32 {
        self.descriptor_bytes
    }

    pub const fn payload_bytes(self) -> u64 {
        self.payload_bytes
    }

    pub const fn artifact_key(self) -> ArtifactKey {
        self.artifact_key
    }

    pub const fn payload_digest(self) -> PayloadDigest {
        self.payload_digest
    }

    fn encode_prefix(self) -> [u8; OBJECT_HEADER_V1_BYTES] {
        let mut bytes = [0_u8; OBJECT_HEADER_V1_BYTES];
        bytes[..8].copy_from_slice(&MAGIC);
        bytes[8..10].copy_from_slice(&ENVELOPE_VERSION.to_le_bytes());
        bytes[10..12].copy_from_slice(&OBJECT_HEADER_V1_BYTES_U16.to_le_bytes());
        bytes[12..16].copy_from_slice(&ENVELOPE_FLAGS.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.schema.id.to_le_bytes());
        bytes[20..22].copy_from_slice(&self.schema.version.to_le_bytes());
        bytes[24..28].copy_from_slice(&self.descriptor_bytes.to_le_bytes());
        bytes[32..40].copy_from_slice(&self.payload_bytes.to_le_bytes());
        bytes[40..72].copy_from_slice(self.artifact_key.as_bytes());
        bytes[72..104].copy_from_slice(self.payload_digest.as_bytes());
        bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("payload schema ID and version must be non-zero")]
pub struct PayloadSchemaError;

pub fn object_payload_digest(payload: &[u8]) -> PayloadDigest {
    let mut hasher = ObjectPayloadHasher::new();
    hasher.update(payload);
    hasher.finalize()
}

/// Incremental checksum for a stored payload.
///
/// Writers and readers can hash the same chunks they already transfer without
/// materializing or traversing the complete payload a second time.
#[derive(Debug)]
pub struct ObjectPayloadHasher(blake3::Hasher);

impl ObjectPayloadHasher {
    pub fn new() -> Self {
        Self(blake3::Hasher::new_derive_key(PAYLOAD_DIGEST_CONTEXT))
    }

    pub fn update(&mut self, chunk: &[u8]) {
        self.0.update(chunk);
    }

    pub fn finalize(self) -> PayloadDigest {
        PayloadDigest::from_bytes(*self.0.finalize().as_bytes())
    }
}

impl Default for ObjectPayloadHasher {
    fn default() -> Self {
        Self::new()
    }
}

/// Counts and hashes exactly the bytes accepted by the wrapped writer.
#[derive(Debug)]
pub struct PayloadDigestWriter<W> {
    inner: W,
    hasher: ObjectPayloadHasher,
    bytes: u64,
}

impl<W> PayloadDigestWriter<W> {
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            hasher: ObjectPayloadHasher::new(),
            bytes: 0,
        }
    }

    /// Finalizes accounting without flushing the wrapped writer.
    ///
    /// This is appropriate for in-memory sinks such as `Vec<u8>`. Buffered or
    /// persistent sinks should normally use [`Self::try_finish`] so a flush
    /// failure prevents the digest from being accepted as a completed write.
    pub fn finish(self) -> (W, u64, PayloadDigest) {
        (self.inner, self.bytes, self.hasher.finalize())
    }
}

impl<W: Write> PayloadDigestWriter<W> {
    /// Flushes the wrapped writer, then returns it with the byte count and digest.
    ///
    /// `Write::flush` only completes the writer's userspace buffering contract.
    /// It is not an `fsync`/durability guarantee. A filesystem object store must
    /// separately define file sync, atomic publication and directory sync.
    pub fn try_finish(mut self) -> std::io::Result<(W, u64, PayloadDigest)> {
        self.inner.flush()?;
        Ok(self.finish())
    }
}

impl<W: Write> Write for PayloadDigestWriter<W> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let written = self.inner.write(buffer)?;
        self.bytes = self
            .bytes
            .checked_add(u64::try_from(written).map_err(|_| digest_length_overflow())?)
            .ok_or_else(digest_length_overflow)?;
        self.hasher.update(&buffer[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// Counts and hashes exactly the bytes returned by the wrapped reader.
#[derive(Debug)]
pub struct PayloadDigestReader<R> {
    inner: R,
    hasher: ObjectPayloadHasher,
    bytes: u64,
}

impl<R> PayloadDigestReader<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            hasher: ObjectPayloadHasher::new(),
            bytes: 0,
        }
    }

    pub fn finish(self) -> (R, u64, PayloadDigest) {
        (self.inner, self.bytes, self.hasher.finalize())
    }
}

impl<R: Read> Read for PayloadDigestReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let read = self.inner.read(buffer)?;
        self.bytes = self
            .bytes
            .checked_add(u64::try_from(read).map_err(|_| digest_length_overflow())?)
            .ok_or_else(digest_length_overflow)?;
        self.hasher.update(&buffer[..read]);
        Ok(read)
    }
}

fn digest_length_overflow() -> std::io::Error {
    std::io::Error::other("payload byte count overflow")
}

fn envelope_digest(prefix: &[u8; OBJECT_HEADER_V1_BYTES], descriptor: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new_derive_key(ENVELOPE_DIGEST_CONTEXT);
    hasher.update(&prefix[..104]);
    hasher.update(descriptor);
    *hasher.finalize().as_bytes()
}

fn validate_lengths(descriptor_bytes: usize, payload_bytes: u64) -> Result<u32, ContainerError> {
    let descriptor_bytes = u32::try_from(descriptor_bytes).map_err(|_| ContainerError::DescriptorTooLarge)?;
    if descriptor_bytes > MAX_OBJECT_DESCRIPTOR_BYTES {
        return Err(ContainerError::DescriptorTooLarge);
    }
    if payload_bytes > MAX_OBJECT_PAYLOAD_BYTES {
        return Err(ContainerError::PayloadTooLarge(payload_bytes));
    }
    expected_file_bytes(descriptor_bytes, payload_bytes)?;
    Ok(descriptor_bytes)
}

fn expected_file_bytes(descriptor_bytes: u32, payload_bytes: u64) -> Result<u64, ContainerError> {
    u64::try_from(OBJECT_HEADER_V1_BYTES)
        .map_err(|_| ContainerError::LengthOverflow)?
        .checked_add(u64::from(descriptor_bytes))
        .and_then(|bytes| bytes.checked_add(payload_bytes))
        .ok_or(ContainerError::LengthOverflow)
}

fn read_u16(bytes: &[u8; OBJECT_HEADER_V1_BYTES], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8; OBJECT_HEADER_V1_BYTES], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("fixed header slice"))
}

fn read_u64(bytes: &[u8; OBJECT_HEADER_V1_BYTES], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("fixed header slice"))
}

fn copy_32(bytes: &[u8; OBJECT_HEADER_V1_BYTES], offset: usize) -> [u8; 32] {
    bytes[offset..offset + 32].try_into().expect("fixed header slice")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ContainerError {
    #[error("bad object container magic")]
    BadMagic,
    #[error("unsupported object envelope version {0}")]
    UnsupportedEnvelopeVersion(u16),
    #[error("invalid object header byte count {0}")]
    InvalidHeaderBytes(u16),
    #[error("unsupported object envelope flags 0x{0:08x}")]
    UnsupportedEnvelopeFlags(u32),
    #[error("payload schema ID and version must be non-zero")]
    ZeroPayloadSchema,
    #[error(
        "object payload schema does not match its locator: expected {expected_id}/{expected_version}, got {actual_id}/{actual_version}"
    )]
    PayloadSchemaMismatch {
        expected_id: u32,
        expected_version: u16,
        actual_id: u32,
        actual_version: u16,
    },
    #[error("object container reserved bytes must be zero")]
    NonZeroReserved,
    #[error("object descriptor exceeds 64 KiB")]
    DescriptorTooLarge,
    #[error("object payload exceeds the hard limit: {0} bytes")]
    PayloadTooLarge(u64),
    #[error("object length arithmetic overflow")]
    LengthOverflow,
    #[error("object file length mismatch: expected {expected}, got {actual}")]
    FileLengthMismatch { expected: u64, actual: u64 },
    #[error("object artifact key does not match its locator")]
    ArtifactKeyMismatch,
    #[error("object descriptor length mismatch")]
    DescriptorLengthMismatch,
    #[error("object payload length mismatch")]
    PayloadLengthMismatch,
    #[error("object envelope checksum mismatch")]
    EnvelopeChecksumMismatch,
    #[error("object payload checksum mismatch")]
    PayloadChecksumMismatch,
}

#[cfg(test)]
mod tests {
    use std::io::{Read as _, Write as _};

    use super::*;

    fn key() -> ArtifactKey {
        ArtifactKey::from_bytes([0x44; 32])
    }

    fn locator() -> ObjectLocator {
        ObjectLocator::new(PayloadSchema::new(1, 1).unwrap(), key())
    }

    fn fixture() -> (ContainerHeaderV1, Vec<u8>, Vec<u8>) {
        let descriptor = (0_u8..106).collect::<Vec<_>>();
        let payload = b"canonical mosaic payload".to_vec();
        let header = ContainerHeaderV1::new(
            locator(),
            &descriptor,
            payload.len() as u64,
            object_payload_digest(&payload),
        )
        .unwrap();
        (header, descriptor, payload)
    }

    #[derive(Debug, Default)]
    struct FlushProbe {
        bytes: Vec<u8>,
        flushes: usize,
        fail_flush: bool,
    }

    impl std::io::Write for FlushProbe {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            self.bytes.extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            self.flushes += 1;
            if self.fail_flush {
                Err(std::io::Error::other("injected flush failure"))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn fixed_header_round_trips_and_covers_descriptor_and_payload() {
        let (header, descriptor, payload) = fixture();
        let bytes = header.encode();
        assert_eq!(&bytes[..8], b"RRRAHOBJ");
        assert_eq!(&bytes[8..12], &[1, 0, 136, 0]);
        assert_eq!(&bytes[16..22], &[1, 0, 0, 0, 1, 0]);
        assert_eq!(read_u32(&bytes, 24), 106);
        assert_eq!(read_u64(&bytes, 32), payload.len() as u64);
        assert_eq!(&bytes[40..72], &[0x44; 32]);

        let file_bytes = 136 + descriptor.len() as u64 + payload.len() as u64;
        let parsed = ContainerHeaderV1::parse_header(&bytes, file_bytes, locator()).unwrap();
        parsed.verify_descriptor(&descriptor).unwrap();
        parsed.verify_payload(&payload).unwrap();
        assert_eq!(parsed, header);
    }

    #[test]
    fn corruption_and_unsupported_versions_are_distinct() {
        let (header, descriptor, payload) = fixture();
        let file_bytes = 136 + descriptor.len() as u64 + payload.len() as u64;

        let mut bytes = header.encode();
        bytes[8..10].copy_from_slice(&2_u16.to_le_bytes());
        assert_eq!(
            ContainerHeaderV1::parse_header(&bytes, file_bytes, locator()),
            Err(ContainerError::UnsupportedEnvelopeVersion(2))
        );

        let mut bytes = header.encode();
        bytes[12..16].copy_from_slice(&1_u32.to_le_bytes());
        assert_eq!(
            ContainerHeaderV1::parse_header(&bytes, file_bytes, locator()),
            Err(ContainerError::UnsupportedEnvelopeFlags(1))
        );

        let mut changed_descriptor = descriptor.clone();
        changed_descriptor[17] ^= 1;
        assert_eq!(
            header.verify_descriptor(&changed_descriptor),
            Err(ContainerError::EnvelopeChecksumMismatch)
        );

        let mut changed_payload = payload;
        changed_payload[3] ^= 1;
        assert_eq!(
            header.verify_payload(&changed_payload),
            Err(ContainerError::PayloadChecksumMismatch)
        );

        assert_eq!(
            ContainerHeaderV1::parse_header(
                &header.encode(),
                file_bytes,
                ObjectLocator::new(PayloadSchema::new(1, 2).unwrap(), key()),
            ),
            Err(ContainerError::PayloadSchemaMismatch {
                expected_id: 1,
                expected_version: 2,
                actual_id: 1,
                actual_version: 1,
            })
        );

        let mut future_preamble = [0_u8; 12];
        future_preamble[..8].copy_from_slice(b"RRRAHOBJ");
        future_preamble[8..10].copy_from_slice(&2_u16.to_le_bytes());
        future_preamble[10..12].copy_from_slice(&12_u16.to_le_bytes());
        assert_eq!(
            ContainerHeaderV1::parse_preamble(&future_preamble),
            Err(ContainerError::UnsupportedEnvelopeVersion(2))
        );
    }

    #[test]
    fn streaming_payload_digest_is_independent_of_chunk_boundaries() {
        let payload = (0..1_000_003_u32)
            .map(|value| value.wrapping_mul(17).to_le_bytes()[1])
            .collect::<Vec<_>>();
        let expected = object_payload_digest(&payload);
        for chunk_size in [1, 3, 4096, 256 * 1024, payload.len()] {
            let mut hasher = ObjectPayloadHasher::new();
            for chunk in payload.chunks(chunk_size) {
                hasher.update(chunk);
            }
            assert_eq!(hasher.finalize(), expected, "chunk size {chunk_size}");

            let mut writer = PayloadDigestWriter::new(Vec::new());
            for chunk in payload.chunks(chunk_size) {
                writer.write_all(chunk).unwrap();
            }
            let (written, byte_count, digest) = writer.try_finish().unwrap();
            assert_eq!(written, payload);
            assert_eq!(byte_count, payload.len() as u64);
            assert_eq!(digest, expected);

            let mut reader = PayloadDigestReader::new(payload.as_slice());
            let mut copied = Vec::new();
            let mut buffer = vec![0_u8; chunk_size];
            loop {
                let read = reader.read(&mut buffer).unwrap();
                if read == 0 {
                    break;
                }
                copied.extend_from_slice(&buffer[..read]);
            }
            let (_, byte_count, digest) = reader.finish();
            assert_eq!(copied, payload);
            assert_eq!(byte_count, payload.len() as u64);
            assert_eq!(digest, expected);
        }
    }

    #[test]
    fn writer_completion_distinguishes_flush_from_digest_finalization() {
        let payload = b"buffered payload";
        let expected = object_payload_digest(payload);

        let mut writer = PayloadDigestWriter::new(FlushProbe::default());
        writer.write_all(payload).unwrap();
        let (probe, byte_count, digest) = writer.try_finish().unwrap();
        assert_eq!(probe.bytes, payload);
        assert_eq!(probe.flushes, 1);
        assert_eq!(byte_count, payload.len() as u64);
        assert_eq!(digest, expected);

        let mut unflushed = PayloadDigestWriter::new(FlushProbe::default());
        unflushed.write_all(payload).unwrap();
        let (probe, _, _) = unflushed.finish();
        assert_eq!(probe.flushes, 0);

        let mut failing = PayloadDigestWriter::new(FlushProbe {
            fail_flush: true,
            ..FlushProbe::default()
        });
        failing.write_all(payload).unwrap();
        assert_eq!(
            failing.try_finish().unwrap_err().to_string(),
            "injected flush failure"
        );
    }

    #[test]
    fn lengths_are_checked_before_descriptor_or_payload_allocation() {
        let (header, descriptor, payload) = fixture();
        let bytes = header.encode();
        let actual = 136 + descriptor.len() as u64 + payload.len() as u64;
        assert!(matches!(
            ContainerHeaderV1::parse_header(&bytes, actual - 1, locator()),
            Err(ContainerError::FileLengthMismatch { .. })
        ));
        assert_eq!(
            ContainerHeaderV1::new(
                locator(),
                &vec![0; MAX_OBJECT_DESCRIPTOR_BYTES as usize + 1],
                0,
                object_payload_digest(&[]),
            ),
            Err(ContainerError::DescriptorTooLarge)
        );
    }

    #[test]
    fn every_fixed_header_guard_has_an_exact_regression_case() {
        let (header, descriptor, payload) = fixture();
        let file_bytes = 136 + descriptor.len() as u64 + payload.len() as u64;

        let mut bytes = header.encode();
        bytes[0] ^= 1;
        assert_eq!(
            ContainerHeaderV1::parse_header(&bytes, file_bytes, locator()),
            Err(ContainerError::BadMagic)
        );

        let mut bytes = header.encode();
        bytes[10..12].copy_from_slice(&135_u16.to_le_bytes());
        assert_eq!(
            ContainerHeaderV1::parse_header(&bytes, file_bytes, locator()),
            Err(ContainerError::InvalidHeaderBytes(135))
        );

        let mut bytes = header.encode();
        bytes[16..20].copy_from_slice(&0_u32.to_le_bytes());
        assert_eq!(
            ContainerHeaderV1::parse_header(&bytes, file_bytes, locator()),
            Err(ContainerError::ZeroPayloadSchema)
        );

        let mut bytes = header.encode();
        bytes[22] = 1;
        assert_eq!(
            ContainerHeaderV1::parse_header(&bytes, file_bytes, locator()),
            Err(ContainerError::NonZeroReserved)
        );

        let mut bytes = header.encode();
        bytes[24..28].copy_from_slice(&(MAX_OBJECT_DESCRIPTOR_BYTES + 1).to_le_bytes());
        assert_eq!(
            ContainerHeaderV1::parse_header(&bytes, file_bytes, locator()),
            Err(ContainerError::DescriptorTooLarge)
        );

        let mut bytes = header.encode();
        bytes[32..40].copy_from_slice(&(MAX_OBJECT_PAYLOAD_BYTES + 1).to_le_bytes());
        assert_eq!(
            ContainerHeaderV1::parse_header(&bytes, file_bytes, locator()),
            Err(ContainerError::PayloadTooLarge(MAX_OBJECT_PAYLOAD_BYTES + 1))
        );

        let mut bytes = header.encode();
        bytes[40] ^= 1;
        assert_eq!(
            ContainerHeaderV1::parse_header(&bytes, file_bytes, locator()),
            Err(ContainerError::ArtifactKeyMismatch)
        );

        let mut bytes = header.encode();
        bytes[104] ^= 1;
        let parsed = ContainerHeaderV1::parse_header(&bytes, file_bytes, locator()).unwrap();
        assert_eq!(
            parsed.verify_descriptor(&descriptor),
            Err(ContainerError::EnvelopeChecksumMismatch)
        );
    }
}
