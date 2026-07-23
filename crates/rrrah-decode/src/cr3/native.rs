//! Unified, bounded parsing of the native EOS R8 CR3 frame.
//!
//! This is intentionally a framing facade, not an entropy decoder. It joins
//! the independently parsed BMFF sample tables, CRX configuration and plane
//! chunks, CMT identity/profile metadata, and CTMD as-shot white balance while
//! retaining only borrowed ranges into the caller-owned file buffer.

use std::fmt;

use thiserror::Error;

use super::{
    bmff::{self, BmffFile, BoxHeader, FourCc, ParseError, SampleLocation},
    crx::{CrxConfig, CrxPlaneChunk},
    ctmd::{self, CtmdError, EosR8AsShotWhiteBalance},
    metadata::{self, EosR8Metadata, MetadataError},
    select::{self, SelectError},
};

const MOOV: FourCc = FourCc(*b"moov");
const META: FourCc = FourCc(*b"meta");
const UUID: FourCc = FourCc(*b"uuid");
const CMT1: FourCc = FourCc(*b"CMT1");
const CMT3: FourCc = FourCc(*b"CMT3");
const CTMD: FourCc = FourCc(*b"CTMD");
const MAX_CTMD_SAMPLE_CANDIDATES: usize = 4_096;
const CANON_METADATA_UUID: [u8; 16] = [
    0x85, 0xc0, 0xb6, 0x87, 0x82, 0x0f, 0x11, 0xe0, 0x81, 0x11, 0xf4, 0xce, 0x46, 0x2b, 0x6a, 0x48,
];

/// Facade-level resource limits in addition to the parsers' internal limits.
#[allow(clippy::struct_field_names)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeParseLimits {
    pub(crate) max_file_bytes: u64,
    pub(crate) max_canon_metadata_bytes: usize,
    pub(crate) max_cmt_payload_bytes: usize,
    pub(crate) max_ctmd_sample_bytes: usize,
}

impl Default for NativeParseLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: 2 * 1024 * 1024 * 1024,
            max_canon_metadata_bytes: 256 * 1024 * 1024,
            max_cmt_payload_bytes: 64 * 1024 * 1024,
            max_ctmd_sample_bytes: 64 * 1024 * 1024,
        }
    }
}

/// A fully framed native CR3 image, before entropy decoding.
///
/// All large byte ranges are borrowed from the input. `Debug` reports only
/// geometry, offsets, lengths, and supported calibration values; it never
/// prints RAW bytes or preserved TIFF values.
pub(crate) struct NativeFrame<'a> {
    pub(crate) file_len: u64,
    pub(crate) raw_track_id: Option<u32>,
    pub(crate) raw_track_index: usize,
    pub(crate) raw_description_index: usize,
    pub(crate) raw_sample_location: SampleLocation,
    pub(crate) raw_declared_payload_size: u32,
    pub(crate) config: CrxConfig,
    pub(crate) planes: [CrxPlaneChunk<'a>; 4],
    pub(crate) metadata: EosR8Metadata<'a>,
    pub(crate) ctmd_track_id: Option<u32>,
    pub(crate) ctmd_sample_location: SampleLocation,
    pub(crate) as_shot_white_balance: EosR8AsShotWhiteBalance,
}

impl fmt::Debug for NativeFrame<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeFrame")
            .field("file_len", &self.file_len)
            .field("raw_track_id", &self.raw_track_id)
            .field("raw_track_index", &self.raw_track_index)
            .field("raw_description_index", &self.raw_description_index)
            .field("raw_sample_location", &self.raw_sample_location)
            .field("raw_declared_payload_size", &self.raw_declared_payload_size)
            .field(
                "coded_geometry",
                &(
                    self.config.compression.image_width,
                    self.config.compression.image_height,
                ),
            )
            .field(
                "plane_byte_lengths",
                &self.planes.each_ref().map(|plane| plane.data.len()),
            )
            .field("camera", &("Canon", "EOS R8"))
            .field("ctmd_track_id", &self.ctmd_track_id)
            .field("ctmd_sample_location", &self.ctmd_sample_location)
            .field("as_shot_white_balance", &self.as_shot_white_balance)
            .finish()
    }
}

#[derive(Debug, Error)]
pub(crate) enum NativeParseError {
    #[error("invalid CR3 BMFF container")]
    Container {
        #[from]
        source: ParseError,
    },
    #[error("could not select a native full-resolution CRX sample")]
    NativeSample {
        #[from]
        source: SelectError,
    },
    #[error("Canon metadata UUID container is missing from moov")]
    MissingCanonMetadataContainer,
    #[error("Canon metadata UUID container occurs more than once in moov")]
    DuplicateCanonMetadataContainer,
    #[error("Canon metadata UUID payload range is outside the parsed input")]
    CanonMetadataContainerOutsideInput,
    #[error("Canon metadata UUID payload has {actual} bytes, above the configured {limit}-byte limit")]
    CanonMetadataContainerLimit { actual: usize, limit: usize },
    #[error("invalid box sequence inside the Canon metadata UUID container")]
    CanonMetadataContainer {
        #[source]
        source: ParseError,
    },
    #[error("required Canon metadata child {box_name} is missing")]
    MissingCmtBox { box_name: &'static str },
    #[error("Canon metadata child {box_name} occurs more than once")]
    DuplicateCmtBox { box_name: &'static str },
    #[error("{box_name} payload has {actual} bytes, above the configured {limit}-byte limit")]
    CmtPayloadLimit {
        box_name: &'static str,
        actual: usize,
        limit: usize,
    },
    #[error("{box_name} payload range is outside the parsed input")]
    CmtPayloadOutsideInput { box_name: &'static str },
    #[error("invalid or unsupported EOS R8 CMT metadata")]
    CmtMetadata {
        #[from]
        source: MetadataError,
    },
    #[error("native CRX configuration is outside the supported one-tile EOS R8 profile")]
    UnsupportedNativeConfiguration,
    #[error("CTMD sample description has no stsz table")]
    MissingCtmdSampleTable,
    #[error("CTMD sample description has an empty stsz table")]
    EmptyCtmdSampleTable,
    #[error("CTMD selection would inspect more than the {limit}-sample safety limit")]
    CtmdCandidateLimitExceeded { limit: usize },
    #[error("CTMD sample table is invalid")]
    InvalidCtmdSampleTable {
        #[source]
        source: ParseError,
    },
    #[error("CTMD sample maps to invalid stsd index {index} with {count} descriptions")]
    InvalidCtmdDescriptionIndex { index: u32, count: usize },
    #[error("no meta/CTMD sample is referenced by the BMFF sample tables")]
    MissingCtmdSample,
    #[error("more than one meta/CTMD sample is referenced by the BMFF sample tables")]
    AmbiguousCtmdSample,
    #[error("CTMD sample range cannot be represented on this platform")]
    CtmdSampleRangeOverflow,
    #[error("CTMD sample range [{start}, {end}) is outside the {file_len}-byte input")]
    CtmdSampleOutsideInput { start: u64, end: u64, file_len: u64 },
    #[error("CTMD sample has {actual} bytes, above the configured {limit}-byte limit")]
    CtmdSampleLimit { actual: usize, limit: usize },
    #[error("invalid or unsupported EOS R8 CTMD metadata")]
    CtmdMetadata {
        #[from]
        source: CtmdError,
    },
}

#[derive(Clone, Copy)]
struct SelectedCtmd<'a> {
    track_id: Option<u32>,
    location: SampleLocation,
    bytes: &'a [u8],
}

/// Parses a native EOS R8 frame without decoding its entropy streams.
pub(crate) fn parse(data: &[u8]) -> Result<NativeFrame<'_>, NativeParseError> {
    parse_with_limits(data, NativeParseLimits::default())
}

pub(crate) fn parse_with_limits(
    data: &[u8],
    limits: NativeParseLimits,
) -> Result<NativeFrame<'_>, NativeParseError> {
    let file = bmff::parse_with_limits(
        data,
        bmff::ParseLimits {
            file_size: limits.max_file_bytes,
            ..bmff::ParseLimits::default()
        },
    )?;
    let selected = select::select_full_resolution_crx(&file, data)?;

    if !is_supported_eos_r8_configuration(&selected.config) {
        return Err(NativeParseError::UnsupportedNativeConfiguration);
    }

    let canon_metadata = canon_metadata_container(&file, data, limits.max_canon_metadata_bytes)?;
    let canon_index = bmff::parse_with_limits(
        canon_metadata,
        bmff::ParseLimits {
            file_size: u64::try_from(limits.max_canon_metadata_bytes).unwrap_or(u64::MAX),
            ..bmff::ParseLimits::default()
        },
    )
    .map_err(|source| NativeParseError::CanonMetadataContainer { source })?;
    let cmt1 = unique_top_level_payload(
        &canon_index,
        canon_metadata,
        CMT1,
        "CMT1",
        limits.max_cmt_payload_bytes,
    )?;
    let cmt3 = unique_top_level_payload(
        &canon_index,
        canon_metadata,
        CMT3,
        "CMT3",
        limits.max_cmt_payload_bytes,
    )?;
    let metadata = metadata::extract_eos_r8(cmt1, cmt3)?;

    if selected.config.compression.n_bits != metadata.profile.bits_per_sample {
        return Err(NativeParseError::UnsupportedNativeConfiguration);
    }

    let ctmd = select_ctmd_sample(&file, data, limits.max_ctmd_sample_bytes)?;
    let as_shot_white_balance = ctmd::extract_eos_r8_as_shot_white_balance(ctmd.bytes)?;

    Ok(NativeFrame {
        file_len: file.file_len,
        raw_track_id: selected.track_id,
        raw_track_index: selected.track_index,
        raw_description_index: selected.description_index,
        raw_sample_location: selected.location,
        raw_declared_payload_size: selected.sample.declared_payload_size,
        config: selected.config,
        planes: selected.sample.planes,
        metadata,
        ctmd_track_id: ctmd.track_id,
        ctmd_sample_location: ctmd.location,
        as_shot_white_balance,
    })
}

fn is_supported_eos_r8_configuration(config: &CrxConfig) -> bool {
    let compression = &config.compression;
    compression.version == 0x0100
        && compression.sample_precision == 15
        && compression.n_bits == 14
        && compression.plane_count == 4
        && compression.tile_width == compression.image_width
        && compression.tile_height == compression.image_height
        && config.image_description.eos_r8_sensor_geometry().is_some()
}

fn canon_metadata_container<'a>(
    file: &BmffFile,
    data: &'a [u8],
    max_container_bytes: usize,
) -> Result<&'a [u8], NativeParseError> {
    let mut matches = file
        .unknown_boxes
        .iter()
        .filter(|unknown| {
            unknown.parent == Some(MOOV)
                && unknown.header.box_type == UUID
                && unknown.header.user_type == Some(CANON_METADATA_UUID)
        })
        .map(|unknown| unknown.header);
    let header = matches
        .next()
        .ok_or(NativeParseError::MissingCanonMetadataContainer)?;
    if matches.next().is_some() {
        return Err(NativeParseError::DuplicateCanonMetadataContainer);
    }

    let range = header.payload_range()?;
    if range.len() > max_container_bytes {
        return Err(NativeParseError::CanonMetadataContainerLimit {
            actual: range.len(),
            limit: max_container_bytes,
        });
    }
    data.get(range)
        .ok_or(NativeParseError::CanonMetadataContainerOutsideInput)
}

fn unique_top_level_payload<'a>(
    file: &BmffFile,
    data: &'a [u8],
    box_type: FourCc,
    box_name: &'static str,
    max_payload_bytes: usize,
) -> Result<&'a [u8], NativeParseError> {
    let mut matches = file
        .top_level_boxes
        .iter()
        .copied()
        .filter(|header| header.box_type == box_type);
    let header = matches
        .next()
        .ok_or(NativeParseError::MissingCmtBox { box_name })?;
    if matches.next().is_some() {
        return Err(NativeParseError::DuplicateCmtBox { box_name });
    }

    checked_cmt_payload(data, header, box_name, max_payload_bytes)
}

fn checked_cmt_payload<'a>(
    data: &'a [u8],
    header: BoxHeader,
    box_name: &'static str,
    max_payload_bytes: usize,
) -> Result<&'a [u8], NativeParseError> {
    let range = header.payload_range()?;
    if range.len() > max_payload_bytes {
        return Err(NativeParseError::CmtPayloadLimit {
            box_name,
            actual: range.len(),
            limit: max_payload_bytes,
        });
    }
    data.get(range)
        .ok_or(NativeParseError::CmtPayloadOutsideInput { box_name })
}

fn select_ctmd_sample<'a>(
    file: &BmffFile,
    data: &'a [u8],
    max_sample_bytes: usize,
) -> Result<SelectedCtmd<'a>, NativeParseError> {
    let mut selected = None;
    let mut candidate_budget = 0usize;

    for track in &file.tracks {
        if track.handler_type != Some(META)
            || !track
                .sample_descriptions
                .iter()
                .any(|description| description.format == CTMD)
        {
            continue;
        }

        let sample_count = track
            .sample_count()
            .ok_or(NativeParseError::MissingCtmdSampleTable)?;
        if sample_count == 0 {
            return Err(NativeParseError::EmptyCtmdSampleTable);
        }
        candidate_budget = candidate_budget.checked_add(sample_count).ok_or(
            NativeParseError::CtmdCandidateLimitExceeded {
                limit: MAX_CTMD_SAMPLE_CANDIDATES,
            },
        )?;
        if candidate_budget > MAX_CTMD_SAMPLE_CANDIDATES {
            return Err(NativeParseError::CtmdCandidateLimitExceeded {
                limit: MAX_CTMD_SAMPLE_CANDIDATES,
            });
        }
        let locations = track
            .sample_locations(file.file_len)
            .map_err(|source| NativeParseError::InvalidCtmdSampleTable { source })?;
        for location in locations {
            let location = location.map_err(|source| NativeParseError::InvalidCtmdSampleTable { source })?;
            let description_index = location
                .description_index
                .checked_sub(1)
                .and_then(|index| usize::try_from(index).ok())
                .ok_or(NativeParseError::InvalidCtmdDescriptionIndex {
                    index: location.description_index,
                    count: track.sample_descriptions.len(),
                })?;
            let description = track.sample_descriptions.get(description_index).ok_or(
                NativeParseError::InvalidCtmdDescriptionIndex {
                    index: location.description_index,
                    count: track.sample_descriptions.len(),
                },
            )?;
            if description.format != CTMD {
                continue;
            }

            let bytes = checked_ctmd_sample(data, location, max_sample_bytes)?;
            if selected.is_some() {
                return Err(NativeParseError::AmbiguousCtmdSample);
            }
            selected = Some(SelectedCtmd {
                track_id: track.id,
                location,
                bytes,
            });
        }
    }

    selected.ok_or(NativeParseError::MissingCtmdSample)
}

fn checked_ctmd_sample(
    data: &[u8],
    location: SampleLocation,
    max_sample_bytes: usize,
) -> Result<&[u8], NativeParseError> {
    let end_u64 = location
        .offset
        .checked_add(u64::from(location.size))
        .ok_or(NativeParseError::CtmdSampleRangeOverflow)?;
    let start = usize::try_from(location.offset).map_err(|_| NativeParseError::CtmdSampleRangeOverflow)?;
    let end = usize::try_from(end_u64).map_err(|_| NativeParseError::CtmdSampleRangeOverflow)?;
    let bytes = data
        .get(start..end)
        .ok_or(NativeParseError::CtmdSampleOutsideInput {
            start: location.offset,
            end: end_u64,
            file_len: u64::try_from(data.len()).unwrap_or(u64::MAX),
        })?;
    if bytes.len() > max_sample_bytes {
        return Err(NativeParseError::CtmdSampleLimit {
            actual: bytes.len(),
            limit: max_sample_bytes,
        });
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::*;

    #[test]
    fn facade_limit_rejects_input_before_box_traversal() {
        let error = parse_with_limits(
            &[0; 16],
            NativeParseLimits {
                max_file_bytes: 8,
                ..NativeParseLimits::default()
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            NativeParseError::Container {
                source: ParseError::FileTooLarge { actual: 16, limit: 8 }
            }
        ));
    }

    #[test]
    fn direct_cmt_lookup_requires_one_bounded_container_child() {
        let mut data = [0u8; 32];
        data[8..12].copy_from_slice(b"TIFF");
        let header = BoxHeader {
            box_type: CMT1,
            offset: 0,
            size: 12,
            header_size: 8,
            user_type: None,
        };
        let mut file = BmffFile {
            file_len: data.len() as u64,
            top_level_boxes: vec![header],
            tracks: Vec::new(),
            unknown_boxes: Vec::new(),
        };

        assert_eq!(
            unique_top_level_payload(&file, &data, CMT1, "CMT1", 4).unwrap(),
            b"TIFF"
        );
        assert!(matches!(
            unique_top_level_payload(&file, &data, CMT3, "CMT3", 4),
            Err(NativeParseError::MissingCmtBox { box_name: "CMT3" })
        ));

        file.top_level_boxes.push(header);
        assert!(matches!(
            unique_top_level_payload(&file, &data, CMT1, "CMT1", 4),
            Err(NativeParseError::DuplicateCmtBox { box_name: "CMT1" })
        ));
    }

    #[test]
    fn cmt_payload_limit_is_checked_before_access() {
        let header = BoxHeader {
            box_type: CMT1,
            offset: 0,
            size: 12,
            header_size: 8,
            user_type: None,
        };
        let error = checked_cmt_payload(&[0; 12], header, "CMT1", 3).unwrap_err();

        assert!(matches!(
            error,
            NativeParseError::CmtPayloadLimit {
                box_name: "CMT1",
                actual: 4,
                limit: 3
            }
        ));
    }

    #[test]
    fn parses_both_local_eos_r8_fixtures_when_available() {
        let fixtures = [
            (
                "/tmp/rrrah-eos-r8-9043.cr3",
                22_382_226usize,
                [4_631_608usize, 4_978_552, 4_977_664, 4_681_312],
                [1_678u16, 1_024, 1_659, 1_024],
            ),
            (
                "/tmp/rrrah-eos-r8-9074.cr3",
                21_368_466usize,
                [4_453_096usize, 4_742_944, 4_740_960, 4_464_936],
                [1_691u16, 1_024, 1_641, 1_024],
            ),
        ];

        for (path, file_len, plane_lengths, white_balance) in fixtures {
            let Some(bytes) = read_optional_fixture(path) else {
                continue;
            };
            let frame = parse(&bytes).expect("local EOS R8 fixture should frame");

            assert_eq!(
                usize::try_from(frame.file_len).expect("fixture length fits usize"),
                file_len
            );
            assert_eq!(frame.raw_track_id, Some(3));
            assert_eq!(
                (
                    frame.config.compression.image_width,
                    frame.config.compression.image_height
                ),
                (6_188, 4_120)
            );
            assert_eq!(
                frame.planes.each_ref().map(|plane| plane.data.len()),
                plane_lengths
            );
            assert_eq!(frame.metadata.recorded_make, "Canon");
            assert_eq!(frame.metadata.recorded_model, "Canon EOS R8");
            assert_eq!(frame.ctmd_track_id, Some(4));
            assert_eq!(
                [
                    frame.as_shot_white_balance.red_numerator,
                    frame.as_shot_white_balance.red_denominator,
                    frame.as_shot_white_balance.blue_numerator,
                    frame.as_shot_white_balance.blue_denominator,
                ],
                white_balance
            );

            let debug = format!("{frame:?}");
            assert!(debug.len() < 1_024);
            assert!(!debug.contains("sample_bytes"));
            assert!(!debug.contains("preserved_entries"));
        }
    }

    fn read_optional_fixture(path: &str) -> Option<Vec<u8>> {
        Path::new(path).is_file().then(|| fs::read(path).ok()).flatten()
    }
}
