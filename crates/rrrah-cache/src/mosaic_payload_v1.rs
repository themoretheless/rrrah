use std::{
    io::{Read, Write},
    sync::Arc,
};

use rrrah_core::{
    CfaColor, CfaPattern, DecodedMosaic, FrameError, LevelGrid, Orientation, Photometric, RawMetadata, Rect,
    WhiteLevel,
};
use thiserror::Error;

use crate::PayloadSchema;

pub const MOSAIC_PAYLOAD_SCHEMA_ID: u32 = 1;
pub const MOSAIC_PAYLOAD_SCHEMA_VERSION_V1: u16 = 1;
pub const MOSAIC_PAYLOAD_HEADER_V1_BYTES: usize = 192;
pub const MAX_MOSAIC_DESCRIPTOR_BYTES: u64 = 1 << 20;
pub const MAX_MOSAIC_SAMPLES: u64 = 250_000_000;

const MAGIC: [u8; 8] = *b"RRMPAY1\0";
const MOSAIC_PAYLOAD_HEADER_V1_BYTES_U32: u32 = 192;
const MOSAIC_PAYLOAD_HEADER_V1_BYTES_U64: u64 = 192;
const MAJOR: u16 = MOSAIC_PAYLOAD_SCHEMA_VERSION_V1;
const MINOR: u16 = 0;
const SAMPLE_FORMAT_U16_LE_INTERLEAVED: u16 = 1;
const FLAG_CFA: u32 = 1 << 0;
const FLAG_ACTIVE_AREA: u32 = 1 << 1;
const FLAG_CROP_AREA: u32 = 1 << 2;
const KNOWN_FLAGS: u32 = FLAG_CFA | FLAG_ACTIVE_AREA | FLAG_CROP_AREA;
const MAX_STRING_BYTES: u32 = 64 * 1024;
const MAX_METADATA_VALUES: u32 = 64 * 1024;
// Keep direct File I/O bounded without requiring callers to add buffering.
// This turns the largest legal f32 vector from 65,536 four-byte operations
// into 16 transfers while keeping stack use modest.
const METADATA_IO_BUFFER_BYTES: usize = 16 * 1024;
const PIXEL_BUFFER_BYTES: usize = 256 * 1024;

pub const fn mosaic_payload_schema_v1() -> PayloadSchema {
    PayloadSchema::from_static(MOSAIC_PAYLOAD_SCHEMA_ID, MOSAIC_PAYLOAD_SCHEMA_VERSION_V1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MosaicPayloadStatsV1 {
    pub metadata_prefix_bytes: u32,
    pub payload_bytes: u64,
    pub sample_count: u64,
}

/// Per-operation admission limits applied after structural validation and
/// before any variable metadata or pixel allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MosaicDecodeLimits {
    max_metadata_prefix_bytes: u64,
    max_samples: u64,
}

impl MosaicDecodeLimits {
    pub const PRODUCTION: Self = Self {
        max_metadata_prefix_bytes: MAX_MOSAIC_DESCRIPTOR_BYTES,
        max_samples: MAX_MOSAIC_SAMPLES,
    };

    pub const fn new(max_metadata_prefix_bytes: u64, max_samples: u64) -> Self {
        Self {
            max_metadata_prefix_bytes,
            max_samples,
        }
    }

    pub const fn max_metadata_prefix_bytes(self) -> u64 {
        self.max_metadata_prefix_bytes
    }

    pub const fn max_samples(self) -> u64 {
        self.max_samples
    }

    fn admit(self, layout: MosaicLayoutV1) -> Result<(), MosaicPayloadError> {
        if u64::from(layout.descriptor_bytes) > self.max_metadata_prefix_bytes {
            return Err(MosaicPayloadError::MetadataBudgetExceeded {
                requested: u64::from(layout.descriptor_bytes),
                limit: self.max_metadata_prefix_bytes,
            });
        }
        if layout.sample_count > self.max_samples {
            return Err(MosaicPayloadError::SampleBudgetExceeded {
                requested: layout.sample_count,
                limit: self.max_samples,
            });
        }
        Ok(())
    }
}

impl Default for MosaicDecodeLimits {
    fn default() -> Self {
        Self::PRODUCTION
    }
}

#[derive(Debug)]
pub struct PreparedMosaicPayloadV1<'a> {
    mosaic: &'a DecodedMosaic,
    layout: MosaicLayoutV1,
}

impl PreparedMosaicPayloadV1<'_> {
    pub const fn stats(&self) -> MosaicPayloadStatsV1 {
        MosaicPayloadStatsV1::from_layout(self.layout)
    }

    pub fn encode(&self, writer: &mut impl Write) -> Result<(), MosaicPayloadError> {
        encode_prepared_mosaic(writer, self.mosaic, self.layout)
    }
}

pub fn prepare_mosaic_payload_v1(
    mosaic: &DecodedMosaic,
) -> Result<PreparedMosaicPayloadV1<'_>, MosaicPayloadError> {
    Ok(PreparedMosaicPayloadV1 {
        mosaic,
        layout: layout_for_mosaic(mosaic)?,
    })
}

pub fn encode_mosaic_payload_v1(
    writer: &mut impl Write,
    mosaic: &DecodedMosaic,
) -> Result<MosaicPayloadStatsV1, MosaicPayloadError> {
    let prepared = prepare_mosaic_payload_v1(mosaic)?;
    prepared.encode(writer)?;
    Ok(prepared.stats())
}

fn encode_prepared_mosaic(
    writer: &mut impl Write,
    mosaic: &DecodedMosaic,
    layout: MosaicLayoutV1,
) -> Result<(), MosaicPayloadError> {
    let metadata = &mosaic.metadata;
    let make = metadata.make.as_bytes();
    let model = metadata.model.as_bytes();

    let header = encode_header(metadata, layout)?;
    writer.write_all(&header)?;
    writer.write_all(make)?;
    writer.write_all(model)?;
    if let Some(cfa) = &metadata.cfa {
        let mut cells = Vec::new();
        cells
            .try_reserve_exact(cfa.cells.len())
            .map_err(|_| MosaicPayloadError::AllocationFailed)?;
        cells.extend(cfa.cells.iter().copied().map(encode_cfa_color));
        writer.write_all(&cells)?;
    }
    write_f32_values(writer, &metadata.black_level.values)?;
    write_f32_values(writer, &metadata.white_level.0)?;
    write_pixels(writer, &mosaic.pixels)?;
    Ok(())
}

pub fn decode_mosaic_payload_v1(
    reader: &mut impl Read,
    payload_bytes: u64,
) -> Result<DecodedMosaic, MosaicPayloadError> {
    decode_mosaic_payload_v1_with_limits(reader, payload_bytes, MosaicDecodeLimits::PRODUCTION)
}

pub fn decode_mosaic_payload_v1_with_limits(
    reader: &mut impl Read,
    payload_bytes: u64,
    limits: MosaicDecodeLimits,
) -> Result<DecodedMosaic, MosaicPayloadError> {
    if payload_bytes < MOSAIC_PAYLOAD_HEADER_V1_BYTES_U64 {
        return Err(MosaicPayloadError::Invalid(
            "payload is shorter than the fixed mosaic header",
        ));
    }
    let mut header = [0_u8; MOSAIC_PAYLOAD_HEADER_V1_BYTES];
    reader.read_exact(&mut header)?;
    let descriptor = ParsedDescriptor::parse(&header, payload_bytes, limits)?;

    let make = read_utf8(reader, descriptor.make_len)?;
    let model = read_utf8(reader, descriptor.model_len)?;
    let cfa = if descriptor.flags & FLAG_CFA != 0 {
        let cells = read_cfa_cells(reader, descriptor.cfa_count)?;
        Some(CfaPattern {
            width: descriptor.cfa_width,
            height: descriptor.cfa_height,
            cells,
        })
    } else {
        None
    };
    let black_values = read_f32_values(reader, descriptor.black_count)?;
    let white_values = read_f32_values(reader, descriptor.white_count)?;

    let metadata = RawMetadata {
        make,
        model,
        width: descriptor.width,
        height: descriptor.height,
        components_per_pixel: descriptor.components_per_pixel,
        bits_per_sample: descriptor.bits_per_sample,
        photometric: descriptor.photometric,
        cfa,
        black_level: LevelGrid {
            width: descriptor.black_width,
            height: descriptor.black_height,
            components: descriptor.black_components,
            values: black_values,
        },
        white_level: WhiteLevel(white_values),
        white_balance: descriptor.white_balance,
        xyz_to_camera: descriptor.xyz_to_camera,
        active_area: descriptor.active_area,
        crop_area: descriptor.crop_area,
        orientation: descriptor.orientation,
    };
    metadata.validate()?;
    let pixels = read_pixels(reader, descriptor.sample_count)?;
    DecodedMosaic::new(metadata, Arc::new(pixels)).map_err(MosaicPayloadError::InvalidFrame)
}

fn validate_mosaic(mosaic: &DecodedMosaic) -> Result<(), MosaicPayloadError> {
    mosaic.metadata.validate()?;
    let expected = u64::from(mosaic.metadata.width)
        .checked_mul(u64::from(mosaic.metadata.height))
        .and_then(|value| value.checked_mul(u64::from(mosaic.metadata.components_per_pixel)))
        .ok_or(MosaicPayloadError::LengthOverflow)?;
    if expected > MAX_MOSAIC_SAMPLES {
        return Err(MosaicPayloadError::TooManySamples(expected));
    }
    if usize::try_from(expected).ok() != Some(mosaic.pixels.len()) {
        return Err(MosaicPayloadError::Invalid("pixel count does not match metadata"));
    }
    Ok(())
}

fn layout_for_mosaic(mosaic: &DecodedMosaic) -> Result<MosaicLayoutV1, MosaicPayloadError> {
    validate_mosaic(mosaic)?;
    MosaicLayoutV1::from_lengths(
        mosaic.metadata.make.len(),
        mosaic.metadata.model.len(),
        mosaic.metadata.cfa.as_ref().map_or(0, |cfa| cfa.cells.len()),
        mosaic.metadata.black_level.values.len(),
        mosaic.metadata.white_level.0.len(),
        u64::try_from(mosaic.pixels.len()).map_err(|_| MosaicPayloadError::LengthOverflow)?,
    )
}

fn encode_header(
    metadata: &RawMetadata,
    layout: MosaicLayoutV1,
) -> Result<[u8; MOSAIC_PAYLOAD_HEADER_V1_BYTES], MosaicPayloadError> {
    let mut header = [0_u8; MOSAIC_PAYLOAD_HEADER_V1_BYTES];
    header[..8].copy_from_slice(&MAGIC);
    put_u16(&mut header, 8, MAJOR);
    put_u16(&mut header, 10, MINOR);
    put_u32(&mut header, 12, MOSAIC_PAYLOAD_HEADER_V1_BYTES_U32);
    put_u32(&mut header, 16, layout.descriptor_bytes);
    let flags = metadata.cfa.as_ref().map_or(0, |_| FLAG_CFA)
        | metadata.active_area.map_or(0, |_| FLAG_ACTIVE_AREA)
        | metadata.crop_area.map_or(0, |_| FLAG_CROP_AREA);
    put_u32(&mut header, 20, flags);
    put_u32(&mut header, 32, metadata.width);
    put_u32(&mut header, 36, metadata.height);
    put_u64(&mut header, 40, layout.sample_count);
    put_u64(&mut header, 48, layout.pixel_bytes);
    put_u32(&mut header, 56, layout.make_len);
    put_u32(&mut header, 60, layout.model_len);
    put_u32(&mut header, 64, layout.cfa_count);
    put_u32(&mut header, 68, layout.black_count);
    put_u32(&mut header, 72, layout.white_count);
    put_u16(&mut header, 76, SAMPLE_FORMAT_U16_LE_INTERLEAVED);
    header[78] = encode_photometric(&metadata.photometric);
    header[79] = encode_orientation(metadata.orientation);
    header[80] = metadata.components_per_pixel;
    header[81] = metadata.bits_per_sample;
    if let Some(cfa) = &metadata.cfa {
        header[82] = cfa.width;
        header[83] = cfa.height;
    }
    header[84] = metadata.black_level.width;
    header[85] = metadata.black_level.height;
    header[86] = metadata.black_level.components;
    write_rect(&mut header, 96, metadata.active_area);
    write_rect(&mut header, 112, metadata.crop_area);
    for (index, value) in metadata.white_balance.iter().copied().enumerate() {
        put_f32(&mut header, 128 + index * 4, value)?;
    }
    for (index, value) in metadata.xyz_to_camera.iter().flatten().copied().enumerate() {
        put_f32(&mut header, 144 + index * 4, value)?;
    }
    Ok(header)
}

#[derive(Debug)]
struct ParsedDescriptor {
    flags: u32,
    width: u32,
    height: u32,
    sample_count: u64,
    make_len: u32,
    model_len: u32,
    cfa_count: u32,
    black_count: u32,
    white_count: u32,
    photometric: Photometric,
    orientation: Orientation,
    components_per_pixel: u8,
    bits_per_sample: u8,
    cfa_width: u8,
    cfa_height: u8,
    black_width: u8,
    black_height: u8,
    black_components: u8,
    active_area: Option<Rect>,
    crop_area: Option<Rect>,
    white_balance: [f32; 4],
    xyz_to_camera: [[f32; 3]; 4],
}

#[derive(Debug, Clone, Copy)]
struct MosaicLayoutV1 {
    make_len: u32,
    model_len: u32,
    cfa_count: u32,
    black_count: u32,
    white_count: u32,
    descriptor_bytes: u32,
    sample_count: u64,
    pixel_bytes: u64,
    payload_bytes: u64,
}

impl MosaicPayloadStatsV1 {
    const fn from_layout(layout: MosaicLayoutV1) -> Self {
        Self {
            metadata_prefix_bytes: layout.descriptor_bytes,
            payload_bytes: layout.payload_bytes,
            sample_count: layout.sample_count,
        }
    }
}

impl MosaicLayoutV1 {
    fn from_lengths(
        make: usize,
        model: usize,
        cfa: usize,
        black: usize,
        white: usize,
        sample_count: u64,
    ) -> Result<Self, MosaicPayloadError> {
        Self::new(
            len_u32(make)?,
            len_u32(model)?,
            len_u32(cfa)?,
            len_u32(black)?,
            len_u32(white)?,
            sample_count,
        )
    }

    fn new(
        make_len: u32,
        model_len: u32,
        cfa_count: u32,
        black_count: u32,
        white_count: u32,
        sample_count: u64,
    ) -> Result<Self, MosaicPayloadError> {
        validate_metadata_lengths(make_len, model_len, cfa_count, black_count, white_count)?;
        if sample_count > MAX_MOSAIC_SAMPLES {
            return Err(MosaicPayloadError::TooManySamples(sample_count));
        }
        let descriptor_bytes =
            checked_descriptor_bytes(make_len, model_len, cfa_count, black_count, white_count)?;
        let pixel_bytes = sample_count
            .checked_mul(2)
            .ok_or(MosaicPayloadError::LengthOverflow)?;
        let payload_bytes = u64::from(descriptor_bytes)
            .checked_add(pixel_bytes)
            .ok_or(MosaicPayloadError::LengthOverflow)?;
        Ok(Self {
            make_len,
            model_len,
            cfa_count,
            black_count,
            white_count,
            descriptor_bytes,
            sample_count,
            pixel_bytes,
            payload_bytes,
        })
    }
}

impl ParsedDescriptor {
    // Keeping the fixed-offset checks together makes the 192-byte wire contract
    // auditable against its specification and prevents partially validated state.
    #[allow(clippy::too_many_lines)]
    fn parse(
        header: &[u8; MOSAIC_PAYLOAD_HEADER_V1_BYTES],
        payload_bytes: u64,
        limits: MosaicDecodeLimits,
    ) -> Result<Self, MosaicPayloadError> {
        if header[..8] != MAGIC {
            return Err(MosaicPayloadError::Invalid("bad mosaic payload magic"));
        }
        if read_u16(header, 8) != MAJOR || read_u16(header, 10) != MINOR {
            return Err(MosaicPayloadError::UnsupportedVersion {
                major: read_u16(header, 8),
                minor: read_u16(header, 10),
            });
        }
        if read_u32(header, 12) != MOSAIC_PAYLOAD_HEADER_V1_BYTES_U32 {
            return Err(MosaicPayloadError::Invalid(
                "invalid mosaic payload header length",
            ));
        }
        let descriptor_bytes = read_u32(header, 16);
        if u64::from(descriptor_bytes) > MAX_MOSAIC_DESCRIPTOR_BYTES {
            return Err(MosaicPayloadError::DescriptorTooLarge(u64::from(
                descriptor_bytes,
            )));
        }
        let flags = read_u32(header, 20);
        if flags & !KNOWN_FLAGS != 0 {
            return Err(MosaicPayloadError::Invalid("unknown mosaic payload flags"));
        }
        if read_u32(header, 24) != 0 || read_u32(header, 28) != 0 || header[87..96] != [0; 9] {
            return Err(MosaicPayloadError::Invalid(
                "non-zero reserved or extension bytes",
            ));
        }
        let width = read_u32(header, 32);
        let height = read_u32(header, 36);
        let sample_count = read_u64(header, 40);
        let pixel_bytes = read_u64(header, 48);
        let make_len = read_u32(header, 56);
        let model_len = read_u32(header, 60);
        let cfa_count = read_u32(header, 64);
        let black_count = read_u32(header, 68);
        let white_count = read_u32(header, 72);
        let layout = MosaicLayoutV1::new(
            make_len,
            model_len,
            cfa_count,
            black_count,
            white_count,
            sample_count,
        )?;
        let components_per_pixel = header[80];
        let expected_samples = u64::from(width)
            .checked_mul(u64::from(height))
            .and_then(|value| value.checked_mul(u64::from(components_per_pixel)))
            .ok_or(MosaicPayloadError::LengthOverflow)?;
        if width == 0 || height == 0 || components_per_pixel == 0 || sample_count != expected_samples {
            return Err(MosaicPayloadError::Invalid("invalid dimensions or sample count"));
        }
        if pixel_bytes != layout.pixel_bytes {
            return Err(MosaicPayloadError::Invalid("invalid pixel byte count"));
        }
        if descriptor_bytes != layout.descriptor_bytes {
            return Err(MosaicPayloadError::Invalid(
                "descriptor length does not match fields",
            ));
        }
        if payload_bytes != layout.payload_bytes {
            return Err(MosaicPayloadError::PayloadLengthMismatch {
                expected: layout.payload_bytes,
                actual: payload_bytes,
            });
        }
        if read_u16(header, 76) != SAMPLE_FORMAT_U16_LE_INTERLEAVED {
            return Err(MosaicPayloadError::Invalid("unsupported sample format"));
        }
        let photometric = decode_photometric(header[78])?;
        let orientation = decode_orientation(header[79])?;
        let bits_per_sample = header[81];
        if !(1..=16).contains(&bits_per_sample) {
            return Err(MosaicPayloadError::Invalid("invalid bits per sample"));
        }
        let cfa_width = header[82];
        let cfa_height = header[83];
        if flags & FLAG_CFA != 0 {
            let expected = u32::from(cfa_width)
                .checked_mul(u32::from(cfa_height))
                .ok_or(MosaicPayloadError::LengthOverflow)?;
            if expected == 0 || expected != cfa_count {
                return Err(MosaicPayloadError::Invalid("invalid CFA dimensions/count"));
            }
        } else if cfa_width != 0 || cfa_height != 0 || cfa_count != 0 {
            return Err(MosaicPayloadError::Invalid("absent CFA must use canonical zeros"));
        }
        let black_width = header[84];
        let black_height = header[85];
        let black_components = header[86];
        let expected_black = u32::from(black_width)
            .checked_mul(u32::from(black_height))
            .and_then(|value| value.checked_mul(u32::from(black_components)))
            .ok_or(MosaicPayloadError::LengthOverflow)?;
        if expected_black == 0 || expected_black != black_count || white_count == 0 {
            return Err(MosaicPayloadError::Invalid("invalid level-grid/vector count"));
        }
        let active_area = read_rect(header, 96, flags & FLAG_ACTIVE_AREA != 0)?;
        let crop_area = read_rect(header, 112, flags & FLAG_CROP_AREA != 0)?;
        let mut white_balance = [0_f32; 4];
        for (index, value) in white_balance.iter_mut().enumerate() {
            *value = read_f32(header, 128 + index * 4)?;
        }
        let mut xyz_to_camera = [[0_f32; 3]; 4];
        for (index, value) in xyz_to_camera.iter_mut().flatten().enumerate() {
            *value = read_f32(header, 144 + index * 4)?;
        }
        limits.admit(layout)?;
        Ok(Self {
            flags,
            width,
            height,
            sample_count,
            make_len,
            model_len,
            cfa_count,
            black_count,
            white_count,
            photometric,
            orientation,
            components_per_pixel,
            bits_per_sample,
            cfa_width,
            cfa_height,
            black_width,
            black_height,
            black_components,
            active_area,
            crop_area,
            white_balance,
            xyz_to_camera,
        })
    }
}

fn checked_descriptor_bytes(
    make: u32,
    model: u32,
    cfa: u32,
    black: u32,
    white: u32,
) -> Result<u32, MosaicPayloadError> {
    let descriptor = u64::try_from(MOSAIC_PAYLOAD_HEADER_V1_BYTES)
        .map_err(|_| MosaicPayloadError::LengthOverflow)?
        .checked_add(u64::from(make))
        .and_then(|value| value.checked_add(u64::from(model)))
        .and_then(|value| value.checked_add(u64::from(cfa)))
        .and_then(|value| value.checked_add(u64::from(black).checked_mul(4)?))
        .and_then(|value| value.checked_add(u64::from(white).checked_mul(4)?))
        .ok_or(MosaicPayloadError::LengthOverflow)?;
    if descriptor > MAX_MOSAIC_DESCRIPTOR_BYTES {
        return Err(MosaicPayloadError::DescriptorTooLarge(descriptor));
    }
    u32::try_from(descriptor).map_err(|_| MosaicPayloadError::LengthOverflow)
}

fn validate_metadata_lengths(
    make: u32,
    model: u32,
    cfa: u32,
    black: u32,
    white: u32,
) -> Result<(), MosaicPayloadError> {
    if make > MAX_STRING_BYTES || model > MAX_STRING_BYTES {
        return Err(MosaicPayloadError::Invalid("make/model exceeds limit"));
    }
    if cfa > MAX_METADATA_VALUES || black > MAX_METADATA_VALUES || white > MAX_METADATA_VALUES {
        return Err(MosaicPayloadError::Invalid("metadata vector exceeds limit"));
    }
    Ok(())
}

fn read_utf8(reader: &mut impl Read, len: u32) -> Result<String, MosaicPayloadError> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(len as usize)
        .map_err(|_| MosaicPayloadError::AllocationFailed)?;
    bytes.resize(len as usize, 0);
    reader.read_exact(&mut bytes)?;
    String::from_utf8(bytes).map_err(|_| MosaicPayloadError::Invalid("make/model is not UTF-8"))
}

fn read_cfa_cells(reader: &mut impl Read, count: u32) -> Result<Vec<CfaColor>, MosaicPayloadError> {
    let count = count as usize;
    let mut cells = Vec::new();
    cells
        .try_reserve_exact(count)
        .map_err(|_| MosaicPayloadError::AllocationFailed)?;
    let mut buffer = [0_u8; METADATA_IO_BUFFER_BYTES];
    let mut remaining = count;
    while remaining != 0 {
        let chunk_bytes = remaining.min(buffer.len());
        reader.read_exact(&mut buffer[..chunk_bytes])?;
        for byte in &buffer[..chunk_bytes] {
            cells.push(decode_cfa_color(*byte)?);
        }
        remaining -= chunk_bytes;
    }
    Ok(cells)
}

fn write_f32_values(writer: &mut impl Write, values: &[f32]) -> Result<(), MosaicPayloadError> {
    let mut buffer = [0_u8; METADATA_IO_BUFFER_BYTES];
    let values_per_chunk = buffer.len() / size_of::<f32>();
    for chunk in values.chunks(values_per_chunk) {
        let bytes = &mut buffer[..std::mem::size_of_val(chunk)];
        for (value, output) in chunk.iter().zip(bytes.chunks_exact_mut(size_of::<f32>())) {
            output.copy_from_slice(&canonical_f32_bits(*value)?.to_le_bytes());
        }
        writer.write_all(bytes)?;
    }
    Ok(())
}

fn read_f32_values(reader: &mut impl Read, count: u32) -> Result<Vec<f32>, MosaicPayloadError> {
    let count = count as usize;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| MosaicPayloadError::AllocationFailed)?;
    let mut buffer = [0_u8; METADATA_IO_BUFFER_BYTES];
    let values_per_chunk = buffer.len() / size_of::<f32>();
    let mut remaining = count;
    while remaining != 0 {
        let chunk_values = remaining.min(values_per_chunk);
        let bytes = &mut buffer[..chunk_values * size_of::<f32>()];
        reader.read_exact(bytes)?;
        for encoded in bytes.chunks_exact(size_of::<f32>()) {
            values.push(decode_f32_bits(u32::from_le_bytes(
                encoded.try_into().expect("four-byte f32 chunk"),
            ))?);
        }
        remaining -= chunk_values;
    }
    Ok(values)
}

fn write_pixels(writer: &mut impl Write, pixels: &[u16]) -> Result<(), MosaicPayloadError> {
    let mut buffer = fallible_zeroed_bytes(PIXEL_BUFFER_BYTES)?;
    for samples in pixels.chunks(PIXEL_BUFFER_BYTES / 2) {
        let bytes = &mut buffer[..samples.len() * 2];
        for (sample, output) in samples.iter().zip(bytes.chunks_exact_mut(2)) {
            output.copy_from_slice(&sample.to_le_bytes());
        }
        writer.write_all(bytes)?;
    }
    Ok(())
}

fn read_pixels(reader: &mut impl Read, sample_count: u64) -> Result<Vec<u16>, MosaicPayloadError> {
    let count = usize::try_from(sample_count).map_err(|_| MosaicPayloadError::LengthOverflow)?;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(count)
        .map_err(|_| MosaicPayloadError::AllocationFailed)?;
    let mut buffer = fallible_zeroed_bytes(PIXEL_BUFFER_BYTES)?;
    let mut remaining = count;
    while remaining != 0 {
        let samples = remaining.min(buffer.len() / 2);
        let bytes = &mut buffer[..samples * 2];
        reader.read_exact(bytes)?;
        pixels.extend(
            bytes
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]])),
        );
        remaining -= samples;
    }
    Ok(pixels)
}

fn fallible_zeroed_bytes(len: usize) -> Result<Box<[u8]>, MosaicPayloadError> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(len)
        .map_err(|_| MosaicPayloadError::AllocationFailed)?;
    bytes.resize(len, 0);
    Ok(bytes.into_boxed_slice())
}

fn write_rect(header: &mut [u8; MOSAIC_PAYLOAD_HEADER_V1_BYTES], offset: usize, rect: Option<Rect>) {
    if let Some(rect) = rect {
        for (index, value) in [rect.x, rect.y, rect.width, rect.height].into_iter().enumerate() {
            put_u32(header, offset + index * 4, value);
        }
    }
}

fn read_rect(
    header: &[u8; MOSAIC_PAYLOAD_HEADER_V1_BYTES],
    offset: usize,
    present: bool,
) -> Result<Option<Rect>, MosaicPayloadError> {
    let values = [
        read_u32(header, offset),
        read_u32(header, offset + 4),
        read_u32(header, offset + 8),
        read_u32(header, offset + 12),
    ];
    if !present {
        if values != [0; 4] {
            return Err(MosaicPayloadError::Invalid(
                "absent rectangle must use canonical zeros",
            ));
        }
        return Ok(None);
    }
    Ok(Some(Rect::new(values[0], values[1], values[2], values[3])))
}

fn encode_photometric(value: &Photometric) -> u8 {
    match value {
        Photometric::Cfa => 1,
        Photometric::LinearRaw => 2,
        Photometric::BlackIsZero => 3,
    }
}

fn decode_photometric(value: u8) -> Result<Photometric, MosaicPayloadError> {
    match value {
        1 => Ok(Photometric::Cfa),
        2 => Ok(Photometric::LinearRaw),
        3 => Ok(Photometric::BlackIsZero),
        _ => Err(MosaicPayloadError::Invalid("unknown photometric code")),
    }
}

fn encode_orientation(value: Orientation) -> u8 {
    match value {
        Orientation::Unknown => 0,
        Orientation::Normal => 1,
        Orientation::HorizontalFlip => 2,
        Orientation::Rotate180 => 3,
        Orientation::VerticalFlip => 4,
        Orientation::Transpose => 5,
        Orientation::Rotate90 => 6,
        Orientation::Transverse => 7,
        Orientation::Rotate270 => 8,
    }
}

fn decode_orientation(value: u8) -> Result<Orientation, MosaicPayloadError> {
    match value {
        0 => Ok(Orientation::Unknown),
        1 => Ok(Orientation::Normal),
        2 => Ok(Orientation::HorizontalFlip),
        3 => Ok(Orientation::Rotate180),
        4 => Ok(Orientation::VerticalFlip),
        5 => Ok(Orientation::Transpose),
        6 => Ok(Orientation::Rotate90),
        7 => Ok(Orientation::Transverse),
        8 => Ok(Orientation::Rotate270),
        _ => Err(MosaicPayloadError::Invalid("unknown orientation code")),
    }
}

fn encode_cfa_color(value: CfaColor) -> u8 {
    match value {
        CfaColor::Red => 0,
        CfaColor::Green => 1,
        CfaColor::Blue => 2,
        CfaColor::Cyan => 3,
        CfaColor::Magenta => 4,
        CfaColor::Yellow => 5,
        CfaColor::White => 6,
        CfaColor::Unknown => 255,
    }
}

fn decode_cfa_color(value: u8) -> Result<CfaColor, MosaicPayloadError> {
    match value {
        0 => Ok(CfaColor::Red),
        1 => Ok(CfaColor::Green),
        2 => Ok(CfaColor::Blue),
        3 => Ok(CfaColor::Cyan),
        4 => Ok(CfaColor::Magenta),
        5 => Ok(CfaColor::Yellow),
        6 => Ok(CfaColor::White),
        255 => Ok(CfaColor::Unknown),
        _ => Err(MosaicPayloadError::Invalid("unknown CFA color code")),
    }
}

fn canonical_f32_bits(value: f32) -> Result<u32, MosaicPayloadError> {
    if !value.is_finite() {
        return Err(MosaicPayloadError::Invalid("non-finite float"));
    }
    Ok(if value == 0.0 { 0 } else { value.to_bits() })
}

fn decode_f32_bits(bits: u32) -> Result<f32, MosaicPayloadError> {
    if bits == (-0.0_f32).to_bits() {
        return Err(MosaicPayloadError::Invalid("non-canonical negative zero"));
    }
    let value = f32::from_bits(bits);
    if !value.is_finite() {
        return Err(MosaicPayloadError::Invalid("non-finite float"));
    }
    Ok(value)
}

fn put_u16(bytes: &mut [u8; MOSAIC_PAYLOAD_HEADER_V1_BYTES], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8; MOSAIC_PAYLOAD_HEADER_V1_BYTES], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8; MOSAIC_PAYLOAD_HEADER_V1_BYTES], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn put_f32(
    bytes: &mut [u8; MOSAIC_PAYLOAD_HEADER_V1_BYTES],
    offset: usize,
    value: f32,
) -> Result<(), MosaicPayloadError> {
    put_u32(bytes, offset, canonical_f32_bits(value)?);
    Ok(())
}

fn read_u16(bytes: &[u8; MOSAIC_PAYLOAD_HEADER_V1_BYTES], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("fixed payload header"),
    )
}

fn read_u32(bytes: &[u8; MOSAIC_PAYLOAD_HEADER_V1_BYTES], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("fixed payload header"),
    )
}

fn read_u64(bytes: &[u8; MOSAIC_PAYLOAD_HEADER_V1_BYTES], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes[offset..offset + 8]
            .try_into()
            .expect("fixed payload header"),
    )
}

fn read_f32(bytes: &[u8; MOSAIC_PAYLOAD_HEADER_V1_BYTES], offset: usize) -> Result<f32, MosaicPayloadError> {
    decode_f32_bits(read_u32(bytes, offset))
}

fn len_u32(value: usize) -> Result<u32, MosaicPayloadError> {
    u32::try_from(value).map_err(|_| MosaicPayloadError::LengthOverflow)
}

#[derive(Debug, Error)]
pub enum MosaicPayloadError {
    #[error("mosaic payload I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid mosaic payload: {0}")]
    Invalid(&'static str),
    #[error("unsupported mosaic payload version {major}.{minor}")]
    UnsupportedVersion { major: u16, minor: u16 },
    #[error("mosaic payload length arithmetic overflow")]
    LengthOverflow,
    #[error("mosaic descriptor is too large: {0} bytes")]
    DescriptorTooLarge(u64),
    #[error("mosaic has too many samples: {0}")]
    TooManySamples(u64),
    #[error("mosaic metadata needs {requested} bytes, exceeding the operation budget {limit}")]
    MetadataBudgetExceeded { requested: u64, limit: u64 },
    #[error("mosaic needs {requested} samples, exceeding the operation budget {limit}")]
    SampleBudgetExceeded { requested: u64, limit: u64 },
    #[error("mosaic payload length mismatch: expected {expected}, got {actual}")]
    PayloadLengthMismatch { expected: u64, actual: u64 },
    #[error("mosaic payload allocation failed")]
    AllocationFailed,
    #[error("invalid decoded mosaic: {0}")]
    InvalidFrame(#[from] FrameError),
}

#[cfg(test)]
mod tests {
    use std::io::{self, Read, Write};

    use super::*;

    #[derive(Debug, Default)]
    struct CountingWriter {
        bytes: Vec<u8>,
        writes: usize,
    }

    impl Write for CountingWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            if !buffer.is_empty() {
                self.writes += 1;
                self.bytes.extend_from_slice(buffer);
            }
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[derive(Debug)]
    struct CountingReader<'a> {
        bytes: &'a [u8],
        offset: usize,
        reads: usize,
    }

    impl<'a> CountingReader<'a> {
        const fn new(bytes: &'a [u8]) -> Self {
            Self {
                bytes,
                offset: 0,
                reads: 0,
            }
        }
    }

    impl Read for CountingReader<'_> {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            if output.is_empty() {
                return Ok(0);
            }
            let remaining = &self.bytes[self.offset..];
            if remaining.is_empty() {
                return Ok(0);
            }
            self.reads += 1;
            let read = remaining.len().min(output.len());
            output[..read].copy_from_slice(&remaining[..read]);
            self.offset += read;
            Ok(read)
        }
    }

    fn mosaic() -> DecodedMosaic {
        DecodedMosaic::new(
            RawMetadata {
                make: "Камера\0Test".into(),
                model: String::new(),
                width: 2,
                height: 2,
                components_per_pixel: 1,
                bits_per_sample: 14,
                photometric: Photometric::Cfa,
                cfa: Some(CfaPattern {
                    width: 2,
                    height: 2,
                    cells: vec![CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue],
                }),
                black_level: LevelGrid {
                    width: 2,
                    height: 2,
                    components: 1,
                    values: vec![512.0, -0.0, 514.0, 515.0],
                },
                white_level: WhiteLevel(vec![16_383.0, 16_382.0, 16_381.0]),
                white_balance: [2.0, 1.0, 1.5, 1.0],
                xyz_to_camera: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, 0.0]],
                active_area: Some(Rect::new(0, 0, 2, 2)),
                crop_area: Some(Rect::new(1, 1, 0, 0)),
                orientation: Orientation::Rotate90,
            },
            Arc::new(vec![1, 2, 3, u16::MAX]),
        )
        .unwrap()
    }

    #[test]
    fn maximum_f32_vector_uses_bounded_io_and_preserves_canonical_bytes() {
        let values = (0..MAX_METADATA_VALUES)
            .map(|index| {
                if index == 0 {
                    -0.0
                } else {
                    f32::from(u16::try_from(index).unwrap()) + 0.25
                }
            })
            .collect::<Vec<_>>();
        let mut expected = Vec::with_capacity(values.len() * size_of::<f32>());
        for value in &values {
            expected.extend_from_slice(&canonical_f32_bits(*value).unwrap().to_le_bytes());
        }
        let expected_transfers = expected.len().div_ceil(METADATA_IO_BUFFER_BYTES);

        let mut writer = CountingWriter::default();
        write_f32_values(&mut writer, &values).unwrap();
        assert_eq!(writer.bytes, expected);
        assert_eq!(writer.writes, expected_transfers);

        let mut reader = CountingReader::new(&expected);
        let decoded = read_f32_values(&mut reader, MAX_METADATA_VALUES).unwrap();
        assert_eq!(reader.offset, expected.len());
        assert_eq!(reader.reads, expected_transfers);
        assert!(
            decoded
                .iter()
                .zip(&values)
                .all(|(actual, source)| actual.to_bits() == canonical_f32_bits(*source).unwrap())
        );
    }

    #[test]
    fn maximum_cfa_vector_uses_bounded_io_and_round_trips_codes() {
        const CODES: [u8; 8] = [0, 1, 2, 3, 4, 5, 6, 255];
        let encoded = (0..MAX_METADATA_VALUES as usize)
            .map(|index| CODES[index % CODES.len()])
            .collect::<Vec<_>>();
        let expected_transfers = encoded.len().div_ceil(METADATA_IO_BUFFER_BYTES);
        let mut reader = CountingReader::new(&encoded);

        let decoded = read_cfa_cells(&mut reader, MAX_METADATA_VALUES).unwrap();

        assert_eq!(reader.offset, encoded.len());
        assert_eq!(reader.reads, expected_transfers);
        assert_eq!(decoded.len(), encoded.len());
        assert!(
            decoded
                .iter()
                .copied()
                .map(encode_cfa_color)
                .eq(encoded.iter().copied())
        );
    }

    #[test]
    fn binary_payload_round_trip_is_canonical_and_streaming() {
        let expected = mosaic();
        let prepared = prepare_mosaic_payload_v1(&expected).unwrap();
        let planned = prepared.stats();
        let mut encoded = Vec::new();
        let stats = encode_mosaic_payload_v1(&mut encoded, &expected).unwrap();
        assert_eq!(stats, planned);
        let mut prepared_encoded = Vec::new();
        prepared.encode(&mut prepared_encoded).unwrap();
        assert_eq!(prepared_encoded, encoded);
        assert_eq!(encoded.len() as u64, stats.payload_bytes);
        assert_eq!(&encoded[..8], b"RRMPAY1\0");
        assert_eq!(read_u32(encoded[..192].try_into().unwrap(), 12), 192);
        assert_eq!(read_u64(encoded[..192].try_into().unwrap(), 40), 4);
        // The negative zero in the source metadata is canonicalized.
        let black_offset = stats.metadata_prefix_bytes as usize - (4 * 4 + 3 * 4);
        assert_eq!(&encoded[black_offset + 4..black_offset + 8], &[0; 4]);

        let decoded = decode_mosaic_payload_v1(&mut encoded.as_slice(), encoded.len() as u64).unwrap();
        assert_eq!(decoded.metadata.make, expected.metadata.make);
        assert_eq!(decoded.metadata.model, expected.metadata.model);
        assert_eq!(decoded.metadata.cfa, expected.metadata.cfa);
        assert_eq!(decoded.metadata.active_area, expected.metadata.active_area);
        assert_eq!(decoded.metadata.crop_area, expected.metadata.crop_area);
        assert_eq!(decoded.metadata.orientation, expected.metadata.orientation);
        assert_eq!(&*decoded.pixels, &*expected.pixels);
        assert_eq!(
            decoded.metadata.black_level.values[1].to_bits(),
            0.0_f32.to_bits()
        );
        let mut reencoded = Vec::new();
        encode_mosaic_payload_v1(&mut reencoded, &decoded).unwrap();
        assert_eq!(reencoded, encoded);
    }

    #[test]
    fn invalid_lengths_and_noncanonical_float_are_rejected_before_pixels() {
        let mut encoded = Vec::new();
        let stats = encode_mosaic_payload_v1(&mut encoded, &mosaic()).unwrap();

        let mut invalid = encoded.clone();
        invalid[40..48].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(matches!(
            decode_mosaic_payload_v1(&mut invalid.as_slice(), invalid.len() as u64),
            Err(MosaicPayloadError::TooManySamples(u64::MAX))
        ));

        let mut invalid = encoded;
        invalid[128..132].copy_from_slice(&(-0.0_f32).to_bits().to_le_bytes());
        assert!(matches!(
            decode_mosaic_payload_v1(&mut invalid.as_slice(), stats.payload_bytes),
            Err(MosaicPayloadError::Invalid("non-canonical negative zero"))
        ));
    }

    #[test]
    fn writer_and_reader_enforce_identical_metadata_limits() {
        let mut accepted = mosaic();
        accepted.metadata.make = "a".repeat(MAX_STRING_BYTES as usize);
        accepted.metadata.white_level = WhiteLevel(vec![1.0; MAX_METADATA_VALUES as usize]);
        let mut encoded = Vec::new();
        let stats = encode_mosaic_payload_v1(&mut encoded, &accepted).unwrap();
        let decoded = decode_mosaic_payload_v1(&mut encoded.as_slice(), stats.payload_bytes).unwrap();
        assert_eq!(decoded.metadata.make.len(), MAX_STRING_BYTES as usize);
        assert_eq!(decoded.metadata.white_level.0.len(), MAX_METADATA_VALUES as usize);

        let mut oversized_string = mosaic();
        oversized_string.metadata.make = "a".repeat(MAX_STRING_BYTES as usize + 1);
        assert!(matches!(
            encode_mosaic_payload_v1(&mut Vec::new(), &oversized_string),
            Err(MosaicPayloadError::Invalid("make/model exceeds limit"))
        ));

        let mut oversized_vector = mosaic();
        oversized_vector.metadata.white_level = WhiteLevel(vec![1.0; MAX_METADATA_VALUES as usize + 1]);
        assert!(matches!(
            encode_mosaic_payload_v1(&mut Vec::new(), &oversized_vector),
            Err(MosaicPayloadError::Invalid("metadata vector exceeds limit"))
        ));
    }

    #[test]
    fn invalid_metadata_is_rejected_before_any_pixel_read() {
        let mut encoded = Vec::new();
        let stats = encode_mosaic_payload_v1(&mut encoded, &mosaic()).unwrap();
        let white_offset = stats.metadata_prefix_bytes as usize - 3 * size_of::<f32>();
        encoded[white_offset..white_offset + 4].copy_from_slice(&0_f32.to_le_bytes());
        encoded.truncate(stats.metadata_prefix_bytes as usize);

        assert!(matches!(
            decode_mosaic_payload_v1(&mut encoded.as_slice(), stats.payload_bytes),
            Err(MosaicPayloadError::InvalidFrame(FrameError::NonFiniteMetadata(
                "white level"
            )))
        ));
    }

    #[test]
    fn operation_budget_rejects_before_variable_sections_are_read() {
        let mut encoded = Vec::new();
        let plan = encode_mosaic_payload_v1(&mut encoded, &mosaic()).unwrap();
        let header_only = &encoded[..MOSAIC_PAYLOAD_HEADER_V1_BYTES];
        let mut sample_limited = header_only;

        assert!(matches!(
            decode_mosaic_payload_v1_with_limits(
                &mut sample_limited,
                plan.payload_bytes,
                MosaicDecodeLimits::new(u64::from(plan.metadata_prefix_bytes), 3),
            ),
            Err(MosaicPayloadError::SampleBudgetExceeded {
                requested: 4,
                limit: 3
            })
        ));
        let mut metadata_limited = header_only;
        assert!(matches!(
            decode_mosaic_payload_v1_with_limits(
                &mut metadata_limited,
                plan.payload_bytes,
                MosaicDecodeLimits::new(u64::from(plan.metadata_prefix_bytes) - 1, 4),
            ),
            Err(MosaicPayloadError::MetadataBudgetExceeded { .. })
        ));
    }

    #[test]
    fn every_truncation_and_one_trailing_byte_are_rejected() {
        let mut encoded = Vec::new();
        let plan = encode_mosaic_payload_v1(&mut encoded, &mosaic()).unwrap();
        for truncated_len in 0..encoded.len() {
            let mut truncated = &encoded[..truncated_len];
            assert!(
                decode_mosaic_payload_v1(&mut truncated, plan.payload_bytes).is_err(),
                "accepted truncation at {truncated_len}"
            );
        }

        let mut with_trailing = encoded;
        with_trailing.push(0);
        assert!(matches!(
            decode_mosaic_payload_v1(&mut with_trailing.as_slice(), plan.payload_bytes + 1),
            Err(MosaicPayloadError::PayloadLengthMismatch { .. })
        ));
    }

    #[test]
    fn registries_and_float_bit_rules_are_closed_and_canonical() {
        let orientations = [
            Orientation::Unknown,
            Orientation::Normal,
            Orientation::HorizontalFlip,
            Orientation::Rotate180,
            Orientation::VerticalFlip,
            Orientation::Transpose,
            Orientation::Rotate90,
            Orientation::Transverse,
            Orientation::Rotate270,
        ];
        for orientation in orientations {
            assert_eq!(
                decode_orientation(encode_orientation(orientation)).unwrap(),
                orientation
            );
        }
        for code in 9..=u8::MAX {
            assert!(decode_orientation(code).is_err(), "orientation code {code}");
        }

        let photometrics = [Photometric::Cfa, Photometric::LinearRaw, Photometric::BlackIsZero];
        for photometric in photometrics {
            assert_eq!(
                decode_photometric(encode_photometric(&photometric)).unwrap(),
                photometric
            );
        }
        for code in [0, 4, u8::MAX] {
            assert!(decode_photometric(code).is_err(), "photometric code {code}");
        }

        let colors = [
            CfaColor::Red,
            CfaColor::Green,
            CfaColor::Blue,
            CfaColor::Cyan,
            CfaColor::Magenta,
            CfaColor::Yellow,
            CfaColor::White,
            CfaColor::Unknown,
        ];
        for color in colors {
            assert_eq!(decode_cfa_color(encode_cfa_color(color)).unwrap(), color);
        }
        for code in 7..u8::MAX {
            assert!(decode_cfa_color(code).is_err(), "CFA code {code}");
        }

        for value in [f32::MIN_POSITIVE, f32::from_bits(1), -f32::from_bits(1), f32::MAX] {
            let bits = canonical_f32_bits(value).unwrap();
            assert_eq!(decode_f32_bits(bits).unwrap().to_bits(), value.to_bits());
        }
        assert_eq!(canonical_f32_bits(-0.0).unwrap(), 0);
        for value in [f32::INFINITY, f32::NEG_INFINITY, f32::NAN] {
            assert!(canonical_f32_bits(value).is_err());
        }
        assert!(decode_f32_bits((-0.0_f32).to_bits()).is_err());
    }

    #[test]
    fn invalid_utf8_and_unknown_registry_codes_fail_before_pixels() {
        let mut encoded = Vec::new();
        let plan = encode_mosaic_payload_v1(&mut encoded, &mosaic()).unwrap();

        let mut invalid_utf8 = encoded.clone();
        invalid_utf8[MOSAIC_PAYLOAD_HEADER_V1_BYTES] = 0xff;
        assert!(matches!(
            decode_mosaic_payload_v1(&mut invalid_utf8.as_slice(), plan.payload_bytes),
            Err(MosaicPayloadError::Invalid("make/model is not UTF-8"))
        ));

        let mut invalid_orientation = encoded.clone();
        invalid_orientation[79] = 9;
        assert!(matches!(
            decode_mosaic_payload_v1(&mut invalid_orientation.as_slice(), plan.payload_bytes),
            Err(MosaicPayloadError::Invalid("unknown orientation code"))
        ));

        let mut invalid_cfa = encoded;
        let cfa_offset = MOSAIC_PAYLOAD_HEADER_V1_BYTES + mosaic().metadata.make.len();
        invalid_cfa[cfa_offset] = 7;
        assert!(matches!(
            decode_mosaic_payload_v1(&mut invalid_cfa.as_slice(), plan.payload_bytes),
            Err(MosaicPayloadError::Invalid("unknown CFA color code"))
        ));
    }
}
