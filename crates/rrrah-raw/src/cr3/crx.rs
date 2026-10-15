//! Canon CRX configuration and sample framing.
//!
//! This module deliberately stops at framing. It does not interpret the CRX
//! entropy stream. The mapped fields below were established from the two EOS
//! R8 files in the local fixture set and from black-box decoder diagnostics;
//! fields without sufficiently strong evidence remain available as raw bytes.

use thiserror::Error;

pub const CRX_PLANE_COUNT: usize = 4;
pub const CRX_SAMPLE_HEADER_LEN: usize = 112;

const CMP1: [u8; 4] = *b"CMP1";
const CDI1: [u8; 4] = *b"CDI1";
const IAD1: [u8; 4] = *b"IAD1";

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CrxError {
    #[error("{context} is truncated: need {needed} bytes, have {available}")]
    Truncated {
        context: &'static str,
        needed: usize,
        available: usize,
    },
    #[error("{context} has box type {actual:?}, expected {expected:?}")]
    UnexpectedBoxType {
        context: &'static str,
        expected: [u8; 4],
        actual: [u8; 4],
    },
    #[error("{context} declares {declared} bytes, but its bounded slice has {actual}")]
    InvalidBoxSize {
        context: &'static str,
        declared: usize,
        actual: usize,
    },
    #[error("{context} uses an unsupported extended or open-ended box size")]
    UnsupportedBoxSize { context: &'static str },
    #[error("invalid CRX field: {field}")]
    InvalidField { field: &'static str },
    #[error("CRX arithmetic overflow while computing {context}")]
    ArithmeticOverflow { context: &'static str },
    #[error("CMP1 dimensions {cmp_width}x{cmp_height} do not match IAD1 dimensions {iad_width}x{iad_height}")]
    DimensionMismatch {
        cmp_width: u32,
        cmp_height: u32,
        iad_width: u16,
        iad_height: u16,
    },
    #[error("CRX sample has {actual} bytes; its header requires exactly {expected}")]
    InvalidSampleSize { expected: usize, actual: usize },
    #[error("CRX marker at byte {offset} is {actual:?}, expected {expected:?}")]
    UnexpectedMarker {
        offset: usize,
        expected: [u8; 2],
        actual: [u8; 2],
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cmp1 {
    /// Low nibble of the leading `0xff` configuration byte. It is 15 in both
    /// EOS R8 tracks.
    pub sample_precision: u8,
    /// Big-endian `0x0100` in the observed configuration.
    pub version: u16,
    pub image_width: u32,
    pub image_height: u32,
    pub tile_width: u32,
    pub tile_height: u32,
    pub n_bits: u8,
    pub plane_count: u8,
    pub sample_header_size: u32,
    /// Unmapped bytes adjacent to `n_bits` and `plane_count`.
    pub format_tail: [u8; 2],
    /// Four observed per-plane records. Their individual bytes are deliberately
    /// left uninterpreted until more than one camera/configuration is available.
    pub raw_plane_configs: [[u8; 4]; CRX_PLANE_COUNT],
}

impl Cmp1 {
    pub fn parse_box(bytes: &[u8]) -> Result<Self, CrxError> {
        let payload = exact_box_payload(bytes, CMP1, "CMP1")?;
        if payload.len() != 52 {
            return Err(CrxError::InvalidBoxSize {
                context: "CMP1 payload",
                declared: 52,
                actual: payload.len(),
            });
        }

        let mut cursor = Cursor::new(payload);
        let internal_header = cursor.array::<4>("CMP1 internal header")?;
        if internal_header[0] != 0xff
            || internal_header[1] != 0
            || internal_header[2] != 0
            || internal_header[3] != 0x30
        {
            return Err(CrxError::InvalidField {
                field: "CMP1 internal header",
            });
        }

        let version_and_mode = cursor.array::<4>("CMP1 version/mode")?;
        let version = u16::from_be_bytes([version_and_mode[0], version_and_mode[1]]);
        if version_and_mode[2..] != [0, 0] {
            return Err(CrxError::InvalidField {
                field: "CMP1 version/mode tail",
            });
        }

        let image_width = cursor.be_u32("CMP1 image width")?;
        let image_height = cursor.be_u32("CMP1 image height")?;
        let tile_width = cursor.be_u32("CMP1 tile width")?;
        let tile_height = cursor.be_u32("CMP1 tile height")?;
        if image_width == 0 || image_height == 0 || tile_width == 0 || tile_height == 0 {
            return Err(CrxError::InvalidField {
                field: "CMP1 zero geometry",
            });
        }

        let format = cursor.array::<4>("CMP1 format")?;
        let n_bits = format[0];
        let plane_count = format[1] >> 4;
        let format_tail = [format[2], format[3]];
        if n_bits == 0
            || n_bits > 16
            || plane_count != u8::try_from(CRX_PLANE_COUNT).unwrap_or(0)
            || format[1] & 0x0f != 0
            || format_tail != [0, 0]
        {
            return Err(CrxError::InvalidField {
                field: "CMP1 bit depth/plane count",
            });
        }

        let sample_header_size = cursor.be_u32("CMP1 sample header size")?;
        if usize::try_from(sample_header_size).ok() != Some(CRX_SAMPLE_HEADER_LEN) {
            return Err(CrxError::InvalidField {
                field: "CMP1 sample header size",
            });
        }
        if cursor.be_u32("CMP1 reserved word")? != 0 {
            return Err(CrxError::InvalidField {
                field: "CMP1 reserved word",
            });
        }

        let raw_plane_configs = [
            cursor.array::<4>("CMP1 plane 0 config")?,
            cursor.array::<4>("CMP1 plane 1 config")?,
            cursor.array::<4>("CMP1 plane 2 config")?,
            cursor.array::<4>("CMP1 plane 3 config")?,
        ];
        if raw_plane_configs.iter().any(|config| *config != [1, 1, 0, 0]) {
            return Err(CrxError::InvalidField {
                field: "CMP1 plane config",
            });
        }
        if !cursor.is_empty() {
            return Err(CrxError::InvalidField {
                field: "CMP1 trailing payload",
            });
        }

        Ok(Self {
            sample_precision: internal_header[0] & 0x0f,
            version,
            image_width,
            image_height,
            tile_width,
            tile_height,
            n_bits,
            plane_count,
            sample_header_size,
            format_tail,
            raw_plane_configs,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InclusiveRect {
    pub left: u16,
    pub top: u16,
    pub right: u16,
    pub bottom: u16,
}

impl InclusiveRect {
    fn geometry(self) -> RectGeometry {
        RectGeometry {
            x: u32::from(self.left),
            y: u32::from(self.top),
            width: u32::from(self.right) - u32::from(self.left) + 1,
            height: u32::from(self.bottom) - u32::from(self.top) + 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RectGeometry {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SensorGeometry {
    pub active_area: RectGeometry,
    pub crop_area: RectGeometry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Iad1 {
    pub image_width: u16,
    pub image_height: u16,
    /// Observed as 1 in both EOS R8 tracks; semantic name intentionally
    /// withheld.
    pub empirical_layout_tag: u16,
    /// Controls the number of following rectangles in the observed layouts:
    /// total rectangle count is `2 + value`.
    pub empirical_additional_rectangles: u16,
    pub empirical_header_tail: [u16; 2],
    pub rectangles: Vec<InclusiveRect>,
}

impl Iad1 {
    pub fn parse_box(bytes: &[u8]) -> Result<Self, CrxError> {
        let payload = exact_box_payload(bytes, IAD1, "IAD1")?;
        let mut cursor = Cursor::new(payload);
        if cursor.be_u32("IAD1 full-box flags")? != 0 {
            return Err(CrxError::InvalidField {
                field: "IAD1 full-box flags",
            });
        }

        let image_width = cursor.be_u16("IAD1 image width")?;
        let image_height = cursor.be_u16("IAD1 image height")?;
        let empirical_layout_tag = cursor.be_u16("IAD1 empirical layout tag")?;
        let empirical_additional_rectangles = cursor.be_u16("IAD1 empirical additional rectangle count")?;
        let empirical_header_tail = [
            cursor.be_u16("IAD1 empirical header tail 0")?,
            cursor.be_u16("IAD1 empirical header tail 1")?,
        ];
        if image_width == 0
            || image_height == 0
            || empirical_layout_tag != 1
            || empirical_header_tail != [1, 0]
        {
            return Err(CrxError::InvalidField {
                field: "IAD1 fixed header",
            });
        }

        let rectangle_count = 2usize
            .checked_add(usize::from(empirical_additional_rectangles))
            .ok_or(CrxError::ArithmeticOverflow {
                context: "IAD1 rectangle count",
            })?;
        let rectangle_bytes = rectangle_count
            .checked_mul(8)
            .ok_or(CrxError::ArithmeticOverflow {
                context: "IAD1 rectangle byte count",
            })?;
        if cursor.remaining() != rectangle_bytes {
            return Err(CrxError::InvalidField {
                field: "IAD1 rectangle count/length",
            });
        }

        let mut rectangles = Vec::with_capacity(rectangle_count);
        for _ in 0..rectangle_count {
            let rectangle = InclusiveRect {
                left: cursor.be_u16("IAD1 rectangle left")?,
                top: cursor.be_u16("IAD1 rectangle top")?,
                right: cursor.be_u16("IAD1 rectangle right")?,
                bottom: cursor.be_u16("IAD1 rectangle bottom")?,
            };
            if rectangle.left > rectangle.right
                || rectangle.top > rectangle.bottom
                || rectangle.right >= image_width
                || rectangle.bottom >= image_height
            {
                return Err(CrxError::InvalidField {
                    field: "IAD1 rectangle bounds",
                });
            }
            rectangles.push(rectangle);
        }

        Ok(Self {
            image_width,
            image_height,
            empirical_layout_tag,
            empirical_additional_rectangles,
            empirical_header_tail,
            rectangles,
        })
    }

    /// Geometry is returned only for the exact full-resolution EOS R8 layout
    /// seen in both fixtures. The first rectangle maps to the 6000x4000 crop.
    /// The active-area far-edge adjustment is empirical: it reproduces the
    /// independently observed 6022x4020 metadata contract, and is intentionally
    /// not generalized to another camera or layout.
    pub fn eos_r8_sensor_geometry(&self) -> Option<SensorGeometry> {
        const RECTANGLES: [InclusiveRect; 4] = [
            InclusiveRect {
                left: 168,
                top: 108,
                right: 6_167,
                bottom: 4_107,
            },
            InclusiveRect {
                left: 0,
                top: 0,
                right: 155,
                bottom: 4_117,
            },
            InclusiveRect {
                left: 156,
                top: 0,
                right: 6_179,
                bottom: 95,
            },
            InclusiveRect {
                left: 156,
                top: 96,
                right: 6_179,
                bottom: 4_117,
            },
        ];
        if self.image_width != 6_188
            || self.image_height != 4_120
            || self.rectangles.as_slice() != RECTANGLES.as_slice()
        {
            return None;
        }

        let raw_active = RECTANGLES[3];
        Some(SensorGeometry {
            active_area: RectGeometry {
                x: u32::from(raw_active.left),
                y: u32::from(raw_active.top),
                width: u32::from(raw_active.right) - u32::from(raw_active.left) - 1,
                height: u32::from(raw_active.bottom) - u32::from(raw_active.top) - 1,
            },
            crop_area: RECTANGLES[0].geometry(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cdi1 {
    pub image_description: Iad1,
}

impl Cdi1 {
    pub fn parse_box(bytes: &[u8]) -> Result<Self, CrxError> {
        let payload = exact_box_payload(bytes, CDI1, "CDI1")?;
        if payload.len() < 4 {
            return Err(CrxError::Truncated {
                context: "CDI1 full-box flags",
                needed: 4,
                available: payload.len(),
            });
        }
        if payload[..4] != [0, 0, 0, 0] {
            return Err(CrxError::InvalidField {
                field: "CDI1 full-box flags",
            });
        }
        Ok(Self {
            image_description: Iad1::parse_box(&payload[4..])?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrxConfig {
    pub compression: Cmp1,
    pub image_description: Iad1,
}

impl CrxConfig {
    pub fn parse(cmp1_box: &[u8], cdi1_box: &[u8]) -> Result<Self, CrxError> {
        let compression = Cmp1::parse_box(cmp1_box)?;
        let image_description = Cdi1::parse_box(cdi1_box)?.image_description;
        if compression.image_width != u32::from(image_description.image_width)
            || compression.image_height != u32::from(image_description.image_height)
        {
            return Err(CrxError::DimensionMismatch {
                cmp_width: compression.image_width,
                cmp_height: compression.image_height,
                iad_width: image_description.image_width,
                iad_height: image_description.image_height,
            });
        }
        Ok(Self {
            compression,
            image_description,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrxPlaneChunk<'a> {
    pub plane_index: u8,
    pub quantization_parameter: u8,
    /// The last byte of the observed FF03 descriptor: 0/6/6/2 for the
    /// full-resolution EOS R8 sample and 4/4/7/7 for its reduced track.
    pub empirical_ff03_tail: u8,
    pub raw_ff02_descriptor: [u8; 4],
    pub raw_ff03_descriptor: [u8; 4],
    pub data: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrxSample<'a> {
    pub declared_payload_size: u32,
    pub planes: [CrxPlaneChunk<'a>; CRX_PLANE_COUNT],
}

#[derive(Debug, Clone, Copy)]
struct ParsedSampleHeader {
    declared_payload_size: u32,
    plane_sizes: [u32; CRX_PLANE_COUNT],
    ff02_descriptors: [[u8; 4]; CRX_PLANE_COUNT],
    ff03_descriptors: [[u8; 4]; CRX_PLANE_COUNT],
}

pub fn parse_crx_sample(sample: &[u8]) -> Result<CrxSample<'_>, CrxError> {
    if sample.len() < CRX_SAMPLE_HEADER_LEN {
        return Err(CrxError::Truncated {
            context: "CRX sample header",
            needed: CRX_SAMPLE_HEADER_LEN,
            available: sample.len(),
        });
    }
    let parsed_header = parse_sample_header(&sample[..CRX_SAMPLE_HEADER_LEN])?;

    let declared_payload_len =
        usize::try_from(parsed_header.declared_payload_size).map_err(|_| CrxError::ArithmeticOverflow {
            context: "CRX declared payload size",
        })?;
    let expected_sample_len =
        CRX_SAMPLE_HEADER_LEN
            .checked_add(declared_payload_len)
            .ok_or(CrxError::ArithmeticOverflow {
                context: "CRX sample size",
            })?;
    if sample.len() != expected_sample_len {
        return Err(CrxError::InvalidSampleSize {
            expected: expected_sample_len,
            actual: sample.len(),
        });
    }

    let mut payload_sum = 0usize;
    let mut bounds = [(0usize, 0usize); CRX_PLANE_COUNT];
    let mut plane_start = CRX_SAMPLE_HEADER_LEN;
    for (plane, size) in parsed_header.plane_sizes.iter().copied().enumerate() {
        let size = usize::try_from(size).map_err(|_| CrxError::ArithmeticOverflow {
            context: "CRX plane size",
        })?;
        payload_sum = payload_sum
            .checked_add(size)
            .ok_or(CrxError::ArithmeticOverflow {
                context: "CRX plane-size sum",
            })?;
        let plane_end = plane_start
            .checked_add(size)
            .ok_or(CrxError::ArithmeticOverflow {
                context: "CRX plane boundary",
            })?;
        if plane_end > sample.len() {
            return Err(CrxError::InvalidField {
                field: "CRX plane boundary",
            });
        }
        bounds[plane] = (plane_start, plane_end);
        plane_start = plane_end;
    }
    if payload_sum != declared_payload_len || plane_start != sample.len() {
        return Err(CrxError::InvalidField {
            field: "CRX plane-size sum",
        });
    }

    let plane = |index: usize, plane_index: u8| {
        let descriptor = parsed_header.ff03_descriptors[index];
        CrxPlaneChunk {
            plane_index,
            quantization_parameter: descriptor[1] >> 3,
            empirical_ff03_tail: descriptor[3],
            raw_ff02_descriptor: parsed_header.ff02_descriptors[index],
            raw_ff03_descriptor: descriptor,
            data: &sample[bounds[index].0..bounds[index].1],
        }
    };
    Ok(CrxSample {
        declared_payload_size: parsed_header.declared_payload_size,
        planes: [plane(0, 0), plane(1, 1), plane(2, 2), plane(3, 3)],
    })
}

fn parse_sample_header(header: &[u8]) -> Result<ParsedSampleHeader, CrxError> {
    let ff01 = marker_payload(header, 0, [0xff, 0x01])?;
    let declared_payload_size = u32::from_be_bytes([ff01[0], ff01[1], ff01[2], ff01[3]]);
    if ff01[4..] != [0, 0, 0, 0] {
        return Err(CrxError::InvalidField {
            field: "FF01 reserved word",
        });
    }

    let mut plane_sizes = [0_u32; CRX_PLANE_COUNT];
    let mut ff02_descriptors = [[0_u8; 4]; CRX_PLANE_COUNT];
    let mut ff03_descriptors = [[0_u8; 4]; CRX_PLANE_COUNT];
    for plane in 0..CRX_PLANE_COUNT {
        let pair_offset = 12usize
            .checked_add(plane.checked_mul(24).ok_or(CrxError::ArithmeticOverflow {
                context: "CRX marker-pair offset",
            })?)
            .ok_or(CrxError::ArithmeticOverflow {
                context: "CRX marker-pair offset",
            })?;
        let ff02 = marker_payload(header, pair_offset, [0xff, 0x02])?;
        let ff03 = marker_payload(header, pair_offset + 12, [0xff, 0x03])?;
        let ff02_size = u32::from_be_bytes([ff02[0], ff02[1], ff02[2], ff02[3]]);
        let ff03_size = u32::from_be_bytes([ff03[0], ff03[1], ff03[2], ff03[3]]);
        if ff02_size == 0 || ff02_size != ff03_size {
            return Err(CrxError::InvalidField {
                field: "FF02/FF03 plane size",
            });
        }
        plane_sizes[plane] = ff02_size;
        ff02_descriptors[plane].copy_from_slice(&ff02[4..]);
        ff03_descriptors[plane].copy_from_slice(&ff03[4..]);

        let expected_plane_tag = u8::try_from(plane)
            .ok()
            .and_then(|value| value.checked_mul(0x10))
            .and_then(|value| value.checked_add(0x08))
            .ok_or(CrxError::ArithmeticOverflow {
                context: "FF02 plane tag",
            })?;
        if ff02_descriptors[plane] != [expected_plane_tag, 0, 0, 0] {
            return Err(CrxError::InvalidField {
                field: "FF02 plane descriptor",
            });
        }
        if ff03_descriptors[plane][0] != 0
            || ff03_descriptors[plane][1] != 0x20
            || ff03_descriptors[plane][2] != 0
        {
            return Err(CrxError::InvalidField {
                field: "FF03 plane descriptor",
            });
        }
    }
    if header[108..] != [0, 0, 0, 0] {
        return Err(CrxError::InvalidField {
            field: "CRX sample header padding",
        });
    }
    Ok(ParsedSampleHeader {
        declared_payload_size,
        plane_sizes,
        ff02_descriptors,
        ff03_descriptors,
    })
}

fn exact_box_payload<'a>(
    bytes: &'a [u8],
    expected_type: [u8; 4],
    context: &'static str,
) -> Result<&'a [u8], CrxError> {
    if bytes.len() < 8 {
        return Err(CrxError::Truncated {
            context,
            needed: 8,
            available: bytes.len(),
        });
    }
    let declared_u32 = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if declared_u32 <= 1 {
        return Err(CrxError::UnsupportedBoxSize { context });
    }
    let declared =
        usize::try_from(declared_u32).map_err(|_| CrxError::ArithmeticOverflow { context: "box size" })?;
    if declared != bytes.len() {
        return Err(CrxError::InvalidBoxSize {
            context,
            declared,
            actual: bytes.len(),
        });
    }
    let actual_type = [bytes[4], bytes[5], bytes[6], bytes[7]];
    if actual_type != expected_type {
        return Err(CrxError::UnexpectedBoxType {
            context,
            expected: expected_type,
            actual: actual_type,
        });
    }
    Ok(&bytes[8..])
}

fn marker_payload(header: &[u8], offset: usize, expected_marker: [u8; 2]) -> Result<[u8; 8], CrxError> {
    let end = offset.checked_add(12).ok_or(CrxError::ArithmeticOverflow {
        context: "CRX marker segment",
    })?;
    let segment = header.get(offset..end).ok_or(CrxError::Truncated {
        context: "CRX marker segment",
        needed: end,
        available: header.len(),
    })?;
    let actual_marker = [segment[0], segment[1]];
    if actual_marker != expected_marker {
        return Err(CrxError::UnexpectedMarker {
            offset,
            expected: expected_marker,
            actual: actual_marker,
        });
    }
    if u16::from_be_bytes([segment[2], segment[3]]) != 8 {
        return Err(CrxError::InvalidField {
            field: "CRX marker payload length",
        });
    }
    let mut payload = [0_u8; 8];
    payload.copy_from_slice(&segment[4..]);
    Ok(payload)
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    fn array<const N: usize>(&mut self, context: &'static str) -> Result<[u8; N], CrxError> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or(CrxError::ArithmeticOverflow { context })?;
        let bytes = self.bytes.get(self.offset..end).ok_or(CrxError::Truncated {
            context,
            needed: end,
            available: self.bytes.len(),
        })?;
        let mut output = [0_u8; N];
        output.copy_from_slice(bytes);
        self.offset = end;
        Ok(output)
    }

    fn be_u16(&mut self, context: &'static str) -> Result<u16, CrxError> {
        Ok(u16::from_be_bytes(self.array::<2>(context)?))
    }

    fn be_u32(&mut self, context: &'static str) -> Result<u32, CrxError> {
        Ok(u32::from_be_bytes(self.array::<4>(context)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EOS_R8_PREVIEW_CMP1: [u8; 60] = [
        0, 0, 0, 60, b'C', b'M', b'P', b'1', 0xff, 0, 0, 0x30, 1, 0, 0, 0, 0, 0, 6, 0x58, 0, 0, 4, 0x38, 0,
        0, 6, 0x58, 0, 0, 4, 0x38, 0x0e, 0x40, 0, 0, 0, 0, 0, 0x70, 0, 0, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1,
        0, 0, 1, 1, 0, 0,
    ];

    const EOS_R8_MAIN_CMP1: [u8; 60] = [
        0, 0, 0, 60, b'C', b'M', b'P', b'1', 0xff, 0, 0, 0x30, 1, 0, 0, 0, 0, 0, 0x18, 0x2c, 0, 0, 0x10,
        0x18, 0, 0, 0x18, 0x2c, 0, 0, 0x10, 0x18, 0x0e, 0x40, 0, 0, 0, 0, 0, 0x70, 0, 0, 0, 0, 1, 1, 0, 0, 1,
        1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0,
    ];

    const EOS_R8_PREVIEW_CDI1: [u8; 52] = [
        0, 0, 0, 52, b'C', b'D', b'I', b'1', 0, 0, 0, 0, 0, 0, 0, 40, b'I', b'A', b'D', b'1', 0, 0, 0, 0, 6,
        0x58, 4, 0x38, 0, 1, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 6, 0x55, 4, 0x37, 0, 0, 0, 0, 6, 0x57, 4, 0x37,
    ];

    const EOS_R8_MAIN_CDI1: [u8; 68] = [
        0, 0, 0, 68, b'C', b'D', b'I', b'1', 0, 0, 0, 0, 0, 0, 0, 56, b'I', b'A', b'D', b'1', 0, 0, 0, 0,
        0x18, 0x2c, 0x10, 0x18, 0, 1, 0, 2, 0, 1, 0, 0, 0, 0xa8, 0, 0x6c, 0x18, 0x17, 0x10, 0x0b, 0, 0, 0, 0,
        0, 0x9b, 0x10, 0x15, 0, 0x9c, 0, 0, 0x18, 0x23, 0, 0x5f, 0, 0x9c, 0, 0x60, 0x18, 0x23, 0x10, 0x15,
    ];

    #[test]
    fn parses_both_eos_r8_track_configurations() {
        let preview = CrxConfig::parse(&EOS_R8_PREVIEW_CMP1, &EOS_R8_PREVIEW_CDI1).unwrap();
        assert_eq!(
            (preview.compression.image_width, preview.compression.image_height),
            (1_624, 1_080)
        );
        assert_eq!(preview.compression.sample_precision, 15);
        assert_eq!(preview.compression.version, 0x0100);
        assert_eq!(preview.compression.n_bits, 14);
        assert_eq!(preview.compression.plane_count, 4);
        assert_eq!(preview.image_description.rectangles.len(), 2);
        assert_eq!(preview.image_description.eos_r8_sensor_geometry(), None);

        let main = CrxConfig::parse(&EOS_R8_MAIN_CMP1, &EOS_R8_MAIN_CDI1).unwrap();
        assert_eq!(
            (main.compression.image_width, main.compression.image_height),
            (6_188, 4_120)
        );
        assert_eq!(main.compression.tile_width, 6_188);
        assert_eq!(main.compression.tile_height, 4_120);
        assert_eq!(main.image_description.rectangles.len(), 4);
        assert_eq!(
            main.image_description.eos_r8_sensor_geometry(),
            Some(SensorGeometry {
                active_area: RectGeometry {
                    x: 156,
                    y: 96,
                    width: 6_022,
                    height: 4_020,
                },
                crop_area: RectGeometry {
                    x: 168,
                    y: 108,
                    width: 6_000,
                    height: 4_000,
                },
            })
        );
    }

    #[test]
    fn splits_a_small_four_plane_sample_into_bounded_chunks() {
        let sizes = [1_u32, 2, 3, 4];
        let tails = [0_u8, 6, 6, 2];
        let mut sample = vec![0_u8; CRX_SAMPLE_HEADER_LEN + 10];
        write_segment(&mut sample, 0, [0xff, 0x01], 10, [0, 0, 0, 0]);
        for plane in 0..CRX_PLANE_COUNT {
            let offset = 12 + plane * 24;
            let plane_tag = 0x08 + u8::try_from(plane).unwrap() * 0x10;
            write_segment(
                &mut sample,
                offset,
                [0xff, 0x02],
                sizes[plane],
                [plane_tag, 0, 0, 0],
            );
            write_segment(
                &mut sample,
                offset + 12,
                [0xff, 0x03],
                sizes[plane],
                [0, 0x20, 0, tails[plane]],
            );
        }
        sample[CRX_SAMPLE_HEADER_LEN..].copy_from_slice(&[10, 20, 21, 30, 31, 32, 40, 41, 42, 43]);

        let parsed = parse_crx_sample(&sample).unwrap();
        assert_eq!(parsed.declared_payload_size, 10);
        assert_eq!(parsed.planes[0].data, &[10]);
        assert_eq!(parsed.planes[1].data, &[20, 21]);
        assert_eq!(parsed.planes[2].data, &[30, 31, 32]);
        assert_eq!(parsed.planes[3].data, &[40, 41, 42, 43]);
        assert_eq!(
            parsed
                .planes
                .iter()
                .map(|plane| plane.quantization_parameter)
                .collect::<Vec<_>>(),
            vec![4, 4, 4, 4]
        );
        assert_eq!(
            parsed
                .planes
                .iter()
                .map(|plane| plane.empirical_ff03_tail)
                .collect::<Vec<_>>(),
            tails
        );
    }

    #[test]
    fn rejects_unbounded_or_internally_inconsistent_samples() {
        let mut sample = vec![0_u8; CRX_SAMPLE_HEADER_LEN + 4];
        write_segment(&mut sample, 0, [0xff, 0x01], 4, [0, 0, 0, 0]);
        for plane in 0..CRX_PLANE_COUNT {
            let offset = 12 + plane * 24;
            let plane_tag = 0x08 + u8::try_from(plane).unwrap() * 0x10;
            write_segment(&mut sample, offset, [0xff, 0x02], 1, [plane_tag, 0, 0, 0]);
            write_segment(&mut sample, offset + 12, [0xff, 0x03], 1, [0, 0x20, 0, 0]);
        }
        let mut wrong_duplicate = sample.clone();
        wrong_duplicate[28..32].copy_from_slice(&2_u32.to_be_bytes());
        assert!(matches!(
            parse_crx_sample(&wrong_duplicate),
            Err(CrxError::InvalidField {
                field: "FF02/FF03 plane size"
            })
        ));

        sample.pop();
        assert!(matches!(
            parse_crx_sample(&sample),
            Err(CrxError::InvalidSampleSize { .. })
        ));
    }

    #[test]
    fn rejects_config_dimension_mismatch() {
        let error = CrxConfig::parse(&EOS_R8_MAIN_CMP1, &EOS_R8_PREVIEW_CDI1).unwrap_err();
        assert!(matches!(error, CrxError::DimensionMismatch { .. }));
    }

    fn write_segment(sample: &mut [u8], offset: usize, marker: [u8; 2], size: u32, descriptor: [u8; 4]) {
        sample[offset..offset + 2].copy_from_slice(&marker);
        sample[offset + 2..offset + 4].copy_from_slice(&8_u16.to_be_bytes());
        sample[offset + 4..offset + 8].copy_from_slice(&size.to_be_bytes());
        sample[offset + 8..offset + 12].copy_from_slice(&descriptor);
    }
}
