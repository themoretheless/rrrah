//! Bounds-checked ISO Base Media File Format parsing for CR3 containers.
//!
//! This module deliberately stops at the sample boundary. It locates sample
//! payloads and preserves extension/unknown box ranges, but does not interpret
//! Canon entropy coding.

use std::{fmt, ops::Range};

use thiserror::Error;

const MOOV: FourCc = FourCc(*b"moov");
const TRAK: FourCc = FourCc(*b"trak");
const TKHD: FourCc = FourCc(*b"tkhd");
const MDIA: FourCc = FourCc(*b"mdia");
const HDLR: FourCc = FourCc(*b"hdlr");
const MINF: FourCc = FourCc(*b"minf");
const STBL: FourCc = FourCc(*b"stbl");
const STSD: FourCc = FourCc(*b"stsd");
const STSZ: FourCc = FourCc(*b"stsz");
const STSC: FourCc = FourCc(*b"stsc");
const STCO: FourCc = FourCc(*b"stco");
const CO64: FourCc = FourCc(*b"co64");
const UUID: FourCc = FourCc(*b"uuid");
const VIDE: FourCc = FourCc(*b"vide");
const SOUN: FourCc = FourCc(*b"soun");
const CRAW: FourCc = FourCc(*b"CRAW");

/// A four-byte ISO BMFF box or handler code.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct FourCc(pub(crate) [u8; 4]);

impl FourCc {
    pub(crate) const fn as_bytes(self) -> [u8; 4] {
        self.0
    }
}

impl fmt::Display for FourCc {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            for escaped in std::ascii::escape_default(byte) {
                formatter.write_str(char::from(escaped).encode_utf8(&mut [0; 4]))?;
            }
        }
        Ok(())
    }
}

/// Resource limits applied before allocating or descending.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ParseLimits {
    pub(crate) file_size: u64,
    pub(crate) depth: usize,
    pub(crate) boxes: usize,
    pub(crate) tracks: usize,
    pub(crate) table_entries: usize,
}

impl Default for ParseLimits {
    fn default() -> Self {
        Self {
            file_size: 16 * 1024 * 1024 * 1024,
            depth: 16,
            boxes: 200_000,
            tracks: 4_096,
            table_entries: 4_000_000,
        }
    }
}

/// Location and extent of one parsed box.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BoxHeader {
    pub(crate) box_type: FourCc,
    pub(crate) offset: u64,
    pub(crate) size: u64,
    pub(crate) header_size: u64,
    pub(crate) user_type: Option<[u8; 16]>,
}

impl BoxHeader {
    pub(crate) fn payload_range(self) -> Result<Range<usize>, ParseError> {
        let payload_start = self
            .offset
            .checked_add(self.header_size)
            .ok_or(ParseError::IntegerOverflow {
                at: self.offset,
                context: "box payload offset",
            })?;
        let end = self
            .offset
            .checked_add(self.size)
            .ok_or(ParseError::IntegerOverflow {
                at: self.offset,
                context: "box end",
            })?;
        Ok(u64_to_usize(payload_start, self.offset, "box payload offset")?
            ..u64_to_usize(end, self.offset, "box end")?)
    }
}

/// An unhandled box whose complete byte range remains available in the input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UnknownBox {
    pub(crate) parent: Option<FourCc>,
    pub(crate) header: BoxHeader,
}

/// One `stsd` entry and its nested extension boxes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SampleDescription {
    pub(crate) format: FourCc,
    pub(crate) data_reference_index: u16,
    pub(crate) entry: BoxHeader,
    pub(crate) extensions: Vec<BoxHeader>,
}

/// One resolved media sample.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SampleLocation {
    pub(crate) sample_index: usize,
    pub(crate) chunk_index: usize,
    pub(crate) description_index: u32,
    pub(crate) offset: u64,
    pub(crate) size: u32,
}

/// Parsed sample-bearing track metadata.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Track {
    pub(crate) id: Option<u32>,
    pub(crate) handler_type: Option<FourCc>,
    pub(crate) sample_descriptions: Vec<SampleDescription>,
    sample_sizes: Option<SampleSizeTable>,
    sample_to_chunk: Option<Vec<SampleToChunk>>,
    chunk_offsets: Option<Vec<u64>>,
}

impl Track {
    pub(crate) fn sample_count(&self) -> Option<usize> {
        self.sample_sizes.as_ref().map(SampleSizeTable::len)
    }

    pub(crate) fn select_sample(
        &self,
        sample_index: usize,
        file_len: u64,
    ) -> Result<SampleLocation, ParseError> {
        let sizes = self.sample_sizes.as_ref().ok_or(ParseError::MissingTable {
            track_id: self.id,
            table: STSZ,
        })?;
        if sample_index >= sizes.len() {
            return Err(ParseError::SampleIndexOutOfRange {
                track_id: self.id,
                index: sample_index,
                count: sizes.len(),
            });
        }
        for location in self.sample_locations(file_len)? {
            let location = location?;
            if location.sample_index == sample_index {
                return Ok(location);
            }
        }
        Err(ParseError::InconsistentSampleMap {
            track_id: self.id,
            index: sample_index,
        })
    }

    /// Resolves this track's sample table in one forward pass.
    ///
    /// Reusing this iterator is important for untrusted tables: repeatedly
    /// calling [`Self::select_sample`] for successive indices would otherwise
    /// rescan all preceding chunks and become quadratic.
    pub(crate) fn sample_locations(&self, file_len: u64) -> Result<SampleLocations<'_>, ParseError> {
        let sizes = self.sample_sizes.as_ref().ok_or(ParseError::MissingTable {
            track_id: self.id,
            table: STSZ,
        })?;
        let mapping = self.sample_to_chunk.as_deref().ok_or(ParseError::MissingTable {
            track_id: self.id,
            table: STSC,
        })?;
        let chunks = self.chunk_offsets.as_deref().ok_or(ParseError::MissingTable {
            track_id: self.id,
            table: STCO,
        })?;
        if sizes.len() != 0 && (mapping.is_empty() || chunks.is_empty()) {
            return Err(ParseError::InconsistentSampleMap {
                track_id: self.id,
                index: 0,
            });
        }

        Ok(SampleLocations {
            track_id: self.id,
            sizes,
            mapping,
            chunks,
            description_count: self.sample_descriptions.len(),
            file_len,
            chunk_index: 0,
            map_index: 0,
            sample_index: 0,
            sample_in_chunk: 0,
            current_offset: 0,
            failed: false,
        })
    }
}

/// A bounded, linear resolver for one track's sample locations.
pub(crate) struct SampleLocations<'a> {
    track_id: Option<u32>,
    sizes: &'a SampleSizeTable,
    mapping: &'a [SampleToChunk],
    chunks: &'a [u64],
    description_count: usize,
    file_len: u64,
    chunk_index: usize,
    map_index: usize,
    sample_index: usize,
    sample_in_chunk: u32,
    current_offset: u64,
    failed: bool,
}

impl Iterator for SampleLocations<'_> {
    type Item = Result<SampleLocation, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.sample_index >= self.sizes.len() {
            return None;
        }

        loop {
            let Some(&chunk_offset) = self.chunks.get(self.chunk_index) else {
                self.failed = true;
                return Some(Err(ParseError::InconsistentSampleMap {
                    track_id: self.track_id,
                    index: self.sample_index,
                }));
            };
            let Some(chunk_number) = self
                .chunk_index
                .checked_add(1)
                .and_then(|number| u32::try_from(number).ok())
            else {
                self.failed = true;
                return Some(Err(ParseError::IntegerOverflow {
                    at: chunk_offset,
                    context: "chunk number",
                }));
            };

            if self.sample_in_chunk == 0 {
                while self.map_index + 1 < self.mapping.len()
                    && self.mapping[self.map_index + 1].first_chunk <= chunk_number
                {
                    self.map_index += 1;
                }
                self.current_offset = chunk_offset;
            }

            let map = self.mapping[self.map_index];
            if self.sample_in_chunk >= map.samples_per_chunk {
                self.chunk_index += 1;
                self.sample_in_chunk = 0;
                continue;
            }

            let Ok(description_usize) = usize::try_from(map.description_index) else {
                self.failed = true;
                return Some(Err(ParseError::IntegerOverflow {
                    at: chunk_offset,
                    context: "sample-description index",
                }));
            };
            if description_usize == 0 || description_usize > self.description_count {
                self.failed = true;
                return Some(Err(ParseError::InconsistentSampleMap {
                    track_id: self.track_id,
                    index: self.sample_index,
                }));
            }

            let size = self.sizes.get(self.sample_index);
            let Some(end) = self.current_offset.checked_add(u64::from(size)) else {
                self.failed = true;
                return Some(Err(ParseError::IntegerOverflow {
                    at: self.current_offset,
                    context: "sample end",
                }));
            };
            if end > self.file_len {
                self.failed = true;
                return Some(Err(ParseError::SampleOutsideFile {
                    track_id: self.track_id,
                    index: self.sample_index,
                    offset: self.current_offset,
                    end,
                    file_len: self.file_len,
                }));
            }

            let location = SampleLocation {
                sample_index: self.sample_index,
                chunk_index: self.chunk_index,
                description_index: map.description_index,
                offset: self.current_offset,
                size,
            };
            self.current_offset = end;
            self.sample_in_chunk += 1;
            self.sample_index += 1;
            return Some(Ok(location));
        }
    }
}

/// Parsed CR3 container index. All byte ranges refer to the original input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BmffFile {
    pub(crate) file_len: u64,
    pub(crate) top_level_boxes: Vec<BoxHeader>,
    pub(crate) tracks: Vec<Track>,
    pub(crate) unknown_boxes: Vec<UnknownBox>,
}

impl BmffFile {
    pub(crate) fn track(&self, track_id: u32) -> Option<&Track> {
        self.tracks.iter().find(|track| track.id == Some(track_id))
    }

    pub(crate) fn select_sample(
        &self,
        track_id: u32,
        sample_index: usize,
    ) -> Result<SampleLocation, ParseError> {
        let track = self.track(track_id).ok_or(ParseError::TrackNotFound(track_id))?;
        track.select_sample(sample_index, self.file_len)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SampleSizeTable {
    default_size: u32,
    sample_count: usize,
    sizes: Vec<u32>,
}

impl SampleSizeTable {
    fn len(&self) -> usize {
        self.sample_count
    }

    fn get(&self, index: usize) -> u32 {
        if self.default_size == 0 {
            self.sizes[index]
        } else {
            self.default_size
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SampleToChunk {
    first_chunk: u32,
    samples_per_chunk: u32,
    description_index: u32,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub(crate) enum ParseError {
    #[error("file is {actual} bytes, above the configured {limit}-byte limit")]
    FileTooLarge { actual: u64, limit: u64 },
    #[error("{kind} limit {limit} exceeded")]
    LimitExceeded { kind: &'static str, limit: usize },
    #[error("could not allocate storage for {count} {kind}")]
    AllocationFailed { kind: &'static str, count: usize },
    #[error("truncated data at byte {at}: need {needed} bytes, only {available} remain in this region")]
    Truncated {
        at: u64,
        needed: usize,
        available: usize,
    },
    #[error("box at byte {at} declares size {declared}, below its {header_size}-byte header")]
    InvalidBoxSize {
        at: u64,
        declared: u64,
        header_size: u64,
    },
    #[error("box at byte {at} ends at {end}, beyond parent end {parent_end}")]
    BoxOutsideParent { at: u64, end: u64, parent_end: u64 },
    #[error("integer overflow at byte {at} while computing {context}")]
    IntegerOverflow { at: u64, context: &'static str },
    #[error("unsupported version {version} in {box_type} box at byte {at}")]
    UnsupportedVersion { box_type: FourCc, version: u8, at: u64 },
    #[error("invalid {box_type} box at byte {at}: {reason}")]
    InvalidTable {
        box_type: FourCc,
        at: u64,
        reason: &'static str,
    },
    #[error("{box_type} box at byte {at} has {count} unexpected trailing bytes")]
    TrailingBytes { box_type: FourCc, at: u64, count: usize },
    #[error("track {track_id:?} is missing its {table} table")]
    MissingTable { track_id: Option<u32>, table: FourCc },
    #[error("sample {index} is outside track {track_id:?}'s {count}-sample table")]
    SampleIndexOutOfRange {
        track_id: Option<u32>,
        index: usize,
        count: usize,
    },
    #[error("sample {index} in track {track_id:?} maps beyond the available chunk table")]
    InconsistentSampleMap { track_id: Option<u32>, index: usize },
    #[error("sample {index} in track {track_id:?} spans [{offset}, {end}), beyond file length {file_len}")]
    SampleOutsideFile {
        track_id: Option<u32>,
        index: usize,
        offset: u64,
        end: u64,
        file_len: u64,
    },
    #[error("track id {0} was not found")]
    TrackNotFound(u32),
    #[error("duplicate track id {0}")]
    DuplicateTrackId(u32),
}

pub(crate) fn parse(data: &[u8]) -> Result<BmffFile, ParseError> {
    parse_with_limits(data, ParseLimits::default())
}

pub(crate) fn parse_with_limits(data: &[u8], limits: ParseLimits) -> Result<BmffFile, ParseError> {
    Parser::new(data, limits)?.parse()
}

#[derive(Clone, Copy, Debug)]
struct ParsedBox {
    header: BoxHeader,
    payload_start: usize,
    end: usize,
}

struct Parser<'a> {
    data: &'a [u8],
    limits: ParseLimits,
    box_count: usize,
    unknown_boxes: Vec<UnknownBox>,
}

impl<'a> Parser<'a> {
    fn new(data: &'a [u8], limits: ParseLimits) -> Result<Self, ParseError> {
        let file_len = u64::try_from(data.len()).map_err(|_| ParseError::IntegerOverflow {
            at: 0,
            context: "file length",
        })?;
        if file_len > limits.file_size {
            return Err(ParseError::FileTooLarge {
                actual: file_len,
                limit: limits.file_size,
            });
        }
        Ok(Self {
            data,
            limits,
            box_count: 0,
            unknown_boxes: Vec::new(),
        })
    }

    fn parse(mut self) -> Result<BmffFile, ParseError> {
        let top_level = self.children(0, self.data.len(), 0)?;
        let mut tracks = Vec::new();
        let mut saw_moov = false;

        for parsed in &top_level {
            if parsed.header.box_type == MOOV {
                if saw_moov {
                    return Err(Self::invalid(*parsed, "duplicate moov box"));
                }
                saw_moov = true;
                self.parse_moov(*parsed, &mut tracks, 1)?;
            } else {
                self.preserve(None, parsed.header);
            }
        }

        for (index, track) in tracks.iter().enumerate() {
            if let Some(id) = track.id
                && tracks[..index].iter().any(|previous| previous.id == Some(id))
            {
                return Err(ParseError::DuplicateTrackId(id));
            }
        }

        Ok(BmffFile {
            file_len: u64::try_from(self.data.len()).map_err(|_| ParseError::IntegerOverflow {
                at: 0,
                context: "file length",
            })?,
            top_level_boxes: top_level.iter().map(|parsed| parsed.header).collect(),
            tracks,
            unknown_boxes: self.unknown_boxes,
        })
    }

    fn parse_moov(
        &mut self,
        moov: ParsedBox,
        tracks: &mut Vec<Track>,
        depth: usize,
    ) -> Result<(), ParseError> {
        for child in self.children(moov.payload_start, moov.end, depth)? {
            if child.header.box_type == TRAK {
                if tracks.len() >= self.limits.tracks {
                    return Err(ParseError::LimitExceeded {
                        kind: "track count",
                        limit: self.limits.tracks,
                    });
                }
                tracks.push(self.parse_trak(child, depth + 1)?);
            } else {
                self.preserve(Some(MOOV), child.header);
            }
        }
        Ok(())
    }

    fn parse_trak(&mut self, trak_box: ParsedBox, depth: usize) -> Result<Track, ParseError> {
        let children = self.children(trak_box.payload_start, trak_box.end, depth)?;
        let mut parsed_track = Track::default();
        let mut saw_tkhd = false;
        let mut saw_mdia = false;

        for child in &children {
            if child.header.box_type == TKHD {
                if saw_tkhd {
                    return Err(Self::invalid(*child, "duplicate tkhd box"));
                }
                saw_tkhd = true;
                parsed_track.id = Some(self.parse_tkhd(*child)?);
            }
        }
        for child in children {
            match child.header.box_type {
                MDIA => {
                    if saw_mdia {
                        return Err(Self::invalid(child, "duplicate mdia box"));
                    }
                    saw_mdia = true;
                    self.parse_mdia(child, &mut parsed_track, depth + 1)?;
                }
                TKHD => {}
                _ => self.preserve(Some(TRAK), child.header),
            }
        }
        Ok(parsed_track)
    }

    fn parse_mdia(&mut self, mdia: ParsedBox, track: &mut Track, depth: usize) -> Result<(), ParseError> {
        let children = self.children(mdia.payload_start, mdia.end, depth)?;
        let mut saw_hdlr = false;
        let mut saw_minf = false;
        for child in &children {
            if child.header.box_type == HDLR {
                if saw_hdlr {
                    return Err(Self::invalid(*child, "duplicate hdlr box"));
                }
                saw_hdlr = true;
                track.handler_type = Some(self.parse_hdlr(*child)?);
            }
        }
        for child in children {
            match child.header.box_type {
                MINF => {
                    if saw_minf {
                        return Err(Self::invalid(child, "duplicate minf box"));
                    }
                    saw_minf = true;
                    self.parse_minf(child, track, depth + 1)?;
                }
                HDLR => {}
                _ => self.preserve(Some(MDIA), child.header),
            }
        }
        Ok(())
    }

    fn parse_minf(&mut self, minf: ParsedBox, track: &mut Track, depth: usize) -> Result<(), ParseError> {
        let mut saw_stbl = false;
        for child in self.children(minf.payload_start, minf.end, depth)? {
            if child.header.box_type == STBL {
                if saw_stbl {
                    return Err(Self::invalid(child, "duplicate stbl box"));
                }
                saw_stbl = true;
                self.parse_stbl(child, track, depth + 1)?;
            } else {
                self.preserve(Some(MINF), child.header);
            }
        }
        Ok(())
    }

    fn parse_stbl(&mut self, stbl: ParsedBox, track: &mut Track, depth: usize) -> Result<(), ParseError> {
        let mut saw_stsd = false;
        for child in self.children(stbl.payload_start, stbl.end, depth)? {
            match child.header.box_type {
                STSD => {
                    if saw_stsd {
                        return Err(Self::invalid(child, "duplicate stsd table"));
                    }
                    saw_stsd = true;
                    track.sample_descriptions = self.parse_stsd(child, track.handler_type, depth + 1)?;
                }
                STSZ => {
                    if track.sample_sizes.is_some() {
                        return Err(Self::invalid(child, "duplicate stsz table"));
                    }
                    track.sample_sizes = Some(self.parse_stsz(child)?);
                }
                STSC => {
                    if track.sample_to_chunk.is_some() {
                        return Err(Self::invalid(child, "duplicate stsc table"));
                    }
                    track.sample_to_chunk = Some(self.parse_stsc(child)?);
                }
                STCO | CO64 => {
                    if track.chunk_offsets.is_some() {
                        return Err(Self::invalid(child, "duplicate chunk-offset table"));
                    }
                    track.chunk_offsets = Some(self.parse_chunk_offsets(child)?);
                }
                _ => self.preserve(Some(STBL), child.header),
            }
        }
        Ok(())
    }

    fn parse_tkhd(&self, parsed: ParsedBox) -> Result<u32, ParseError> {
        let mut reader = Reader::new(self.data, parsed.payload_start, parsed.end);
        let version = reader.read_u8()?;
        reader.skip(3)?;
        match version {
            0 => reader.skip(8)?,
            1 => reader.skip(16)?,
            _ => return Err(Self::unsupported(parsed, version)),
        }
        let track_id = reader.read_u32()?;
        if track_id == 0 {
            return Err(Self::invalid(parsed, "track id must be non-zero"));
        }
        Ok(track_id)
    }

    fn parse_hdlr(&self, parsed: ParsedBox) -> Result<FourCc, ParseError> {
        let mut reader = Reader::new(self.data, parsed.payload_start, parsed.end);
        let version = reader.read_u8()?;
        reader.skip(3)?;
        if version != 0 {
            return Err(Self::unsupported(parsed, version));
        }
        reader.skip(4)?;
        Ok(FourCc(reader.read_array::<4>()?))
    }

    fn parse_stsd(
        &mut self,
        parsed: ParsedBox,
        handler: Option<FourCc>,
        depth: usize,
    ) -> Result<Vec<SampleDescription>, ParseError> {
        let mut reader = Reader::new(self.data, parsed.payload_start, parsed.end);
        Self::read_version_zero(&mut reader, parsed)?;
        let count = self.read_count(&mut reader, "sample-description entry count")?;
        Self::require_count_bytes(&reader, count, 16, parsed)?;
        let mut descriptions = Vec::new();
        descriptions
            .try_reserve_exact(count)
            .map_err(|_| ParseError::AllocationFailed {
                kind: "sample descriptions",
                count,
            })?;

        for _ in 0..count {
            let entry = self.next_box(reader.position(), parsed.end)?;
            reader.set_position(entry.end)?;
            let mut entry_reader = Reader::new(self.data, entry.payload_start, entry.end);
            entry_reader.skip(6)?;
            let data_reference_index = entry_reader.read_u16()?;
            if data_reference_index == 0 {
                return Err(Self::invalid(entry, "data-reference index must be non-zero"));
            }

            // Canon's CRAW entry in the fixture retains the standard visual
            // sample-entry fields followed by one additional reserved u32.
            // Treat it as opaque padding; interpretation belongs to the CRX
            // layer, while child boxes still follow ordinary BMFF framing.
            let fixed_fields = match (handler, entry.header.box_type) {
                (Some(VIDE), CRAW) => 82,
                (Some(VIDE), _) => 78,
                (Some(SOUN), _) => 28,
                _ => entry.end - entry.payload_start,
            };
            let extension_start =
                entry
                    .payload_start
                    .checked_add(fixed_fields)
                    .ok_or(ParseError::IntegerOverflow {
                        at: entry.header.offset,
                        context: "sample-entry header",
                    })?;
            if extension_start > entry.end {
                return Err(Self::invalid(entry, "truncated sample-entry fields"));
            }
            let extensions = self
                .children(extension_start, entry.end, depth)?
                .into_iter()
                .map(|extension| extension.header)
                .collect();
            descriptions.push(SampleDescription {
                format: entry.header.box_type,
                data_reference_index,
                entry: entry.header,
                extensions,
            });
        }
        Self::require_end(reader, parsed)?;
        Ok(descriptions)
    }

    fn parse_stsz(&self, parsed: ParsedBox) -> Result<SampleSizeTable, ParseError> {
        let mut reader = Reader::new(self.data, parsed.payload_start, parsed.end);
        Self::read_version_zero(&mut reader, parsed)?;
        let default_size = reader.read_u32()?;
        let sample_count = self.read_count(&mut reader, "sample-size entry count")?;
        let mut sizes = Vec::new();
        if default_size == 0 {
            Self::require_count_bytes(&reader, sample_count, 4, parsed)?;
            sizes
                .try_reserve_exact(sample_count)
                .map_err(|_| ParseError::AllocationFailed {
                    kind: "sample sizes",
                    count: sample_count,
                })?;
            for _ in 0..sample_count {
                sizes.push(reader.read_u32()?);
            }
        }
        Self::require_end(reader, parsed)?;
        Ok(SampleSizeTable {
            default_size,
            sample_count,
            sizes,
        })
    }

    fn parse_stsc(&self, parsed: ParsedBox) -> Result<Vec<SampleToChunk>, ParseError> {
        let mut reader = Reader::new(self.data, parsed.payload_start, parsed.end);
        Self::read_version_zero(&mut reader, parsed)?;
        let count = self.read_count(&mut reader, "sample-to-chunk entry count")?;
        Self::require_count_bytes(&reader, count, 12, parsed)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(count)
            .map_err(|_| ParseError::AllocationFailed {
                kind: "sample-to-chunk entries",
                count,
            })?;
        for index in 0..count {
            let entry = SampleToChunk {
                first_chunk: reader.read_u32()?,
                samples_per_chunk: reader.read_u32()?,
                description_index: reader.read_u32()?,
            };
            if entry.first_chunk == 0 || entry.samples_per_chunk == 0 || entry.description_index == 0 {
                return Err(Self::invalid(parsed, "stsc fields must be non-zero"));
            }
            if index == 0 && entry.first_chunk != 1 {
                return Err(Self::invalid(parsed, "first stsc entry must begin at chunk 1"));
            }
            if entries
                .last()
                .is_some_and(|previous: &SampleToChunk| previous.first_chunk >= entry.first_chunk)
            {
                return Err(Self::invalid(parsed, "stsc first-chunk values must increase"));
            }
            entries.push(entry);
        }
        Self::require_end(reader, parsed)?;
        Ok(entries)
    }

    fn parse_chunk_offsets(&self, parsed: ParsedBox) -> Result<Vec<u64>, ParseError> {
        let mut reader = Reader::new(self.data, parsed.payload_start, parsed.end);
        Self::read_version_zero(&mut reader, parsed)?;
        let count = self.read_count(&mut reader, "chunk-offset entry count")?;
        let encoded_size = if parsed.header.box_type == STCO { 4 } else { 8 };
        Self::require_count_bytes(&reader, count, encoded_size, parsed)?;
        let mut offsets = Vec::new();
        offsets
            .try_reserve_exact(count)
            .map_err(|_| ParseError::AllocationFailed {
                kind: "chunk offsets",
                count,
            })?;
        for _ in 0..count {
            offsets.push(if parsed.header.box_type == STCO {
                u64::from(reader.read_u32()?)
            } else {
                reader.read_u64()?
            });
        }
        Self::require_end(reader, parsed)?;
        Ok(offsets)
    }

    fn read_version_zero(reader: &mut Reader<'_>, parsed: ParsedBox) -> Result<(), ParseError> {
        let version = reader.read_u8()?;
        reader.skip(3)?;
        if version != 0 {
            return Err(Self::unsupported(parsed, version));
        }
        Ok(())
    }

    fn read_count(&self, reader: &mut Reader<'_>, kind: &'static str) -> Result<usize, ParseError> {
        let count = usize::try_from(reader.read_u32()?).map_err(|_| ParseError::IntegerOverflow {
            at: u64::try_from(reader.position()).unwrap_or(u64::MAX),
            context: "table entry count",
        })?;
        if count > self.limits.table_entries {
            return Err(ParseError::LimitExceeded {
                kind,
                limit: self.limits.table_entries,
            });
        }
        Ok(count)
    }

    fn require_end(reader: Reader<'_>, parsed: ParsedBox) -> Result<(), ParseError> {
        if reader.remaining() == 0 {
            Ok(())
        } else {
            Err(ParseError::TrailingBytes {
                box_type: parsed.header.box_type,
                at: parsed.header.offset,
                count: reader.remaining(),
            })
        }
    }

    fn require_count_bytes(
        reader: &Reader<'_>,
        count: usize,
        encoded_size: usize,
        parsed: ParsedBox,
    ) -> Result<(), ParseError> {
        let needed = count
            .checked_mul(encoded_size)
            .ok_or(ParseError::IntegerOverflow {
                at: parsed.header.offset,
                context: "table byte length",
            })?;
        if needed > reader.remaining() {
            return Err(ParseError::Truncated {
                at: usize_to_u64(reader.position()),
                needed,
                available: reader.remaining(),
            });
        }
        Ok(())
    }

    fn unsupported(parsed: ParsedBox, version: u8) -> ParseError {
        ParseError::UnsupportedVersion {
            box_type: parsed.header.box_type,
            version,
            at: parsed.header.offset,
        }
    }

    fn invalid(parsed: ParsedBox, reason: &'static str) -> ParseError {
        ParseError::InvalidTable {
            box_type: parsed.header.box_type,
            at: parsed.header.offset,
            reason,
        }
    }

    fn preserve(&mut self, parent: Option<FourCc>, header: BoxHeader) {
        self.unknown_boxes.push(UnknownBox { parent, header });
    }

    fn children(&mut self, start: usize, end: usize, depth: usize) -> Result<Vec<ParsedBox>, ParseError> {
        if depth > self.limits.depth {
            return Err(ParseError::LimitExceeded {
                kind: "box nesting depth",
                limit: self.limits.depth,
            });
        }
        let mut boxes = Vec::new();
        let mut cursor = start;
        while cursor < end {
            let parsed = self.next_box(cursor, end)?;
            cursor = parsed.end;
            boxes.push(parsed);
        }
        Ok(boxes)
    }

    fn next_box(&mut self, offset: usize, parent_end: usize) -> Result<ParsedBox, ParseError> {
        if self.box_count >= self.limits.boxes {
            return Err(ParseError::LimitExceeded {
                kind: "box count",
                limit: self.limits.boxes,
            });
        }
        self.box_count += 1;

        let mut reader = Reader::new(self.data, offset, parent_end);
        let size32 = reader.read_u32()?;
        let box_type = FourCc(reader.read_array::<4>()?);
        let declared_size = if size32 == 1 {
            reader.read_u64()?
        } else if size32 == 0 {
            u64::try_from(parent_end - offset).map_err(|_| ParseError::IntegerOverflow {
                at: usize_to_u64(offset),
                context: "open-ended box size",
            })?
        } else {
            u64::from(size32)
        };
        let user_type = if box_type == UUID {
            Some(reader.read_array::<16>()?)
        } else {
            None
        };
        let header_size =
            u64::try_from(reader.position() - offset).map_err(|_| ParseError::IntegerOverflow {
                at: usize_to_u64(offset),
                context: "box header size",
            })?;
        if declared_size < header_size {
            return Err(ParseError::InvalidBoxSize {
                at: usize_to_u64(offset),
                declared: declared_size,
                header_size,
            });
        }
        let declared_bytes = u64_to_usize(declared_size, usize_to_u64(offset), "box size")?;
        let end = offset
            .checked_add(declared_bytes)
            .ok_or(ParseError::IntegerOverflow {
                at: usize_to_u64(offset),
                context: "box end",
            })?;
        if end > parent_end {
            return Err(ParseError::BoxOutsideParent {
                at: usize_to_u64(offset),
                end: usize_to_u64(end),
                parent_end: usize_to_u64(parent_end),
            });
        }

        Ok(ParsedBox {
            header: BoxHeader {
                box_type,
                offset: usize_to_u64(offset),
                size: declared_size,
                header_size,
                user_type,
            },
            payload_start: reader.position(),
            end,
        })
    }
}

#[cfg(test)]
fn resolve_sample(
    track_id: Option<u32>,
    sizes: &SampleSizeTable,
    mapping: &[SampleToChunk],
    chunks: &[u64],
    description_count: usize,
    wanted: usize,
    file_len: u64,
) -> Result<SampleLocation, ParseError> {
    if mapping.is_empty() || chunks.is_empty() {
        return Err(ParseError::InconsistentSampleMap {
            track_id,
            index: wanted,
        });
    }
    let mut map_index = 0;
    let mut sample_index = 0;

    for (zero_based_chunk, &chunk_offset) in chunks.iter().enumerate() {
        let chunk_number = u32::try_from(zero_based_chunk + 1).map_err(|_| ParseError::IntegerOverflow {
            at: chunk_offset,
            context: "chunk number",
        })?;
        while map_index + 1 < mapping.len() && mapping[map_index + 1].first_chunk <= chunk_number {
            map_index += 1;
        }
        let map = mapping[map_index];
        let description_usize =
            usize::try_from(map.description_index).map_err(|_| ParseError::IntegerOverflow {
                at: chunk_offset,
                context: "sample-description index",
            })?;
        if description_usize == 0 || description_usize > description_count {
            return Err(ParseError::InconsistentSampleMap {
                track_id,
                index: sample_index,
            });
        }

        let mut offset = chunk_offset;
        for _ in 0..map.samples_per_chunk {
            if sample_index >= sizes.len() {
                return Err(ParseError::InconsistentSampleMap {
                    track_id,
                    index: sample_index,
                });
            }
            let size = sizes.get(sample_index);
            let end = offset
                .checked_add(u64::from(size))
                .ok_or(ParseError::IntegerOverflow {
                    at: offset,
                    context: "sample end",
                })?;
            if end > file_len {
                return Err(ParseError::SampleOutsideFile {
                    track_id,
                    index: sample_index,
                    offset,
                    end,
                    file_len,
                });
            }
            if sample_index == wanted {
                return Ok(SampleLocation {
                    sample_index,
                    chunk_index: zero_based_chunk,
                    description_index: map.description_index,
                    offset,
                    size,
                });
            }
            offset = end;
            sample_index += 1;
        }
    }

    Err(ParseError::InconsistentSampleMap {
        track_id,
        index: wanted,
    })
}

#[derive(Clone, Copy)]
struct Reader<'a> {
    data: &'a [u8],
    position: usize,
    end: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8], position: usize, end: usize) -> Self {
        Self { data, position, end }
    }

    fn position(self) -> usize {
        self.position
    }

    fn remaining(self) -> usize {
        self.end.saturating_sub(self.position)
    }

    fn set_position(&mut self, position: usize) -> Result<(), ParseError> {
        if position > self.end {
            return Err(ParseError::Truncated {
                at: usize_to_u64(self.position),
                needed: position - self.position,
                available: self.remaining(),
            });
        }
        self.position = position;
        Ok(())
    }

    fn skip(&mut self, count: usize) -> Result<(), ParseError> {
        let next = self
            .position
            .checked_add(count)
            .ok_or(ParseError::IntegerOverflow {
                at: usize_to_u64(self.position),
                context: "reader position",
            })?;
        self.set_position(next)
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], ParseError> {
        let next = self.position.checked_add(N).ok_or(ParseError::IntegerOverflow {
            at: usize_to_u64(self.position),
            context: "reader position",
        })?;
        if next > self.end {
            return Err(ParseError::Truncated {
                at: usize_to_u64(self.position),
                needed: N,
                available: self.remaining(),
            });
        }
        let mut value = [0; N];
        value.copy_from_slice(&self.data[self.position..next]);
        self.position = next;
        Ok(value)
    }

    fn read_u8(&mut self) -> Result<u8, ParseError> {
        Ok(self.read_array::<1>()?[0])
    }

    fn read_u16(&mut self) -> Result<u16, ParseError> {
        Ok(u16::from_be_bytes(self.read_array()?))
    }

    fn read_u32(&mut self) -> Result<u32, ParseError> {
        Ok(u32::from_be_bytes(self.read_array()?))
    }

    fn read_u64(&mut self) -> Result<u64, ParseError> {
        Ok(u64::from_be_bytes(self.read_array()?))
    }
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn u64_to_usize(value: u64, at: u64, context: &'static str) -> Result<usize, ParseError> {
    usize::try_from(value).map_err(|_| ParseError::IntegerOverflow { at, context })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CRAW: FourCc = FourCc(*b"CRAW");
    const CMP1: FourCc = FourCc(*b"CMP1");

    fn boxed(kind: [u8; 4], payload: &[u8]) -> Vec<u8> {
        let size = u32::try_from(payload.len() + 8).expect("synthetic box fits u32");
        let mut bytes = Vec::with_capacity(size as usize);
        bytes.extend_from_slice(&size.to_be_bytes());
        bytes.extend_from_slice(&kind);
        bytes.extend_from_slice(payload);
        bytes
    }

    fn full_box_payload(fields: &[u8]) -> Vec<u8> {
        let mut payload = vec![0, 0, 0, 0];
        payload.extend_from_slice(fields);
        payload
    }

    fn tkhd(track_id: u32) -> Vec<u8> {
        let mut payload = full_box_payload(&[0; 8]);
        payload.extend_from_slice(&track_id.to_be_bytes());
        boxed(*b"tkhd", &payload)
    }

    fn hdlr(handler: [u8; 4]) -> Vec<u8> {
        let mut payload = full_box_payload(&[0; 4]);
        payload.extend_from_slice(&handler);
        boxed(*b"hdlr", &payload)
    }

    fn stsd() -> Vec<u8> {
        let extension = boxed(*b"CMP1", &[1, 2, 3, 4]);
        let mut entry_payload = vec![0; 82];
        entry_payload[7] = 1;
        entry_payload.extend_from_slice(&extension);
        let entry = boxed(*b"CRAW", &entry_payload);
        let mut fields = 1_u32.to_be_bytes().to_vec();
        fields.extend_from_slice(&entry);
        boxed(*b"stsd", &full_box_payload(&fields))
    }

    fn stsz(sample_sizes: &[u32]) -> Vec<u8> {
        let mut fields = 0_u32.to_be_bytes().to_vec();
        fields.extend_from_slice(
            &u32::try_from(sample_sizes.len())
                .expect("count fits")
                .to_be_bytes(),
        );
        for size in sample_sizes {
            fields.extend_from_slice(&size.to_be_bytes());
        }
        boxed(*b"stsz", &full_box_payload(&fields))
    }

    fn stsc(samples_per_chunk: u32) -> Vec<u8> {
        let mut fields = 1_u32.to_be_bytes().to_vec();
        fields.extend_from_slice(&1_u32.to_be_bytes());
        fields.extend_from_slice(&samples_per_chunk.to_be_bytes());
        fields.extend_from_slice(&1_u32.to_be_bytes());
        boxed(*b"stsc", &full_box_payload(&fields))
    }

    fn stco(offsets: &[u32]) -> Vec<u8> {
        let mut fields = u32::try_from(offsets.len())
            .expect("count fits")
            .to_be_bytes()
            .to_vec();
        for offset in offsets {
            fields.extend_from_slice(&offset.to_be_bytes());
        }
        boxed(*b"stco", &full_box_payload(&fields))
    }

    fn co64(offsets: &[u64]) -> Vec<u8> {
        let mut fields = u32::try_from(offsets.len())
            .expect("count fits")
            .to_be_bytes()
            .to_vec();
        for offset in offsets {
            fields.extend_from_slice(&offset.to_be_bytes());
        }
        boxed(*b"co64", &full_box_payload(&fields))
    }

    fn synthetic_file_with_offsets(chunk_offsets: &[u8]) -> Vec<u8> {
        let mut stbl_payload = Vec::new();
        stbl_payload.extend_from_slice(&stsd());
        stbl_payload.extend_from_slice(&stsz(&[3, 4]));
        stbl_payload.extend_from_slice(&stsc(2));
        stbl_payload.extend_from_slice(chunk_offsets);
        stbl_payload.extend_from_slice(&boxed(*b"free", &[9]));
        let stbl = boxed(*b"stbl", &stbl_payload);
        let minf = boxed(*b"minf", &stbl);
        let mut mdia_payload = hdlr(*b"vide");
        mdia_payload.extend_from_slice(&minf);
        let mdia = boxed(*b"mdia", &mdia_payload);
        let mut trak_payload = tkhd(3);
        trak_payload.extend_from_slice(&mdia);
        let trak = boxed(*b"trak", &trak_payload);
        let moov = boxed(*b"moov", &trak);
        let mut file = boxed(*b"ftyp", &[0; 16]);
        file.extend_from_slice(&moov);
        file.resize(519, 0);
        file
    }

    fn synthetic_file() -> Vec<u8> {
        synthetic_file_with_offsets(&stco(&[512]))
    }

    #[test]
    fn parses_track_extensions_and_resolves_samples() {
        let file = synthetic_file();
        let parsed = parse(&file).expect("synthetic BMFF parses");
        let track = parsed.track(3).expect("track 3");
        assert_eq!(track.handler_type, Some(VIDE));
        assert_eq!(track.sample_count(), Some(2));
        assert_eq!(track.sample_descriptions[0].format, CRAW);
        assert_eq!(track.sample_descriptions[0].extensions[0].box_type, CMP1);
        assert_eq!(
            parsed.select_sample(3, 0).expect("sample zero"),
            SampleLocation {
                sample_index: 0,
                chunk_index: 0,
                description_index: 1,
                offset: 512,
                size: 3,
            }
        );
        assert_eq!(parsed.select_sample(3, 1).expect("sample one").offset, 515);
        assert_eq!(
            track
                .sample_locations(parsed.file_len)
                .expect("sample iterator")
                .collect::<Result<Vec<_>, _>>()
                .expect("linear sample resolution")
                .iter()
                .map(|sample| sample.offset)
                .collect::<Vec<_>>(),
            vec![512, 515]
        );
        assert!(
            parsed.unknown_boxes.iter().any(|unknown| {
                unknown.parent == Some(STBL) && unknown.header.box_type == FourCc(*b"free")
            })
        );
    }

    #[test]
    fn handles_large_open_ended_and_uuid_headers() {
        let mut large = Vec::new();
        large.extend_from_slice(&1_u32.to_be_bytes());
        large.extend_from_slice(b"wide");
        large.extend_from_slice(&16_u64.to_be_bytes());

        let user_type = [7_u8; 16];
        let mut uuid = Vec::new();
        uuid.extend_from_slice(&0_u32.to_be_bytes());
        uuid.extend_from_slice(b"uuid");
        uuid.extend_from_slice(&user_type);

        let mut bytes = large;
        bytes.extend_from_slice(&uuid);
        let parsed = parse(&bytes).expect("header variants parse");
        assert_eq!(parsed.top_level_boxes[0].header_size, 16);
        assert_eq!(parsed.top_level_boxes[1].size, 24);
        assert_eq!(parsed.top_level_boxes[1].header_size, 24);
        assert_eq!(parsed.top_level_boxes[1].user_type, Some(user_type));
    }

    #[test]
    fn parses_64_bit_chunk_offsets() {
        let file = synthetic_file_with_offsets(&co64(&[512]));
        let parsed = parse(&file).expect("co64 synthetic BMFF parses");
        assert_eq!(parsed.select_sample(3, 1).expect("second sample").offset, 515);
    }

    #[test]
    fn rejects_truncated_base_and_extended_headers() {
        assert!(matches!(parse(&[0, 0, 0, 8]), Err(ParseError::Truncated { .. })));
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1_u32.to_be_bytes());
        bytes.extend_from_slice(b"mdat");
        bytes.extend_from_slice(&[0; 4]);
        assert!(matches!(parse(&bytes), Err(ParseError::Truncated { .. })));
    }

    #[test]
    fn rejects_boxes_smaller_than_headers_or_outside_parent() {
        let mut too_small = Vec::new();
        too_small.extend_from_slice(&4_u32.to_be_bytes());
        too_small.extend_from_slice(b"free");
        assert!(matches!(
            parse(&too_small),
            Err(ParseError::InvalidBoxSize { .. })
        ));

        let mut too_large = Vec::new();
        too_large.extend_from_slice(&99_u32.to_be_bytes());
        too_large.extend_from_slice(b"free");
        assert!(matches!(
            parse(&too_large),
            Err(ParseError::BoxOutsideParent { .. })
        ));

        let mut overflowing = boxed(*b"free", &[]);
        overflowing.extend_from_slice(&1_u32.to_be_bytes());
        overflowing.extend_from_slice(b"wide");
        overflowing.extend_from_slice(&u64::MAX.to_be_bytes());
        assert!(matches!(
            parse(&overflowing),
            Err(ParseError::IntegerOverflow { .. })
        ));
    }

    #[test]
    fn rejects_malformed_table_counts_before_allocating() {
        let stsz = boxed(
            *b"stsz",
            &full_box_payload(&[
                0, 0, 0, 0, // default size
                0, 0, 0, 2, // two entries, but no entry bytes
            ]),
        );
        let stbl = boxed(*b"stbl", &stsz);
        let minf = boxed(*b"minf", &stbl);
        let mdia = boxed(*b"mdia", &minf);
        let trak = boxed(*b"trak", &mdia);
        let moov = boxed(*b"moov", &trak);
        assert!(matches!(parse(&moov), Err(ParseError::Truncated { .. })));

        let limits = ParseLimits {
            table_entries: 1,
            ..ParseLimits::default()
        };
        assert!(matches!(
            parse_with_limits(&synthetic_file(), limits),
            Err(ParseError::LimitExceeded { .. })
        ));

        let stsz = boxed(
            *b"stsz",
            &full_box_payload(&[
                0, 0, 0, 0, // default size
                0, 0x3d, 0x09, 0, // four million entries, no entry bytes
            ]),
        );
        let stbl = boxed(*b"stbl", &stsz);
        let minf = boxed(*b"minf", &stbl);
        let mdia = boxed(*b"mdia", &minf);
        let trak = boxed(*b"trak", &mdia);
        let moov = boxed(*b"moov", &trak);
        assert!(matches!(parse(&moov), Err(ParseError::Truncated { .. })));
    }

    #[test]
    fn rejects_duplicate_structural_boxes_even_when_the_first_is_empty() {
        let empty_stsd = boxed(*b"stsd", &full_box_payload(&0_u32.to_be_bytes()));
        let mut stbl_payload = empty_stsd.clone();
        stbl_payload.extend_from_slice(&empty_stsd);
        let stbl = boxed(*b"stbl", &stbl_payload);
        let minf = boxed(*b"minf", &stbl);
        let mdia = boxed(*b"mdia", &minf);
        let trak = boxed(*b"trak", &mdia);
        let moov = boxed(*b"moov", &trak);
        assert!(matches!(
            parse(&moov),
            Err(ParseError::InvalidTable {
                box_type: STSD,
                reason: "duplicate stsd table",
                ..
            })
        ));

        let moov = boxed(*b"moov", &[]);
        let mut duplicate_moov = moov.clone();
        duplicate_moov.extend_from_slice(&moov);
        assert!(matches!(
            parse(&duplicate_moov),
            Err(ParseError::InvalidTable {
                box_type: MOOV,
                reason: "duplicate moov box",
                ..
            })
        ));
    }

    #[test]
    fn rejects_sample_end_overflow_and_out_of_file_range() {
        let file = synthetic_file();
        let parsed = parse(&file).expect("synthetic BMFF parses");
        let track = parsed.track(3).expect("track");
        assert!(matches!(
            track.select_sample(0, 513),
            Err(ParseError::SampleOutsideFile { .. })
        ));

        let sizes = SampleSizeTable {
            default_size: u32::MAX,
            sample_count: 1,
            sizes: Vec::new(),
        };
        let error = resolve_sample(
            Some(9),
            &sizes,
            &[SampleToChunk {
                first_chunk: 1,
                samples_per_chunk: 1,
                description_index: 1,
            }],
            &[u64::MAX],
            1,
            0,
            u64::MAX,
        );
        assert!(matches!(error, Err(ParseError::IntegerOverflow { .. })));
    }

    #[test]
    fn linear_sample_iterator_reports_one_error_then_fuses() {
        let parsed = parse(&synthetic_file()).expect("synthetic BMFF parses");
        let track = parsed.track(3).expect("track").clone();
        let mut malformed = track;
        malformed.sample_to_chunk = Some(vec![SampleToChunk {
            first_chunk: 1,
            samples_per_chunk: 1,
            description_index: 1,
        }]);

        let mut locations = malformed.sample_locations(parsed.file_len).expect("tables exist");
        assert!(matches!(
            locations.next(),
            Some(Ok(SampleLocation { sample_index: 0, .. }))
        ));
        assert!(matches!(
            locations.next(),
            Some(Err(ParseError::InconsistentSampleMap { index: 1, .. }))
        ));
        assert!(locations.next().is_none());
    }

    #[test]
    fn enforces_box_and_file_limits() {
        let bytes = boxed(*b"free", &[]);
        let limits = ParseLimits {
            boxes: 0,
            ..ParseLimits::default()
        };
        assert!(matches!(
            parse_with_limits(&bytes, limits),
            Err(ParseError::LimitExceeded {
                kind: "box count",
                ..
            })
        ));

        let limits = ParseLimits {
            file_size: 7,
            ..ParseLimits::default()
        };
        assert!(matches!(
            parse_with_limits(&bytes, limits),
            Err(ParseError::FileTooLarge { .. })
        ));

        let limits = ParseLimits {
            depth: 2,
            ..ParseLimits::default()
        };
        assert!(matches!(
            parse_with_limits(&synthetic_file(), limits),
            Err(ParseError::LimitExceeded {
                kind: "box nesting depth",
                ..
            })
        ));
    }

    #[test]
    fn optionally_validates_a_real_cr3_fixture() {
        let Ok(path) = std::env::var("RRRAH_CR3_FIXTURE") else {
            return;
        };
        let bytes = std::fs::read(&path).expect("read CR3 fixture");
        let parsed = parse(&bytes).expect("parse CR3 fixture");
        let full_raw = parsed.track(3).expect("full-resolution raw track 3");
        assert_eq!(full_raw.sample_descriptions[0].format, CRAW);
        assert!(
            full_raw.sample_descriptions[0]
                .extensions
                .iter()
                .any(|extension| extension.box_type == CMP1)
        );
        let sample = parsed.select_sample(3, 0).expect("select full RAW sample");
        assert!(sample.size > 0);
        assert!(sample.offset + u64::from(sample.size) <= parsed.file_len);
        if path.ends_with("rrrah-eos-r8-9043.cr3") {
            assert_eq!(sample.offset, 2_992_128);
            assert_eq!(sample.size, 19_269_248);
        }
    }
}
