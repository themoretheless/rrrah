//! Selection of the full-resolution native CRX sample from a parsed CR3 file.
//!
//! A `CRAW` sample entry is not sufficient evidence by itself: Canon also uses
//! it for a JPEG preview. Native candidates must carry both the `CMP1`
//! compression description and the `CDI1` image description. `CDI1` in turn
//! contains the `IAD1` box parsed by [`CrxConfig`].

use std::ops::Range;

use thiserror::Error;

use super::{
    bmff::{BmffFile, BoxHeader, FourCc, ParseError, SampleDescription, SampleLocation},
    crx::{CrxConfig, CrxError, CrxSample, parse_crx_sample},
};

const VIDE: FourCc = FourCc(*b"vide");
const CRAW: FourCc = FourCc(*b"CRAW");
const CMP1: FourCc = FourCc(*b"CMP1");
const CDI1: FourCc = FourCc(*b"CDI1");
const MAX_NATIVE_SAMPLE_CANDIDATES: usize = 65_536;

/// One native CRX sample together with the configuration that describes it.
#[derive(Debug)]
pub(crate) struct SelectedCrxSample<'a> {
    pub(crate) track_id: Option<u32>,
    pub(crate) track_index: usize,
    pub(crate) sample_index: usize,
    /// Zero-based index into the track's `stsd` entries.
    pub(crate) description_index: usize,
    pub(crate) location: SampleLocation,
    pub(crate) config: CrxConfig,
    /// The exact byte slice described by the sample's `stsz` entry.
    pub(crate) sample_bytes: &'a [u8],
    pub(crate) sample: CrxSample<'a>,
}

#[derive(Debug, Error)]
pub(crate) enum SelectError {
    #[error("BMFF index covers {indexed} bytes, but selection received {actual} bytes")]
    InputLengthMismatch { indexed: u64, actual: u64 },
    #[error("no video CRAW description with native CMP1/CDI1 evidence was found")]
    NoNativeCrxDescription,
    #[error("native CRX selection would inspect more than the {limit}-sample safety limit")]
    CandidateLimitExceeded { limit: usize },
    #[error("more than one native CRX sample ties for the best full-resolution score")]
    AmbiguousBestNativeCrxSample,
    #[error(
        "none of the {rejected_candidates} native CRX candidates was valid; last rejection: {last_rejection}"
    )]
    NoValidNativeCrxSample {
        rejected_candidates: usize,
        #[source]
        last_rejection: Box<CandidateRejection>,
    },
}

#[derive(Debug, Error)]
pub(crate) enum CandidateRejection {
    #[error("native CRAW description has no media samples")]
    NoSamples,
    #[error("native CRAW sample could not be resolved through stsz/stsc/stco")]
    SampleTable {
        #[source]
        source: ParseError,
    },
    #[error("sample description index {index} is invalid for {count} stsd entries")]
    InvalidDescriptionIndex { index: u32, count: usize },
    #[error("native CRAW description is missing its {box_type} extension")]
    MissingExtension { box_type: FourCc },
    #[error("native CRAW description contains more than one {box_type} extension")]
    DuplicateExtension { box_type: FourCc },
    #[error("{box_type} box range cannot be represented on this platform")]
    BoxRangeOverflow { box_type: FourCc },
    #[error("{box_type} box range [{start}, {end}) is outside the {file_len}-byte input")]
    BoxOutsideInput {
        box_type: FourCc,
        start: u64,
        end: u64,
        file_len: u64,
    },
    #[error("sample range cannot be represented on this platform")]
    SampleRangeOverflow,
    #[error("sample range [{start}, {end}) is outside the {file_len}-byte input")]
    SampleOutsideInput { start: u64, end: u64, file_len: u64 },
    #[error("CMP1/CDI1/IAD1 configuration is invalid")]
    InvalidConfiguration {
        #[source]
        source: CrxError,
    },
    #[error("CRX sample framing does not exactly match its stsz entry")]
    InvalidSample {
        #[source]
        source: CrxError,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CandidateScore {
    coded_pixels: u64,
    crop_pixels: u64,
    sample_bytes: u32,
}

/// Selects the largest valid native CRX sample.
///
/// Candidates are ranked by coded dimensions, then by the first IAD1 crop
/// rectangle, then by exact `stsz` size. A tie for the best score is rejected
/// as ambiguous. A candidate is admitted only after its sample-description
/// extensions, BMFF sample mapping, input bounds, and CRX internal plane
/// lengths have all been validated.
#[allow(clippy::too_many_lines)]
pub(crate) fn select_full_resolution_crx<'a>(
    file: &BmffFile,
    data: &'a [u8],
) -> Result<SelectedCrxSample<'a>, SelectError> {
    validate_input_length(file, data)?;

    let mut saw_native_description = false;
    let mut rejected_candidates = 0usize;
    let mut last_rejection = None;
    let mut best: Option<(CandidateScore, SelectedCrxSample<'a>)> = None;
    let mut best_is_ambiguous = false;
    let mut candidate_budget = 0usize;

    for (track_index, track) in file.tracks.iter().enumerate() {
        if track.handler_type != Some(VIDE) {
            continue;
        }
        let track_has_native_description = track.sample_descriptions.iter().any(is_native_crx_hint);
        if !track_has_native_description {
            continue;
        }
        saw_native_description = true;

        let Some(sample_count) = track.sample_count() else {
            rejected_candidates = rejected_candidates.saturating_add(1);
            last_rejection = Some(CandidateRejection::SampleTable {
                source: ParseError::MissingTable {
                    track_id: track.id,
                    table: FourCc(*b"stsz"),
                },
            });
            continue;
        };
        if sample_count == 0 {
            rejected_candidates = rejected_candidates.saturating_add(1);
            last_rejection = Some(CandidateRejection::NoSamples);
            continue;
        }
        candidate_budget =
            candidate_budget
                .checked_add(sample_count)
                .ok_or(SelectError::CandidateLimitExceeded {
                    limit: MAX_NATIVE_SAMPLE_CANDIDATES,
                })?;
        if candidate_budget > MAX_NATIVE_SAMPLE_CANDIDATES {
            return Err(SelectError::CandidateLimitExceeded {
                limit: MAX_NATIVE_SAMPLE_CANDIDATES,
            });
        }

        let rejected_before_track_samples = rejected_candidates;
        let mut mapped_native_sample = false;
        let locations = match track.sample_locations(file.file_len) {
            Ok(locations) => locations,
            Err(source) => {
                rejected_candidates = rejected_candidates.saturating_add(1);
                last_rejection = Some(CandidateRejection::SampleTable { source });
                continue;
            }
        };
        for location in locations {
            let location = match location {
                Ok(location) => location,
                Err(source) => {
                    rejected_candidates = rejected_candidates.saturating_add(1);
                    last_rejection = Some(CandidateRejection::SampleTable { source });
                    break;
                }
            };
            let sample_index = location.sample_index;
            let Some(description_index) = location
                .description_index
                .checked_sub(1)
                .and_then(|index| usize::try_from(index).ok())
            else {
                rejected_candidates = rejected_candidates.saturating_add(1);
                last_rejection = Some(CandidateRejection::InvalidDescriptionIndex {
                    index: location.description_index,
                    count: track.sample_descriptions.len(),
                });
                continue;
            };
            let Some(description) = track.sample_descriptions.get(description_index) else {
                rejected_candidates = rejected_candidates.saturating_add(1);
                last_rejection = Some(CandidateRejection::InvalidDescriptionIndex {
                    index: location.description_index,
                    count: track.sample_descriptions.len(),
                });
                continue;
            };

            // A JPEG preview can share the CRAW entry type. It is not a failed
            // native candidate unless CMP1 or CDI1 evidence is present.
            if !is_native_crx_hint(description) {
                continue;
            }
            mapped_native_sample = true;

            rejected_candidates = rejected_candidates.saturating_add(1);
            let candidate = match parse_candidate(
                data,
                track.id,
                track_index,
                sample_index,
                description_index,
                description,
                location,
            ) {
                Ok(candidate) => candidate,
                Err(rejection) => {
                    last_rejection = Some(rejection);
                    continue;
                }
            };
            rejected_candidates = rejected_candidates.saturating_sub(1);

            let score = score(&candidate);
            match best.as_ref().map(|(best_score, _)| score.cmp(best_score)) {
                None | Some(std::cmp::Ordering::Greater) => {
                    best = Some((score, candidate));
                    best_is_ambiguous = false;
                }
                Some(std::cmp::Ordering::Equal) => {
                    best_is_ambiguous = true;
                }
                Some(std::cmp::Ordering::Less) => {}
            }
        }
        if !mapped_native_sample && rejected_candidates == rejected_before_track_samples {
            rejected_candidates = rejected_candidates.saturating_add(1);
            last_rejection = Some(CandidateRejection::NoSamples);
        }
    }

    if best_is_ambiguous {
        return Err(SelectError::AmbiguousBestNativeCrxSample);
    }
    if let Some((_, selected)) = best {
        return Ok(selected);
    }
    if !saw_native_description {
        return Err(SelectError::NoNativeCrxDescription);
    }

    Err(SelectError::NoValidNativeCrxSample {
        rejected_candidates,
        last_rejection: Box::new(last_rejection.unwrap_or(CandidateRejection::NoSamples)),
    })
}

fn validate_input_length(file: &BmffFile, data: &[u8]) -> Result<(), SelectError> {
    let actual = u64::try_from(data.len()).unwrap_or(u64::MAX);
    if actual == file.file_len {
        Ok(())
    } else {
        Err(SelectError::InputLengthMismatch {
            indexed: file.file_len,
            actual,
        })
    }
}

fn parse_candidate<'a>(
    data: &'a [u8],
    track_id: Option<u32>,
    track_index: usize,
    sample_index: usize,
    description_index: usize,
    description: &SampleDescription,
    location: SampleLocation,
) -> Result<SelectedCrxSample<'a>, CandidateRejection> {
    let cmp1 = unique_extension(description, CMP1)?
        .ok_or(CandidateRejection::MissingExtension { box_type: CMP1 })?;
    let cdi1 = unique_extension(description, CDI1)?
        .ok_or(CandidateRejection::MissingExtension { box_type: CDI1 })?;
    let cmp1_bytes = checked_box_slice(data, cmp1)?;
    let cdi1_bytes = checked_box_slice(data, cdi1)?;
    let config = CrxConfig::parse(cmp1_bytes, cdi1_bytes)
        .map_err(|source| CandidateRejection::InvalidConfiguration { source })?;

    let sample_range = checked_sample_range(data, location)?;
    let sample_bytes = &data[sample_range];
    let sample =
        parse_crx_sample(sample_bytes).map_err(|source| CandidateRejection::InvalidSample { source })?;

    Ok(SelectedCrxSample {
        track_id,
        track_index,
        sample_index,
        description_index,
        location,
        config,
        sample_bytes,
        sample,
    })
}

fn is_native_crx_hint(description: &SampleDescription) -> bool {
    description.format == CRAW
        && description
            .extensions
            .iter()
            .any(|extension| extension.box_type == CMP1 || extension.box_type == CDI1)
}

fn unique_extension(
    description: &SampleDescription,
    wanted: FourCc,
) -> Result<Option<BoxHeader>, CandidateRejection> {
    let mut matches = description
        .extensions
        .iter()
        .copied()
        .filter(|extension| extension.box_type == wanted);
    let first = matches.next();
    if matches.next().is_some() {
        return Err(CandidateRejection::DuplicateExtension { box_type: wanted });
    }
    Ok(first)
}

fn checked_box_slice(data: &[u8], header: BoxHeader) -> Result<&[u8], CandidateRejection> {
    let end_u64 = header
        .offset
        .checked_add(header.size)
        .ok_or(CandidateRejection::BoxRangeOverflow {
            box_type: header.box_type,
        })?;
    let start = usize::try_from(header.offset).map_err(|_| CandidateRejection::BoxRangeOverflow {
        box_type: header.box_type,
    })?;
    let end = usize::try_from(end_u64).map_err(|_| CandidateRejection::BoxRangeOverflow {
        box_type: header.box_type,
    })?;
    data.get(start..end).ok_or(CandidateRejection::BoxOutsideInput {
        box_type: header.box_type,
        start: header.offset,
        end: end_u64,
        file_len: u64::try_from(data.len()).unwrap_or(u64::MAX),
    })
}

fn checked_sample_range(data: &[u8], location: SampleLocation) -> Result<Range<usize>, CandidateRejection> {
    let end_u64 = location
        .offset
        .checked_add(u64::from(location.size))
        .ok_or(CandidateRejection::SampleRangeOverflow)?;
    let start = usize::try_from(location.offset).map_err(|_| CandidateRejection::SampleRangeOverflow)?;
    let end = usize::try_from(end_u64).map_err(|_| CandidateRejection::SampleRangeOverflow)?;
    if data.get(start..end).is_none() {
        return Err(CandidateRejection::SampleOutsideInput {
            start: location.offset,
            end: end_u64,
            file_len: u64::try_from(data.len()).unwrap_or(u64::MAX),
        });
    }
    Ok(start..end)
}

fn score(candidate: &SelectedCrxSample<'_>) -> CandidateScore {
    let compression = &candidate.config.compression;
    let coded_pixels = u64::from(compression.image_width) * u64::from(compression.image_height);
    let crop_pixels = candidate
        .config
        .image_description
        .rectangles
        .first()
        .map_or(0, |rectangle| {
            let width = u64::from(rectangle.right) - u64::from(rectangle.left) + 1;
            let height = u64::from(rectangle.bottom) - u64::from(rectangle.top) + 1;
            width * height
        });
    CandidateScore {
        coded_pixels,
        crop_pixels,
        sample_bytes: candidate.location.size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cr3::bmff;

    const SAMPLE_HEADER_LEN: usize = 112;

    #[derive(Clone)]
    struct FixtureTrack {
        id: u32,
        extensions: Vec<Vec<u8>>,
        sample: Vec<u8>,
        sample_count: u32,
    }

    #[test]
    fn chooses_largest_native_track_and_ignores_jpeg_craw_preview() {
        let jpeg_preview = FixtureTrack {
            id: 1,
            extensions: vec![make_box(*b"JPEG", &[])],
            sample: vec![0xff, 0xd8, 0xff, 0xd9],
            sample_count: 1,
        };
        let reduced = native_track(2, 4, 4, &[[0, 0, 3, 3], [0, 0, 3, 3]]);
        let full = native_track(3, 8, 6, &[[1, 1, 6, 4], [0, 0, 0, 5], [1, 0, 7, 0], [1, 1, 7, 5]]);
        let bytes = make_file(&[jpeg_preview, reduced, full]);
        let file = bmff::parse(&bytes).unwrap();

        let selected = select_full_resolution_crx(&file, &bytes).unwrap();

        assert_eq!(selected.track_id, Some(3));
        assert_eq!(selected.track_index, 2);
        assert_eq!(selected.sample_index, 0);
        assert_eq!(selected.description_index, 0);
        assert_eq!(
            (
                selected.config.compression.image_width,
                selected.config.compression.image_height
            ),
            (8, 6)
        );
        assert_eq!(selected.sample_bytes.len(), SAMPLE_HEADER_LEN + 4);
        assert_eq!(selected.sample.planes.map(|plane| plane.data.len()), [1; 4]);
    }

    #[test]
    fn rejects_extra_bytes_beyond_the_crx_declared_plane_sum() {
        let reduced = native_track(2, 4, 4, &[[0, 0, 3, 3], [0, 0, 3, 3]]);
        let mut malformed_full =
            native_track(3, 8, 6, &[[1, 1, 6, 4], [0, 0, 0, 5], [1, 0, 7, 0], [1, 1, 7, 5]]);
        malformed_full.sample.push(0);
        let bytes = make_file(&[reduced, malformed_full]);
        let file = bmff::parse(&bytes).unwrap();

        let selected = select_full_resolution_crx(&file, &bytes).unwrap();

        assert_eq!(selected.track_id, Some(2));
        assert_eq!(selected.location.size as usize, SAMPLE_HEADER_LEN + 4);
    }

    #[test]
    fn rejects_input_that_is_not_the_indexed_file() {
        let bytes = make_file(&[native_track(
            3,
            8,
            6,
            &[[1, 1, 6, 4], [0, 0, 0, 5], [1, 0, 7, 0], [1, 1, 7, 5]],
        )]);
        let file = bmff::parse(&bytes).unwrap();

        let error = select_full_resolution_crx(&file, &bytes[..bytes.len() - 1]).unwrap_err();

        assert!(matches!(error, SelectError::InputLengthMismatch { .. }));
    }

    #[test]
    fn a_craw_jpeg_without_cmp1_or_cdi1_is_not_native() {
        let bytes = make_file(&[FixtureTrack {
            id: 1,
            extensions: vec![make_box(*b"JPEG", &[])],
            sample: vec![0xff, 0xd8, 0xff, 0xd9],
            sample_count: 1,
        }]);
        let file = bmff::parse(&bytes).unwrap();

        let error = select_full_resolution_crx(&file, &bytes).unwrap_err();

        assert!(matches!(error, SelectError::NoNativeCrxDescription));
    }

    #[test]
    fn selects_real_fixture_when_environment_path_is_set() {
        let Ok(path) = std::env::var("RRRAH_CR3_FIXTURE") else {
            return;
        };
        let bytes = std::fs::read(path).unwrap();
        let file = bmff::parse(&bytes).unwrap();

        let selected = select_full_resolution_crx(&file, &bytes).unwrap();

        assert_eq!(
            selected.sample_bytes.len(),
            usize::try_from(selected.location.size).unwrap()
        );
        assert!(selected.config.compression.image_width >= 6_000);
        assert!(selected.config.compression.image_height >= 4_000);
        assert_eq!(selected.sample.planes.len(), 4);
    }

    #[test]
    fn rejects_pathological_candidate_counts_before_walking_the_table() {
        let mut track = native_track(3, 8, 6, &[[1, 1, 6, 4], [0, 0, 0, 5], [1, 0, 7, 0], [1, 1, 7, 5]]);
        track.sample_count = u32::try_from(MAX_NATIVE_SAMPLE_CANDIDATES + 1).expect("test limit fits u32");
        let bytes = make_file(&[track]);
        let file = bmff::parse(&bytes).expect("compact default-size table should parse");

        assert!(matches!(
            select_full_resolution_crx(&file, &bytes),
            Err(SelectError::CandidateLimitExceeded {
                limit: MAX_NATIVE_SAMPLE_CANDIDATES
            })
        ));
    }

    #[test]
    fn rejects_equal_best_candidates_instead_of_using_file_order() {
        let geometry = [[1, 1, 6, 4], [0, 0, 0, 5], [1, 0, 7, 0], [1, 1, 7, 5]];
        let bytes = make_file(&[native_track(2, 8, 6, &geometry), native_track(3, 8, 6, &geometry)]);
        let file = bmff::parse(&bytes).expect("duplicate-score fixture should parse");

        assert!(matches!(
            select_full_resolution_crx(&file, &bytes),
            Err(SelectError::AmbiguousBestNativeCrxSample)
        ));
    }

    fn native_track(id: u32, width: u16, height: u16, rectangles: &[[u16; 4]]) -> FixtureTrack {
        FixtureTrack {
            id,
            extensions: vec![
                make_cmp1(u32::from(width), u32::from(height)),
                make_cdi1(width, height, rectangles),
            ],
            sample: make_crx_sample(),
            sample_count: 1,
        }
    }

    fn make_file(tracks: &[FixtureTrack]) -> Vec<u8> {
        let placeholder_offsets = vec![0_u32; tracks.len()];
        let placeholder_moov = make_moov(tracks, &placeholder_offsets);
        let mut next_offset = u32::try_from(placeholder_moov.len() + 8).unwrap();
        let mut offsets = Vec::with_capacity(tracks.len());
        for track in tracks {
            offsets.push(next_offset);
            next_offset = next_offset
                .checked_add(u32::try_from(track.sample.len()).unwrap())
                .unwrap();
        }
        let moov = make_moov(tracks, &offsets);
        assert_eq!(moov.len(), placeholder_moov.len());

        let mut mdat_payload = Vec::new();
        for track in tracks {
            mdat_payload.extend_from_slice(&track.sample);
        }
        let mut file = moov;
        file.extend(make_box(*b"mdat", &mdat_payload));
        file
    }

    fn make_moov(tracks: &[FixtureTrack], offsets: &[u32]) -> Vec<u8> {
        let mut payload = Vec::new();
        for (track, &offset) in tracks.iter().zip(offsets) {
            payload.extend(make_trak(track, offset));
        }
        make_box(*b"moov", &payload)
    }

    fn make_trak(track: &FixtureTrack, sample_offset: u32) -> Vec<u8> {
        let mut tkhd_payload = vec![0_u8; 12];
        tkhd_payload.extend_from_slice(&track.id.to_be_bytes());
        let tkhd = make_box(*b"tkhd", &tkhd_payload);

        let mut hdlr_payload = vec![0_u8; 8];
        hdlr_payload.extend_from_slice(b"vide");
        let hdlr = make_box(*b"hdlr", &hdlr_payload);

        let mut entry_payload = vec![0_u8; 82];
        entry_payload[7] = 1;
        for extension in &track.extensions {
            entry_payload.extend_from_slice(extension);
        }
        let entry = make_box(*b"CRAW", &entry_payload);

        let mut description_table_payload = vec![0_u8; 4];
        push_u32(&mut description_table_payload, 1);
        description_table_payload.extend(entry);
        let description_table = make_box(*b"stsd", &description_table_payload);

        let mut size_table_payload = vec![0_u8; 4];
        let sample_size = u32::try_from(track.sample.len()).unwrap();
        if track.sample_count == 1 {
            push_u32(&mut size_table_payload, 0);
            push_u32(&mut size_table_payload, 1);
            push_u32(&mut size_table_payload, sample_size);
        } else {
            push_u32(&mut size_table_payload, sample_size);
            push_u32(&mut size_table_payload, track.sample_count);
        }
        let size_table = make_box(*b"stsz", &size_table_payload);

        let mut chunk_map_payload = vec![0_u8; 4];
        push_u32(&mut chunk_map_payload, 1);
        push_u32(&mut chunk_map_payload, 1);
        push_u32(&mut chunk_map_payload, track.sample_count);
        push_u32(&mut chunk_map_payload, 1);
        let chunk_map = make_box(*b"stsc", &chunk_map_payload);

        let mut offset_table_payload = vec![0_u8; 4];
        push_u32(&mut offset_table_payload, 1);
        push_u32(&mut offset_table_payload, sample_offset);
        let offset_table = make_box(*b"stco", &offset_table_payload);

        let mut stbl_payload = Vec::new();
        stbl_payload.extend(description_table);
        stbl_payload.extend(size_table);
        stbl_payload.extend(chunk_map);
        stbl_payload.extend(offset_table);
        let stbl = make_box(*b"stbl", &stbl_payload);
        let minf = make_box(*b"minf", &stbl);

        let mut mdia_payload = hdlr;
        mdia_payload.extend(minf);
        let mdia = make_box(*b"mdia", &mdia_payload);

        let mut trak_payload = tkhd;
        trak_payload.extend(mdia);
        make_box(*b"trak", &trak_payload)
    }

    fn make_cmp1(width: u32, height: u32) -> Vec<u8> {
        let mut payload = vec![0xff, 0, 0, 0x30, 1, 0, 0, 0];
        push_u32(&mut payload, width);
        push_u32(&mut payload, height);
        push_u32(&mut payload, width);
        push_u32(&mut payload, height);
        payload.extend_from_slice(&[14, 0x40, 0, 0]);
        push_u32(&mut payload, u32::try_from(SAMPLE_HEADER_LEN).unwrap());
        push_u32(&mut payload, 0);
        for _ in 0..4 {
            payload.extend_from_slice(&[1, 1, 0, 0]);
        }
        make_box(*b"CMP1", &payload)
    }

    fn make_cdi1(width: u16, height: u16, rectangles: &[[u16; 4]]) -> Vec<u8> {
        assert!(rectangles.len() >= 2);
        let mut iad_payload = vec![0_u8; 4];
        push_u16(&mut iad_payload, width);
        push_u16(&mut iad_payload, height);
        push_u16(&mut iad_payload, 1);
        push_u16(&mut iad_payload, u16::try_from(rectangles.len() - 2).unwrap());
        push_u16(&mut iad_payload, 1);
        push_u16(&mut iad_payload, 0);
        for rectangle in rectangles {
            for value in rectangle {
                push_u16(&mut iad_payload, *value);
            }
        }
        let iad1 = make_box(*b"IAD1", &iad_payload);
        let mut cdi_payload = vec![0_u8; 4];
        cdi_payload.extend(iad1);
        make_box(*b"CDI1", &cdi_payload)
    }

    fn make_crx_sample() -> Vec<u8> {
        let mut sample = vec![0_u8; SAMPLE_HEADER_LEN + 4];
        write_segment(&mut sample, 0, [0xff, 0x01], 4, [0, 0, 0, 0]);
        for plane in 0..4 {
            let offset = 12 + plane * 24;
            let plane_tag = 0x08 + u8::try_from(plane).unwrap() * 0x10;
            write_segment(&mut sample, offset, [0xff, 0x02], 1, [plane_tag, 0, 0, 0]);
            write_segment(&mut sample, offset + 12, [0xff, 0x03], 1, [0, 0x20, 0, 0]);
            sample[SAMPLE_HEADER_LEN + plane] = u8::try_from(plane).unwrap();
        }
        sample
    }

    fn write_segment(output: &mut [u8], offset: usize, marker: [u8; 2], size: u32, descriptor: [u8; 4]) {
        output[offset..offset + 2].copy_from_slice(&marker);
        output[offset + 2..offset + 4].copy_from_slice(&8_u16.to_be_bytes());
        output[offset + 4..offset + 8].copy_from_slice(&size.to_be_bytes());
        output[offset + 8..offset + 12].copy_from_slice(&descriptor);
    }

    fn make_box(box_type: [u8; 4], payload: &[u8]) -> Vec<u8> {
        let size = u32::try_from(payload.len() + 8).unwrap();
        let mut output = Vec::with_capacity(usize::try_from(size).unwrap());
        push_u32(&mut output, size);
        output.extend_from_slice(&box_type);
        output.extend_from_slice(payload);
        output
    }

    fn push_u16(output: &mut Vec<u8>, value: u16) {
        output.extend_from_slice(&value.to_be_bytes());
    }

    fn push_u32(output: &mut Vec<u8>, value: u32) {
        output.extend_from_slice(&value.to_be_bytes());
    }
}
