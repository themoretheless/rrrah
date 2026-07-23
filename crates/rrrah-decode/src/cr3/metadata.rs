//! Clean-room extraction of the metadata needed by the native EOS R8 path.
//!
//! CR3 stores several standalone classic-TIFF payloads in `CMT*` boxes.  This
//! module consumes the already bounded `CMT1` and `CMT3` payloads; locating the
//! boxes remains the container parser's responsibility.
//!
//! Only standard TIFF identity tags are interpreted.  Canon's proprietary
//! white-balance fields are retained losslessly, but are deliberately not
//! assigned a meaning until their encoding can be established independently.

use std::fmt;

use rrrah_core::{CfaColor, Orientation};
use thiserror::Error;

use super::tiff::{Entry, FieldType, Ifd, Tiff, TiffError};

const TAG_MAKE: u16 = 0x010f;
const TAG_MODEL: u16 = 0x0110;
const TAG_ORIENTATION: u16 = 0x0112;

// These two changing CMT3 fields were observed in both local EOS R8 fixtures.
// Their semantics and any relationship to final WB coefficients are unknown.
const TAG_EMPIRICAL_WB_A: u16 = 0x00aa;
const TAG_EMPIRICAL_WB_B: u16 = 0x4037;
const EMPIRICAL_WB_A_COUNT: usize = 6;
const EMPIRICAL_WB_B_COUNT: usize = 24;

/// Empirical EOS R8 calibration profile.
///
/// The calibration values are independent black-box observations for the two
/// local EOS R8 fixtures, not values decoded from a CMT tag.  Keeping this
/// distinction explicit prevents a constant camera profile from being
/// mistaken for per-capture metadata.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EosR8Profile {
    pub(crate) bits_per_sample: u8,
    pub(crate) cfa: [CfaColor; 4],
    pub(crate) black_level: [f32; 4],
    pub(crate) white_level: f32,
    pub(crate) xyz_to_camera: [[f32; 3]; 4],
}

pub(crate) const EOS_R8_PROFILE: EosR8Profile = EosR8Profile {
    bits_per_sample: 14,
    cfa: [CfaColor::Red, CfaColor::Green, CfaColor::Green, CfaColor::Blue],
    black_level: [512.0; 4],
    white_level: 12_735.0,
    xyz_to_camera: [
        [0.9539, -0.2795, -0.1224],
        [-0.4175, 1.1998, 0.2458],
        [-0.0465, 0.1755, 0.6048],
        [0.0, 0.0, 0.0],
    ],
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CmtKind {
    Cmt1,
    Cmt3,
}

impl fmt::Display for CmtKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Cmt1 => "CMT1",
            Self::Cmt3 => "CMT3",
        })
    }
}

/// One CMT entry retained without assigning proprietary semantics to it.
///
/// `Debug` intentionally omits the value bytes.  Some CR3 metadata can contain
/// personal or device-identifying strings, so routine diagnostics must not
/// print the preserved payload.
#[derive(Clone, Copy)]
pub(crate) struct PreservedEntry<'a> {
    cmt: CmtKind,
    tag: u16,
    field_type: FieldType,
    count: u32,
    raw_bytes: &'a [u8],
}

impl PreservedEntry<'_> {
    pub(crate) const fn cmt(&self) -> CmtKind {
        self.cmt
    }

    pub(crate) const fn tag(&self) -> u16 {
        self.tag
    }

    pub(crate) const fn field_type(&self) -> FieldType {
        self.field_type
    }

    pub(crate) const fn count(&self) -> u32 {
        self.count
    }

    pub(crate) const fn raw_bytes(&self) -> &[u8] {
        self.raw_bytes
    }
}

impl fmt::Debug for PreservedEntry<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreservedEntry")
            .field("cmt", &self.cmt)
            .field("tag", &format_args!("{:#06x}", self.tag))
            .field("field_type", &self.field_type)
            .field("count", &self.count)
            .field("byte_len", &self.raw_bytes.len())
            .finish()
    }
}

/// Raw evidence retained for a future clean-room white-balance mapping.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct WhiteBalanceEvidence {
    pub(crate) empirical_tag_00aa: Option<[u16; EMPIRICAL_WB_A_COUNT]>,
    pub(crate) empirical_tag_4037: Option<[u8; EMPIRICAL_WB_B_COUNT]>,
}

impl WhiteBalanceEvidence {
    const fn is_empty(self) -> bool {
        self.empirical_tag_00aa.is_none() && self.empirical_tag_4037.is_none()
    }
}

/// Metadata that is established independently for an EOS R8 CR3.
#[derive(Clone, Debug)]
pub(crate) struct EosR8Metadata<'a> {
    /// TIFF Make as recorded in CMT1.
    pub(crate) recorded_make: &'a str,
    /// TIFF Model as recorded in CMT1 (`Canon EOS R8` in both fixtures).
    pub(crate) recorded_model: &'a str,
    /// Standard TIFF Orientation as recorded in CMT1.
    pub(crate) orientation: Orientation,
    pub(crate) profile: EosR8Profile,
    pub(crate) white_balance_evidence: WhiteBalanceEvidence,
    pub(crate) preserved_entries: Vec<PreservedEntry<'a>>,
}

impl EosR8Metadata<'_> {
    pub(crate) const fn canonical_make() -> &'static str {
        "Canon"
    }

    pub(crate) const fn canonical_model() -> &'static str {
        "EOS R8"
    }

    /// Returns an explicit error instead of silently inventing neutral gains.
    pub(crate) fn white_balance(&self) -> Result<[f32; 4], MetadataError> {
        if self.white_balance_evidence.is_empty() {
            Err(MetadataError::MissingWhiteBalanceEvidence)
        } else {
            Err(MetadataError::UnsupportedWhiteBalanceEncoding)
        }
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub(crate) enum MetadataError {
    #[error("invalid {cmt} classic-TIFF payload: {source}")]
    InvalidTiff {
        cmt: CmtKind,
        #[source]
        source: TiffError,
    },
    #[error("{cmt} has no root IFD")]
    MissingRootIfd { cmt: CmtKind },
    #[error("{cmt} is missing required TIFF tag {tag:#06x} ({name})")]
    MissingTag {
        cmt: CmtKind,
        tag: u16,
        name: &'static str,
    },
    #[error("{cmt} contains {occurrences} copies of interpreted TIFF tag {tag:#06x} ({name})")]
    AmbiguousTag {
        cmt: CmtKind,
        tag: u16,
        name: &'static str,
        occurrences: usize,
    },
    #[error("{cmt} tag {tag:#06x} ({name}) is invalid: {reason}")]
    InvalidTag {
        cmt: CmtKind,
        tag: u16,
        name: &'static str,
        reason: &'static str,
    },
    #[error("CMT1 identifies a camera other than the supported Canon EOS R8 profile")]
    UnsupportedCamera,
    #[error("EOS R8 CMT3 contains no retained white-balance evidence")]
    MissingWhiteBalanceEvidence,
    #[error("EOS R8 proprietary white-balance encoding is not established")]
    UnsupportedWhiteBalanceEncoding,
}

/// Extracts safe identity fields and retains every root CMT entry.
///
/// The caller must pass the TIFF payload only, excluding each BMFF box header.
pub(crate) fn extract_eos_r8<'a>(
    cmt1_payload: &'a [u8],
    cmt3_payload: &'a [u8],
) -> Result<EosR8Metadata<'a>, MetadataError> {
    let cmt1 = parse_root(cmt1_payload, CmtKind::Cmt1)?;
    let make = required_ascii(&cmt1, CmtKind::Cmt1, TAG_MAKE, "Make")?;
    let recorded_model = required_ascii(&cmt1, CmtKind::Cmt1, TAG_MODEL, "Model")?;
    let orientation = required_orientation(&cmt1)?;
    if make != "Canon" || recorded_model != "Canon EOS R8" {
        return Err(MetadataError::UnsupportedCamera);
    }

    let cmt3 = parse_root(cmt3_payload, CmtKind::Cmt3)?;
    let white_balance_evidence = WhiteBalanceEvidence {
        empirical_tag_00aa: optional_short_array::<EMPIRICAL_WB_A_COUNT>(
            &cmt3,
            CmtKind::Cmt3,
            TAG_EMPIRICAL_WB_A,
            "empirical WB field A",
        )?,
        empirical_tag_4037: optional_byte_array::<EMPIRICAL_WB_B_COUNT>(
            &cmt3,
            CmtKind::Cmt3,
            TAG_EMPIRICAL_WB_B,
            "empirical WB field B",
        )?,
    };

    let mut preserved_entries = Vec::new();
    let preserved_entry_count =
        cmt1.entries()
            .len()
            .checked_add(cmt3.entries().len())
            .ok_or(MetadataError::InvalidTag {
                cmt: CmtKind::Cmt3,
                tag: 0,
                name: "preserved entries",
                reason: "entry count overflows",
            })?;
    preserved_entries
        .try_reserve_exact(preserved_entry_count)
        .map_err(|_| MetadataError::InvalidTag {
            cmt: CmtKind::Cmt3,
            tag: 0,
            name: "preserved entries",
            reason: "allocation failed",
        })?;
    preserve_entries(&mut preserved_entries, CmtKind::Cmt1, &cmt1);
    preserve_entries(&mut preserved_entries, CmtKind::Cmt3, &cmt3);

    Ok(EosR8Metadata {
        recorded_make: make,
        recorded_model,
        orientation,
        profile: EOS_R8_PROFILE,
        white_balance_evidence,
        preserved_entries,
    })
}

fn parse_root(payload: &[u8], cmt: CmtKind) -> Result<Ifd<'_>, MetadataError> {
    let tiff = Tiff::parse(payload).map_err(|source| MetadataError::InvalidTiff { cmt, source })?;
    tiff.first_ifd()
        .map_err(|source| MetadataError::InvalidTiff { cmt, source })?
        .ok_or(MetadataError::MissingRootIfd { cmt })
}

fn required_ascii<'a>(
    ifd: &Ifd<'a>,
    cmt: CmtKind,
    tag: u16,
    name: &'static str,
) -> Result<&'a str, MetadataError> {
    let entry = required_entry(ifd, cmt, tag, name)?;
    entry
        .ascii()
        .map_err(|source| MetadataError::InvalidTiff { cmt, source })
}

fn required_orientation(ifd: &Ifd<'_>) -> Result<Orientation, MetadataError> {
    const CMT: CmtKind = CmtKind::Cmt1;
    const NAME: &str = "Orientation";

    let entry = required_entry(ifd, CMT, TAG_ORIENTATION, NAME)?;
    if entry.field_type() != FieldType::Short {
        return Err(MetadataError::InvalidTag {
            cmt: CMT,
            tag: TAG_ORIENTATION,
            name: NAME,
            reason: "expected one TIFF SHORT",
        });
    }
    if entry.count() != 1 {
        return Err(MetadataError::InvalidTag {
            cmt: CMT,
            tag: TAG_ORIENTATION,
            name: NAME,
            reason: "expected exactly one value",
        });
    }

    let value = entry
        .short()
        .map_err(|source| MetadataError::InvalidTiff { cmt: CMT, source })?;
    match value {
        1 => Ok(Orientation::Normal),
        2 => Ok(Orientation::HorizontalFlip),
        3 => Ok(Orientation::Rotate180),
        4 => Ok(Orientation::VerticalFlip),
        5 => Ok(Orientation::Transpose),
        6 => Ok(Orientation::Rotate90),
        7 => Ok(Orientation::Transverse),
        8 => Ok(Orientation::Rotate270),
        _ => Err(MetadataError::InvalidTag {
            cmt: CMT,
            tag: TAG_ORIENTATION,
            name: NAME,
            reason: "expected TIFF orientation value 1 through 8",
        }),
    }
}

fn required_entry<'ifd, 'data>(
    ifd: &'ifd Ifd<'data>,
    cmt: CmtKind,
    tag: u16,
    name: &'static str,
) -> Result<&'ifd Entry<'data>, MetadataError> {
    unique_entry(ifd, cmt, tag, name)?.ok_or(MetadataError::MissingTag { cmt, tag, name })
}

fn unique_entry<'ifd, 'data>(
    ifd: &'ifd Ifd<'data>,
    cmt: CmtKind,
    tag: u16,
    name: &'static str,
) -> Result<Option<&'ifd Entry<'data>>, MetadataError> {
    let mut entries = ifd.entries_with_tag(tag);
    let first = entries.next();
    if entries.next().is_some() {
        return Err(MetadataError::AmbiguousTag {
            cmt,
            tag,
            name,
            occurrences: 2usize.saturating_add(entries.count()),
        });
    }
    Ok(first)
}

fn optional_short_array<const N: usize>(
    ifd: &Ifd<'_>,
    cmt: CmtKind,
    tag: u16,
    name: &'static str,
) -> Result<Option<[u16; N]>, MetadataError> {
    let Some(entry) = unique_entry(ifd, cmt, tag, name)? else {
        return Ok(None);
    };
    if usize::try_from(entry.count()).ok() != Some(N) {
        return Err(MetadataError::InvalidTag {
            cmt,
            tag,
            name,
            reason: if usize::try_from(entry.count()).is_ok_and(|count| count < N) {
                "value count is too small"
            } else {
                "value count is too large"
            },
        });
    }
    let values = entry
        .short_values()
        .map_err(|source| MetadataError::InvalidTiff { cmt, source })?;
    values
        .try_into()
        .map(Some)
        .map_err(|values: Vec<u16>| MetadataError::InvalidTag {
            cmt,
            tag,
            name,
            reason: if values.len() < N {
                "value count is too small"
            } else {
                "value count is too large"
            },
        })
}

fn optional_byte_array<const N: usize>(
    ifd: &Ifd<'_>,
    cmt: CmtKind,
    tag: u16,
    name: &'static str,
) -> Result<Option<[u8; N]>, MetadataError> {
    let Some(entry) = unique_entry(ifd, cmt, tag, name)? else {
        return Ok(None);
    };
    if entry.field_type() != FieldType::Undefined {
        return Err(MetadataError::InvalidTag {
            cmt,
            tag,
            name,
            reason: "expected TIFF UNDEFINED",
        });
    }
    let bytes = entry
        .undefined()
        .map_err(|source| MetadataError::InvalidTiff { cmt, source })?;
    bytes.try_into().map(Some).map_err(|_| MetadataError::InvalidTag {
        cmt,
        tag,
        name,
        reason: "unexpected byte count",
    })
}

fn preserve_entries<'a>(output: &mut Vec<PreservedEntry<'a>>, cmt: CmtKind, ifd: &Ifd<'a>) {
    output.extend(ifd.entries().iter().map(|entry| PreservedEntry {
        cmt,
        tag: entry.tag(),
        field_type: entry.field_type(),
        count: entry.count(),
        raw_bytes: entry.raw_bytes(),
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    const TIFF_HEADER_LEN: usize = 8;
    const IFD_ENTRY_LEN: usize = 12;

    struct FixtureEntry<'a> {
        tag: u16,
        field_type: u16,
        count: u32,
        value: &'a [u8],
    }

    fn little_endian_tiff(entries: &[FixtureEntry<'_>]) -> Vec<u8> {
        let directory_len = 2 + entries.len() * IFD_ENTRY_LEN + 4;
        let mut next_value_offset = TIFF_HEADER_LEN + directory_len;
        let mut payloads = Vec::new();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"II");
        bytes.extend_from_slice(&42_u16.to_le_bytes());
        bytes.extend_from_slice(
            &u32::try_from(TIFF_HEADER_LEN)
                .expect("TIFF header length")
                .to_le_bytes(),
        );
        bytes.extend_from_slice(
            &u16::try_from(entries.len())
                .expect("fixture entry count")
                .to_le_bytes(),
        );

        for entry in entries {
            bytes.extend_from_slice(&entry.tag.to_le_bytes());
            bytes.extend_from_slice(&entry.field_type.to_le_bytes());
            bytes.extend_from_slice(&entry.count.to_le_bytes());
            if entry.value.len() <= 4 {
                bytes.extend_from_slice(entry.value);
                bytes.resize(bytes.len() + 4 - entry.value.len(), 0);
            } else {
                bytes.extend_from_slice(
                    &u32::try_from(next_value_offset)
                        .expect("fixture value offset")
                        .to_le_bytes(),
                );
                payloads.extend_from_slice(entry.value);
                next_value_offset += entry.value.len();
            }
        }
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&payloads);
        bytes
    }

    fn shorts(values: &[u16]) -> Vec<u8> {
        values.iter().flat_map(|value| value.to_le_bytes()).collect()
    }

    fn identity_fixture(make: &[u8], model: &[u8]) -> Vec<u8> {
        let orientation = 1_u16.to_le_bytes();
        little_endian_tiff(&[
            FixtureEntry {
                tag: TAG_MAKE,
                field_type: FieldType::Ascii as u16,
                count: u32::try_from(make.len()).expect("make length"),
                value: make,
            },
            FixtureEntry {
                tag: TAG_MODEL,
                field_type: FieldType::Ascii as u16,
                count: u32::try_from(model.len()).expect("model length"),
                value: model,
            },
            FixtureEntry {
                tag: TAG_ORIENTATION,
                field_type: FieldType::Short as u16,
                count: 1,
                value: &orientation,
            },
        ])
    }

    #[test]
    fn extracts_identity_profile_and_preserves_unknown_fields() {
        let cmt1 = identity_fixture(b"Canon\0", b"Canon EOS R8\0");
        let field_a = shorts(&[12, 845, 1024, 1024, 546, 0]);
        let field_b = [0x5a; EMPIRICAL_WB_B_COUNT];
        let private = [1, 2, 3, 4, 5];
        let cmt3 = little_endian_tiff(&[
            FixtureEntry {
                tag: TAG_EMPIRICAL_WB_A,
                field_type: FieldType::Short as u16,
                count: u32::try_from(EMPIRICAL_WB_A_COUNT).expect("field A count"),
                value: &field_a,
            },
            FixtureEntry {
                tag: TAG_EMPIRICAL_WB_B,
                field_type: FieldType::Undefined as u16,
                count: u32::try_from(EMPIRICAL_WB_B_COUNT).expect("field B count"),
                value: &field_b,
            },
            FixtureEntry {
                tag: 0x7777,
                field_type: FieldType::Undefined as u16,
                count: u32::try_from(private.len()).expect("private length"),
                value: &private,
            },
        ]);

        let metadata = extract_eos_r8(&cmt1, &cmt3).expect("EOS R8 metadata");
        assert_eq!(metadata.recorded_make, "Canon");
        assert_eq!(metadata.recorded_model, "Canon EOS R8");
        assert_eq!(metadata.orientation, Orientation::Normal);
        assert_eq!(EosR8Metadata::canonical_model(), "EOS R8");
        assert_eq!(metadata.profile, EOS_R8_PROFILE);
        assert_eq!(
            metadata.white_balance_evidence.empirical_tag_00aa,
            Some([12, 845, 1024, 1024, 546, 0])
        );
        assert_eq!(
            metadata.white_balance(),
            Err(MetadataError::UnsupportedWhiteBalanceEncoding)
        );

        let preserved = metadata
            .preserved_entries
            .iter()
            .find(|entry| entry.cmt() == CmtKind::Cmt3 && entry.tag() == 0x7777)
            .expect("preserved private field");
        assert_eq!(preserved.field_type(), FieldType::Undefined);
        assert_eq!(preserved.count(), 5);
        assert_eq!(preserved.raw_bytes(), private);
        assert!(!format!("{preserved:?}").contains("01, 02, 03"));
    }

    #[test]
    fn rejects_another_camera_before_applying_the_profile() {
        let cmt1 = identity_fixture(b"Canon\0", b"Canon Other\0");
        let cmt3 = little_endian_tiff(&[]);
        assert!(matches!(
            extract_eos_r8(&cmt1, &cmt3),
            Err(MetadataError::UnsupportedCamera)
        ));
    }

    #[test]
    fn reports_missing_and_unmapped_white_balance_distinctly() {
        let cmt1 = identity_fixture(b"Canon\0", b"Canon EOS R8\0");
        let empty_cmt3 = little_endian_tiff(&[]);
        let metadata = extract_eos_r8(&cmt1, &empty_cmt3).expect("identity-only metadata");
        assert_eq!(
            metadata.white_balance(),
            Err(MetadataError::MissingWhiteBalanceEvidence)
        );

        let wrong_count = shorts(&[1, 2, 3]);
        let malformed_cmt3 = little_endian_tiff(&[FixtureEntry {
            tag: TAG_EMPIRICAL_WB_A,
            field_type: FieldType::Short as u16,
            count: 3,
            value: &wrong_count,
        }]);
        assert!(matches!(
            extract_eos_r8(&cmt1, &malformed_cmt3),
            Err(MetadataError::InvalidTag {
                cmt: CmtKind::Cmt3,
                tag: TAG_EMPIRICAL_WB_A,
                ..
            })
        ));
    }

    #[test]
    fn malformed_tiff_and_missing_identity_have_explicit_errors() {
        let cmt3 = little_endian_tiff(&[]);
        assert!(matches!(
            extract_eos_r8(b"not a TIFF", &cmt3),
            Err(MetadataError::InvalidTiff {
                cmt: CmtKind::Cmt1,
                ..
            })
        ));

        let no_model = little_endian_tiff(&[FixtureEntry {
            tag: TAG_MAKE,
            field_type: FieldType::Ascii as u16,
            count: 6,
            value: b"Canon\0",
        }]);
        assert!(matches!(
            extract_eos_r8(&no_model, &cmt3),
            Err(MetadataError::MissingTag {
                cmt: CmtKind::Cmt1,
                tag: TAG_MODEL,
                ..
            })
        ));
    }

    #[test]
    fn maps_all_standard_tiff_orientations() {
        let expected = [
            Orientation::Normal,
            Orientation::HorizontalFlip,
            Orientation::Rotate180,
            Orientation::VerticalFlip,
            Orientation::Transpose,
            Orientation::Rotate90,
            Orientation::Transverse,
            Orientation::Rotate270,
        ];
        let cmt3 = little_endian_tiff(&[]);

        for (value, expected_orientation) in (1_u16..=8).zip(expected) {
            let orientation = value.to_le_bytes();
            let cmt1 = little_endian_tiff(&[
                FixtureEntry {
                    tag: TAG_MAKE,
                    field_type: FieldType::Ascii as u16,
                    count: 6,
                    value: b"Canon\0",
                },
                FixtureEntry {
                    tag: TAG_MODEL,
                    field_type: FieldType::Ascii as u16,
                    count: 13,
                    value: b"Canon EOS R8\0",
                },
                FixtureEntry {
                    tag: TAG_ORIENTATION,
                    field_type: FieldType::Short as u16,
                    count: 1,
                    value: &orientation,
                },
            ]);
            let metadata = extract_eos_r8(&cmt1, &cmt3).expect("standard TIFF orientation");
            assert_eq!(metadata.orientation, expected_orientation, "orientation {value}");
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn rejects_missing_invalid_and_duplicate_orientation() {
        let cmt3 = little_endian_tiff(&[]);
        let missing = little_endian_tiff(&[
            FixtureEntry {
                tag: TAG_MAKE,
                field_type: FieldType::Ascii as u16,
                count: 6,
                value: b"Canon\0",
            },
            FixtureEntry {
                tag: TAG_MODEL,
                field_type: FieldType::Ascii as u16,
                count: 13,
                value: b"Canon EOS R8\0",
            },
        ]);
        assert!(matches!(
            extract_eos_r8(&missing, &cmt3),
            Err(MetadataError::MissingTag {
                cmt: CmtKind::Cmt1,
                tag: TAG_ORIENTATION,
                ..
            })
        ));

        let invalid_type = 1_u32.to_le_bytes();
        let invalid_type_cmt1 = little_endian_tiff(&[
            FixtureEntry {
                tag: TAG_MAKE,
                field_type: FieldType::Ascii as u16,
                count: 6,
                value: b"Canon\0",
            },
            FixtureEntry {
                tag: TAG_MODEL,
                field_type: FieldType::Ascii as u16,
                count: 13,
                value: b"Canon EOS R8\0",
            },
            FixtureEntry {
                tag: TAG_ORIENTATION,
                field_type: FieldType::Long as u16,
                count: 1,
                value: &invalid_type,
            },
        ]);
        assert!(matches!(
            extract_eos_r8(&invalid_type_cmt1, &cmt3),
            Err(MetadataError::InvalidTag {
                cmt: CmtKind::Cmt1,
                tag: TAG_ORIENTATION,
                ..
            })
        ));

        let invalid_value = 9_u16.to_le_bytes();
        let invalid_value_cmt1 = little_endian_tiff(&[
            FixtureEntry {
                tag: TAG_MAKE,
                field_type: FieldType::Ascii as u16,
                count: 6,
                value: b"Canon\0",
            },
            FixtureEntry {
                tag: TAG_MODEL,
                field_type: FieldType::Ascii as u16,
                count: 13,
                value: b"Canon EOS R8\0",
            },
            FixtureEntry {
                tag: TAG_ORIENTATION,
                field_type: FieldType::Short as u16,
                count: 1,
                value: &invalid_value,
            },
        ]);
        assert!(matches!(
            extract_eos_r8(&invalid_value_cmt1, &cmt3),
            Err(MetadataError::InvalidTag {
                cmt: CmtKind::Cmt1,
                tag: TAG_ORIENTATION,
                ..
            })
        ));

        let normal = 1_u16.to_le_bytes();
        let duplicate = little_endian_tiff(&[
            FixtureEntry {
                tag: TAG_MAKE,
                field_type: FieldType::Ascii as u16,
                count: 6,
                value: b"Canon\0",
            },
            FixtureEntry {
                tag: TAG_MODEL,
                field_type: FieldType::Ascii as u16,
                count: 13,
                value: b"Canon EOS R8\0",
            },
            FixtureEntry {
                tag: TAG_ORIENTATION,
                field_type: FieldType::Short as u16,
                count: 1,
                value: &normal,
            },
            FixtureEntry {
                tag: TAG_ORIENTATION,
                field_type: FieldType::Short as u16,
                count: 1,
                value: &normal,
            },
        ]);
        assert!(matches!(
            extract_eos_r8(&duplicate, &cmt3),
            Err(MetadataError::AmbiguousTag {
                cmt: CmtKind::Cmt1,
                tag: TAG_ORIENTATION,
                occurrences: 2,
                ..
            })
        ));
    }

    #[test]
    fn rejects_duplicate_interpreted_tags_instead_of_using_file_order() {
        let cmt1 = little_endian_tiff(&[
            FixtureEntry {
                tag: TAG_MAKE,
                field_type: FieldType::Ascii as u16,
                count: 6,
                value: b"Canon\0",
            },
            FixtureEntry {
                tag: TAG_MAKE,
                field_type: FieldType::Ascii as u16,
                count: 5,
                value: b"Evil\0",
            },
            FixtureEntry {
                tag: TAG_MODEL,
                field_type: FieldType::Ascii as u16,
                count: 13,
                value: b"Canon EOS R8\0",
            },
        ]);
        let cmt3 = little_endian_tiff(&[]);

        assert!(matches!(
            extract_eos_r8(&cmt1, &cmt3),
            Err(MetadataError::AmbiguousTag {
                cmt: CmtKind::Cmt1,
                tag: TAG_MAKE,
                occurrences: 2,
                ..
            })
        ));

        let cmt1 = identity_fixture(b"Canon\0", b"Canon EOS R8\0");
        let values = shorts(&[1; EMPIRICAL_WB_A_COUNT]);
        let empirical_count = u32::try_from(EMPIRICAL_WB_A_COUNT).expect("empirical fixture count fits u32");
        let duplicate_evidence = little_endian_tiff(&[
            FixtureEntry {
                tag: TAG_EMPIRICAL_WB_A,
                field_type: FieldType::Short as u16,
                count: empirical_count,
                value: &values,
            },
            FixtureEntry {
                tag: TAG_EMPIRICAL_WB_A,
                field_type: FieldType::Short as u16,
                count: empirical_count,
                value: &values,
            },
        ]);
        assert!(matches!(
            extract_eos_r8(&cmt1, &duplicate_evidence),
            Err(MetadataError::AmbiguousTag {
                cmt: CmtKind::Cmt3,
                tag: TAG_EMPIRICAL_WB_A,
                occurrences: 2,
                ..
            })
        ));
    }
}
