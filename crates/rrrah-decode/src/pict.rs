//! Bounded PICT framing and rectangle renderer. Apple Inside Macintosh, Pictures,
//! pp. 7-5 and 7-7: file data forks have a 512-byte application header;
//! resource/scrap picture records begin directly with picSize and picFrame.
#![allow(dead_code)]
/// Conservative v2 file-data-fork signature; not proof of supported commands.
pub(crate) fn has_magic(bytes: &[u8]) -> bool {
    let Ok(header) = PictureHeader::parse(bytes, true, 100_000_000) else { return false };
    matches!(version_prefix(&bytes[header.commands_offset..]),
        Ok((PictureVersion::Two | PictureVersion::ExtendedTwo, _)))
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PictureHeader {
    pub declared_size: u16,
    pub top: i16,
    pub left: i16,
    pub width: u32,
    pub height: u32,
    pub commands_offset: usize,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HeaderError { Truncated, EmptyOrInverted, PixelLimit, Mapping }
fn canvas_header(bytes:&[u8],data_fork:bool) -> Result<PictureHeader,HeaderError> {
    let mut header=PictureHeader::parse(bytes,data_fork,100_000_000)?;
    let (version,_)=version_prefix(&bytes[header.commands_offset..]).map_err(|_|HeaderError::Mapping)?;
    if version==PictureVersion::ExtendedTwo {
        let info=&bytes[header.commands_offset+6..header.commands_offset+30];
        let fixed=|i|i64::from(i32::from_be_bytes(info[i..i+4].try_into().unwrap()));
        let word=|i|i16::from_be_bytes([info[i],info[i+1]]);
        let hres=fixed(4);let vres=fixed(8);
        let top=word(12);let left=word(14);
        let width=i32::from(word(18))-i32::from(left);
        let height=i32::from(word(16))-i32::from(top);
        if hres<=0 || vres<=0 || width<=0 || height<=0 {return Err(HeaderError::Mapping);}
        let logical=|value:i64,res:i64|value*72*65536/res;
        if logical(i64::from(width),hres)!=i64::from(header.width)
            || logical(i64::from(height),vres)!=i64::from(header.height)
            || logical(i64::from(left),hres)!=i64::from(header.left)
            || logical(i64::from(top),vres)!=i64::from(header.top) {return Err(HeaderError::Mapping);}
        if width as u64*height as u64>100_000_000 {return Err(HeaderError::PixelLimit);}
        header.top=top;header.left=left;header.width=width as u32;header.height=height as u32;
    }
    Ok(header)
}
impl PictureHeader {
    /// Storage representation is explicit; application header bytes are arbitrary.
    /// picSize is retained, not used to truncate version-2 streams.
    pub fn parse(bytes: &[u8], data_fork: bool, max_pixels: u64) -> Result<Self, HeaderError> {
        let offset = if data_fork { 512 } else { 0 };
        let header = bytes.get(offset..offset+10).ok_or(HeaderError::Truncated)?;
        let word = |i| i16::from_be_bytes([header[i],header[i+1]]);
        let top=word(2); let left=word(4);
        let width=i32::from(word(8))-i32::from(left);
        let height=i32::from(word(6))-i32::from(top);
        if width<=0 || height<=0 { return Err(HeaderError::EmptyOrInverted); }
        let width=width as u32; let height=height as u32;
        if u64::from(width)*u64::from(height)>max_pixels { return Err(HeaderError::PixelLimit); }
        Ok(Self { declared_size:u16::from_be_bytes([header[0],header[1]]),top,left,width,height,commands_offset:offset+10 })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn record_and_arbitrary_file_header_preserve_signed_origin_and_size() {
        let record=[0,0,255,254,255,253,0,2,0,5];
        let header=PictureHeader::parse(&record,false,32).unwrap();
        assert_eq!((header.top,header.left,header.width,header.height),(-2,-3,8,4));
        let mut file=vec![0xa5;512]; file.extend_from_slice(&record);
        assert_eq!(PictureHeader::parse(&file,true,32).unwrap().commands_offset,522);
        for n in 0..522 { assert_eq!(PictureHeader::parse(&file[..n],true,32),Err(HeaderError::Truncated)); }
        assert_eq!(PictureHeader::parse(&record,false,31),Err(HeaderError::PixelLimit));
        for n in 0..10 { assert_eq!(PictureHeader::parse(&record[..n],false,32),Err(HeaderError::Truncated)); }
    }
    #[test]
    fn inverted_empty_and_full_signed_range_admission() {
        for r in [[0;10],[0,0,0,2,0,0,0,1,0,1]] { assert_eq!(PictureHeader::parse(&r,false,u64::MAX),Err(HeaderError::EmptyOrInverted)); }
        let r=[0,0,128,0,128,0,127,255,127,255];
        let h=PictureHeader::parse(&r,false,65535u64*65535).unwrap();
        assert_eq!((h.width,h.height),(65535,65535));
        assert_eq!(PictureHeader::parse(&r,false,65535u64*65535-1),Err(HeaderError::PixelLimit));
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PictureVersion { One, Two, ExtendedTwo }
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum VersionError { Truncated, Unsupported, MissingHeader }
/// Parses version framing without allocation; header payload semantics must be
/// validated separately. It does not imply supported drawing or resolution.
pub(crate) fn version_prefix(bytes: &[u8]) -> Result<(PictureVersion, usize), VersionError> {
    if bytes.get(..2).ok_or(VersionError::Truncated)? == [0x11, 1] {
        return Ok((PictureVersion::One, 2));
    }
    if bytes.get(..4).ok_or(VersionError::Truncated)? != [0, 0x11, 2, 0xff] {
        return Err(VersionError::Unsupported);
    }
    if bytes.get(4..6).ok_or(VersionError::Truncated)? != [0x0c, 0] {
        return Err(VersionError::MissingHeader);
    }
    let header=bytes.get(6..30).ok_or(VersionError::Truncated)?;
    let version=match i16::from_be_bytes([header[0],header[1]]) {
        -1 => PictureVersion::Two,
        -2 => PictureVersion::ExtendedTwo,
        _ => return Err(VersionError::Unsupported),
    };
    Ok((version,30))
}
#[cfg(test)]
mod version_tests {
    use super::*;
    #[test]
    fn version_headers_are_bounded_and_word_prefix_is_exact() {
        assert_eq!(version_prefix(&[0x11,1]),Ok((PictureVersion::One,2)));
        let mut bytes=vec![0,0x11,2,0xff,0x0c,0];
        bytes.extend_from_slice(&[0xff;24]);
        assert_eq!(version_prefix(&bytes),Ok((PictureVersion::Two,30)));
        for n in 0..30 { assert_eq!(version_prefix(&bytes[..n]),Err(VersionError::Truncated)); }
        bytes[7]=0xfe;
        assert_eq!(version_prefix(&bytes),Ok((PictureVersion::ExtendedTwo,30)));
        bytes[7]=0xfd;
        assert_eq!(version_prefix(&bytes),Err(VersionError::Unsupported));
        bytes[5]=1;
        assert_eq!(version_prefix(&bytes),Err(VersionError::MissingHeader));
        assert_eq!(version_prefix(&[0,0x11,3,0xff]),Err(VersionError::Unsupported));
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CommandError { Truncated, Unsupported(u16), Limit, Trailing, Cancelled }
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Command<'a> { pub opcode: u16, pub data: &'a [u8] }
struct DirectBits {
    width: usize, height: usize, row_bytes: usize, scratch: usize, unit: usize,
    destination: [i32;4], crop: [usize;4], packed: bool, raw_stride: usize, length: usize,
}
fn direct_bits(bytes: &[u8], cancelled: impl Fn() -> bool) -> Result<DirectBits, CommandError> {
    let header=bytes.get(..68).ok_or(CommandError::Truncated)?;
    let word=|i|u16::from_be_bytes([header[i],header[i+1]]);
    let rect=|offset|std::array::from_fn::<_,4,_>(|i|i32::from(i16::from_be_bytes([header[offset+i*2],header[offset+i*2+1]])));
    let bounds=rect(6);let source=rect(50);let destination=rect(58);
    let width=bounds[3]-bounds[1];let height=bounds[2]-bounds[0];
    let row_bytes=usize::from(word(4)&0x3fff);
    let unit=match (word(16),word(32),word(34),word(36)) {
        (1 | 3,16,3,5)=>2, (1 | 2 | 4,32,3,8)=>1,
        _=>return Err(CommandError::Unsupported(0x9a)),
    };
    if header[..4]!=[0,0,0,255] || word(4)&0x8000==0 || word(14)!=0
        || header[18..22]!=[0;4] || word(30)!=16 || word(66)!=0
        || width<=0 || height<=0 || source[0]<bounds[0] || source[1]<bounds[1]
        || source[2]>bounds[2] || source[3]>bounds[3]
        || source[2]<=source[0] || source[3]<=source[1]
        || destination[3]-destination[1]!=source[3]-source[1]
        || destination[2]-destination[0]!=source[2]-source[0]
        || row_bytes != width as usize * if unit==2 {2} else {4} {
        return Err(CommandError::Unsupported(0x9a));
    }
    let mut offset=68usize;
    let packed=word(16)>2 && row_bytes>=8;
    let raw_stride=if unit==2 {2} else if word(16)==2 && row_bytes>=8 {3} else {4};
    let raw_bytes=width as usize*raw_stride;
    for _ in 0..height {
        if cancelled() {return Err(CommandError::Cancelled);}
        let length=if !packed {raw_bytes} else if row_bytes>250 {
            let p=bytes.get(offset..offset+2).ok_or(CommandError::Truncated)?;
            offset+=2;usize::from(u16::from_be_bytes([p[0],p[1]]))
        } else {let p=*bytes.get(offset).ok_or(CommandError::Truncated)?;offset+=1;usize::from(p)};
        offset=offset.checked_add(length).ok_or(CommandError::Truncated)?;
        if offset>bytes.len() {return Err(CommandError::Truncated);}
    }
    if offset%2!=0 {
        if bytes.get(offset)!=Some(&0) {return Err(CommandError::Truncated);}
        offset+=1;
    }
    Ok(DirectBits {width:width as usize,height:height as usize,row_bytes,
        scratch:if !packed {raw_bytes} else {width as usize*if unit==2 {2} else {3}},unit,destination,packed,raw_stride,
        crop:[(source[0]-bounds[0]) as usize,(source[1]-bounds[1]) as usize,
            (source[2]-bounds[0]) as usize,(source[3]-bounds[1]) as usize],length:offset})
}
/// Decode one length-delimited packed DirectBits row into caller-owned memory.
/// Type 3 uses two-byte pixels; type 4 uses single-byte planar components.
/// Control -128 is a no-op. Runs cannot cross the destination row boundary.
fn unpack_direct_row(
    input: &[u8], output: &mut [u8], unit: usize,
    cancelled: impl Fn() -> bool,
) -> Result<(), CommandError> {
    if !matches!(unit, 1 | 2) || output.len() % unit != 0 {
        return Err(CommandError::Unsupported(0x9a));
    }
    let mut source=0usize;
    let mut target=0usize;
    while source<input.len() {
        if cancelled() { return Err(CommandError::Cancelled); }
        let control=input[source] as i8;source+=1;
        if control==i8::MIN { continue; }
        let count=if control>=0 {control as usize+1} else {usize::from(control.unsigned_abs())+1};
        let bytes=count*unit;
        let destination=output.get_mut(target..target+bytes).ok_or(CommandError::Trailing)?;
        if control>=0 {
            let literal=input.get(source..source+bytes).ok_or(CommandError::Truncated)?;
            destination.copy_from_slice(literal);source+=bytes;
        } else {
            let pixel=input.get(source..source+unit).ok_or(CommandError::Truncated)?;
            for chunk in destination.chunks_exact_mut(unit) {chunk.copy_from_slice(pixel);}
            source+=unit;
        }
        target+=bytes;
    }
    if target!=output.len() { return Err(CommandError::Truncated); }
    if cancelled() { return Err(CommandError::Cancelled); }
    Ok(())
}

#[cfg(test)]
mod packed_row_tests {
    use super::*;
    #[test]
    #[ignore = "requires external PICT files and TwelveMonkeys RGBA oracle outputs"]
    fn external_packed_rows_match_independent_rgba_oracles() {
        let root=std::env::var_os("RRRAH_PICT_CORPUS").expect("RRRAH_PICT_CORPUS required");
        for (name,unit,size) in [("mire16.pict",2,128),("mire32.pict",1,192)] {
            let path=std::path::Path::new(&root);
            let file=std::fs::read(path.join(name)).unwrap();
            let oracle=std::fs::read(path.join(format!("{name}.rgba"))).unwrap();
            assert_eq!(oracle.len(),64*64*4);
            assert_eq!(&file[566..568],&[0,0x9a]);
            let row_bytes=usize::from(u16::from_be_bytes([file[572],file[573]])&0x3fff);
            let budget=rrrah_core::MemoryBudget::new(size);
            let mut row=budget.try_buffer(size as usize,0u8).unwrap();
            let mut offset=568+68;
            for y in 0..64 {
                let packed=if row_bytes>250 {
                    let n=usize::from(u16::from_be_bytes([file[offset],file[offset+1]]));offset+=2;n
                } else {let n=usize::from(file[offset]);offset+=1;n};
                unpack_direct_row(&file[offset..offset+packed],&mut row,unit,||false).unwrap();
                offset+=packed;
                for x in 0..64 {
                    let rgb=if unit==2 {
                        let value=u16::from_be_bytes([row[x*2],row[x*2+1]]);
                        [((value>>10)&31),((value>>5)&31),(value&31)].map(|v|((v<<3)|(v>>2)) as u8)
                    } else {[row[x],row[64+x],row[128+x]]};
                    assert_eq!(&oracle[(y*64+x)*4..(y*64+x)*4+4],&[rgb[0],rgb[1],rgb[2],255],"{name} {x},{y}");
                }
            }
            assert_eq!(budget.peak(),size);
            drop(row);assert_eq!(budget.used(),0);
        }
    }
    #[test]
    fn byte_and_word_runs_preserve_units_and_exact_row_size() {
        let mut bytes=[0;6];
        unpack_direct_row(&[1,10,20,253,30,128],&mut bytes,1,||false).unwrap();
        assert_eq!(bytes,[10,20,30,30,30,30]);
        let mut words=[0;12];
        unpack_direct_row(&[1,0x12,0x34,0xab,0xcd,253,0x56,0x78],&mut words,2,||false).unwrap();
        assert_eq!(words,[0x12,0x34,0xab,0xcd,0x56,0x78,0x56,0x78,0x56,0x78,0x56,0x78]);
    }
    #[test]
    fn malformed_runs_cannot_overwrite_canaries_and_cancel_before_mutation() {
        for unit in [1,2] {
            for input in [&[0u8][..], &[255][..], &[127,1][..], &[129,2][..]] {
                let mut guarded=[9;10];
                assert!(unpack_direct_row(input,&mut guarded[1..9],unit,||false).is_err());
                assert_eq!((guarded[0],guarded[9]),(9,9));
            }
        }
        let mut output=[9;4];
        assert_eq!(unpack_direct_row(&[253,1],&mut output,1,||true),Err(CommandError::Cancelled));
        assert_eq!(output,[9;4]);
        assert!(unpack_direct_row(&[],&mut output,3,||false).is_err());
        assert!(unpack_direct_row(&[],&mut output[..3],2,||false).is_err());
    }
}

/// Borrowed v2 command stream. Unknown operations refuse rather than being
/// silently omitted from a resulting image. A renderer must interpret all
/// admitted state operations, including foreground/background and pen modes.
pub(crate) fn walk_v2(
    bytes: &[u8], max_commands: usize, cancelled: impl Fn() -> bool,
    mut visit: impl FnMut(Command<'_>) -> Result<(), CommandError>,
) -> Result<(), CommandError> {
    let mut offset=0usize;
    let mut count=0usize;
    loop {
        if cancelled() { return Err(CommandError::Cancelled); }
        if count==max_commands { return Err(CommandError::Limit); }
        count+=1;
        let opcode=u16::from_be_bytes(bytes.get(offset..offset+2).ok_or(CommandError::Truncated)?.try_into().unwrap());
        offset+=2;
        let length=match opcode {
            0x00 | 0x1e | 0xff | 0x38..=0x3c => 0,
            0x01 => {
                let size=u16::from_be_bytes(bytes.get(offset..offset+2).ok_or(CommandError::Truncated)?.try_into().unwrap());
                if size!=10 { return Err(CommandError::Unsupported(opcode)); }
                10
            },
            0x07 | 0x0c => 4,
            0x08 => 2,
            0x09 | 0x0a | 0x30..=0x34 => 8,
            0x1a | 0x1b => 6,
            0x9a => direct_bits(&bytes[offset..],&cancelled)?.length,
            _ => return Err(CommandError::Unsupported(opcode)),
        };
        let data=bytes.get(offset..offset+length).ok_or(CommandError::Truncated)?;
        offset+=length;
        if opcode==0xff {
            if offset!=bytes.len() { return Err(CommandError::Trailing); }
            return Ok(());
        }
        visit(Command {opcode,data})?;
    }
}
#[cfg(test)]
mod command_tests {
    use super::*;
    #[test]
    fn borrowed_colors_rectangles_and_end_are_exact_and_bounded() {
        let b=[0,0x1a,0xff,0xff,0,0,0,0,0,0x31,0,0,0,0,0,2,0,3,0,0xff];
        let mut ops=Vec::new();
        walk_v2(&b,3,||false,|c| {ops.push(c.opcode);Ok(())}).unwrap();
        assert_eq!(ops,[0x1a,0x31]);
        assert_eq!(walk_v2(&b,2,||false,|_|Ok(())),Err(CommandError::Limit));
        for n in 0..b.len() { assert!(walk_v2(&b[..n],3,||false,|_|Ok(())).is_err()); }
        let mut trailing=b.to_vec();trailing.push(0);
        assert_eq!(walk_v2(&trailing,3,||false,|_|Ok(())),Err(CommandError::Trailing));
        assert_eq!(walk_v2(&[0,0x90],1,||false,|_|Ok(())),Err(CommandError::Unsupported(0x90)));
    }
    #[test]
    fn cancellation_and_visitor_failure_stop_before_next_command() {
        let calls=std::cell::Cell::new(0);
        let b=[0,0,0,0xff];
        assert_eq!(walk_v2(&b,2,|| {calls.set(calls.get()+1);calls.get()>1},|_|Ok(())),Err(CommandError::Cancelled));
        assert_eq!(walk_v2(&[],0,||true,|_|panic!()),Err(CommandError::Cancelled));
        assert_eq!(walk_v2(&b,2,||false,|_|Err(CommandError::Limit)),Err(CommandError::Limit));
    }
}

fn command_error(error: CommandError) -> crate::RasterDecodeError {
    match error {
        CommandError::Cancelled => crate::DecodeError::Cancelled.into(),
        other => crate::RasterDecodeError::InvalidPict(format!("{other:?}")),
    }
}

/// Strict initial renderer: opaque RGB8-equivalent colors and paintRect only.
/// Wider QuickDraw modes/patterns require their own interpretation, not skipping.
pub(crate) fn render_rectangles(
    bytes: &[u8], data_fork: bool, budget: &rrrah_core::MemoryBudget,
    cancelled: impl Fn() -> bool,
) -> Result<rrrah_core::SharedBuffer<u8>, crate::RasterDecodeError> {
    if cancelled() { return Err(crate::DecodeError::Cancelled.into()); }
    let header=canvas_header(bytes,data_fork).map_err(|e|crate::RasterDecodeError::InvalidPict(format!("{e:?}")))?;
    let (version,prefix)=version_prefix(&bytes[header.commands_offset..]).map_err(|e|crate::RasterDecodeError::InvalidPict(format!("{e:?}")))?;
    if version==PictureVersion::One { return Err(crate::RasterDecodeError::InvalidPict("unsupported picture version".into())); }
    let info=&bytes[header.commands_offset+6..header.commands_offset+30];
    let fixed=|i| i32::from_be_bytes(info[i..i+4].try_into().unwrap());
    let left=i32::from(header.left);let top=i32::from(header.top);
    let valid=match version {
        // The final long is unused, not a required zero-valued signature.
        PictureVersion::Two => info[..4]==[255;4]
            && fixed(4)==left*65536 && fixed(8)==top*65536
            && fixed(12)==(left+header.width as i32)*65536
            && fixed(16)==(top+header.height as i32)*65536,
        PictureVersion::ExtendedTwo => {
            let word=|i|i32::from(i16::from_be_bytes([info[i],info[i+1]]));
            info[..4]==[255,254,0,0]
                && fixed(4)>0 && fixed(8)>0
                && word(12)==top && word(14)==left
                && word(16)==top+header.height as i32
                && word(18)==left+header.width as i32
        }
        PictureVersion::One => false,
    };
    if !valid { return Err(crate::RasterDecodeError::InvalidPict("unsupported picture resolution or bounding-box mapping".into())); }
    let stream=&bytes[header.commands_offset+prefix..];
    let check=|c: Command<'_>| match c.opcode {
        // DefHilite resets highlight color. No currently admitted drawing
        // operation uses highlight color; HiliteMode/invert modes still refuse.
        0 | 0x01 | 0x1e | 0x31 | 0x39 | 0x9a => Ok(()),
        0x1a if c.data.chunks_exact(2).all(|v|v[0]==v[1]) => Ok(()),
        _ => Err(CommandError::Unsupported(c.opcode)),
    };
    let mut has_rect=false;
    let mut scratch_bytes=0usize;
    walk_v2(stream,100_000,&cancelled,|c| {
        check(Command {opcode:c.opcode,data:c.data})?;
        if c.opcode==0x31 {has_rect=true;}
        if c.opcode==0x39 && !has_rect {return Err(CommandError::Unsupported(c.opcode));}
        if c.opcode==0x9a {scratch_bytes=scratch_bytes.max(direct_bits(c.data,&cancelled)?.scratch);}
        Ok(())
    }).map_err(command_error)?;
    if cancelled() {return Err(crate::DecodeError::Cancelled.into());}
    let count=(header.width as usize).checked_mul(header.height as usize).and_then(|v|v.checked_mul(4)).ok_or(crate::DecodeError::DimensionOverflow)?;
    let mut output=budget.try_buffer(count,255u8).map_err(crate::DecodeError::Memory)?;
    let mut scratch=budget.try_buffer(scratch_bytes,0u8).map_err(crate::DecodeError::Memory)?;
    // Preserve untouched canvas as transparent; RGB white is retained beneath
    // zero alpha for deterministic byte identity with the independent oracle.
    for chunk in output.chunks_mut(16 * 1024) {
        if cancelled() { return Err(crate::DecodeError::Cancelled.into()); }
        for pixel in chunk.chunks_exact_mut(4) { pixel[3]=0; }
    }
    let mut color=[0u8;3];
    let mut rect=[0i16;4];
    let mut clip=[0,0,header.height as i32,header.width as i32];
    walk_v2(stream,100_000,&cancelled,|c| {
        match c.opcode {
            0x9a => {
                let image=direct_bits(c.data,&cancelled)?;
                let mut offset=68usize;
                for y in 0..image.height {
                    if cancelled() {return Err(CommandError::Cancelled);}
                    let length=if !image.packed {image.scratch} else if image.row_bytes>250 {
                        let n=usize::from(u16::from_be_bytes([c.data[offset],c.data[offset+1]]));offset+=2;n
                    } else {let n=usize::from(c.data[offset]);offset+=1;n};
                    let row=&mut scratch[..image.scratch];
                    if image.packed {
                        unpack_direct_row(&c.data[offset..offset+length],row,image.unit,&cancelled)?;
                    } else {row.copy_from_slice(&c.data[offset..offset+length]);}
                    offset+=length;
                    if y<image.crop[0] || y>=image.crop[2] {continue;}
                    let dy=image.destination[0]+(y-image.crop[0]) as i32-i32::from(header.top);
                    for x in image.crop[1]..image.crop[3] {
                        let dx=image.destination[1]+(x-image.crop[1]) as i32-i32::from(header.left);
                        if dy<0 || dx<0 || dy>=header.height as i32 || dx>=header.width as i32
                            || dy<clip[0] || dx<clip[1] || dy>=clip[2] || dx>=clip[3] {continue;}
                        let rgb=if image.unit==2 {
                            let value=u16::from_be_bytes([row[x*2],row[x*2+1]]);
                            [(value>>10)&31,(value>>5)&31,value&31].map(|v|((v<<3)|(v>>2)) as u8)
                        } else if image.packed {[row[x],row[image.width+x],row[2*image.width+x]]}
                        else {
                            let start=image.raw_stride*x+usize::from(image.raw_stride==4);
                            [row[start],row[start+1],row[start+2]]
                        };
                        let target=(dy as usize*header.width as usize+dx as usize)*4;
                        output[target..target+4].copy_from_slice(&[rgb[0],rgb[1],rgb[2],255]);
                    }
                }
            },
            0x1a => color=[c.data[0],c.data[2],c.data[4]],
            0x01 => {
                let r: [i32;4]=std::array::from_fn(|i|i32::from(i16::from_be_bytes([c.data[2+i*2],c.data[3+i*2]])));
                clip=[r[0]-i32::from(header.top),r[1]-i32::from(header.left),r[2]-i32::from(header.top),r[3]-i32::from(header.left)];
            },
            0x31 | 0x39 => {
                if c.opcode==0x31 {rect=std::array::from_fn(|i|i16::from_be_bytes([c.data[i*2],c.data[i*2+1]]));}
                let [top,left,bottom,right]=rect.map(i32::from);
                let x0=(left-i32::from(header.left)).clamp(0,header.width as i32);
                let x1=(right-i32::from(header.left)).clamp(0,header.width as i32);
                let y0=(top-i32::from(header.top)).clamp(0,header.height as i32);
                let y1=(bottom-i32::from(header.top)).clamp(0,header.height as i32);
                for y in y0.max(clip[0])..y1.min(clip[2]) {
                    if cancelled() {return Err(CommandError::Cancelled);}
                    for x in x0.max(clip[1])..x1.min(clip[3]) {
                        let index=(y as usize*header.width as usize+x as usize)*4;
                        output[index..index+3].copy_from_slice(&color);
                        output[index+3]=255;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }).map_err(command_error)?;
    if cancelled() { return Err(crate::DecodeError::Cancelled.into()); }
    Ok(output.freeze())
}
#[cfg(test)]
mod render_tests {
    use super::*;
    fn picture() -> Vec<u8> {
        let mut b=vec![0,0,0,0,0,0,0,2,0,3,0,0x11,2,0xff,0x0c,0];
        b.extend_from_slice(&[0xff;4]);
        for value in [0i32,0,3*65536,2*65536,0] { b.extend_from_slice(&value.to_be_bytes()); }
        b.extend_from_slice(&[0,0x1a,0xff,0xff,0,0,0,0,0,0x31,0,0,0,1,0,1,0,3,0,0xff]);b
    }
    #[test]
    fn direct_bits_unpacked_and_short_rows_preserve_rgb_and_ignore_pad_byte() {
        for (depth,pack,width) in [(16u16,3u16,1u16),(32,4,1),(16,1,4),(32,1,2),(32,2,1),(32,2,2)] {
            let mut b=picture()[..40].to_vec();
            b[6..8].copy_from_slice(&1u16.to_be_bytes());b[8..10].copy_from_slice(&width.to_be_bytes());
            b[28..32].copy_from_slice(&(i32::from(width)*65536).to_be_bytes());
            b[32..36].copy_from_slice(&65536i32.to_be_bytes());
            b.extend_from_slice(&[0,0x9a]);let mut header=[0u8;68];header[3]=255;
            let pitch=width*(depth/8);
            for (offset,value) in [(4,0x8000|pitch),(10,1),(12,width),(16,pack),(30,16),
                (32,depth),(34,3),(36,if depth==16 {5} else {8}),(54,1),(56,width),(62,1),(64,width)] {
                header[offset..offset+2].copy_from_slice(&value.to_be_bytes());
            }
            b.extend_from_slice(&header);
            for _ in 0..width {
                if depth==16 {b.extend_from_slice(&[0x7c,0]);}
                else if pack==2 && pitch>=8 {b.extend_from_slice(&[255,0,0]);}
                else {b.extend_from_slice(&[91,255,0,0]);}
            }
            b.extend_from_slice(&[0,255]);
            let budget=rrrah_core::MemoryBudget::new(u64::from(width)*4+u64::from(pitch));
            let output=render_rectangles(&b,false,&budget,||false).unwrap();
            assert!(output.chunks_exact(4).all(|pixel|pixel==[255,0,0,255]));
            drop(output);assert_eq!(budget.used(),0);
            assert!(render_rectangles(&b[..b.len()-3],false,&budget,||false).is_err());
            assert_eq!(budget.used(),0);
        }
    }
    #[test]
    fn direct_bits_crop_excludes_padded_columns_and_preserves_row_pitch() {
        let mut b=picture()[..40].to_vec();
        b.extend_from_slice(&[0,0x9a]);
        let mut header=[0u8;68];
        header[3]=255;
        for (offset,value) in [(4,0x8008u16),(10,2),(12,4),(16,3),(30,16),(32,16),(34,3),(36,5),
            (52,1),(54,2),(56,4),(62,2),(64,3)] {
            header[offset..offset+2].copy_from_slice(&value.to_be_bytes());
        }
        b.extend_from_slice(&header);
        for pixels in [[0x7fffu16,0x7c00,0x03e0,0x001f],[0x7fff,0,0x7fff,0x7c00]] {
            b.extend_from_slice(&[9,3]);
            for pixel in pixels {b.extend_from_slice(&pixel.to_be_bytes());}
        }
        b.extend_from_slice(&[0,255]);
        let budget=rrrah_core::MemoryBudget::new(32);
        let output=render_rectangles(&b,false,&budget,||false).unwrap();
        assert_eq!(&*output,&[255,0,0,255,0,255,0,255,0,0,255,255,
            0,0,0,255,255,255,255,255,255,0,0,255]);
        assert_eq!(budget.peak(),32);assert_eq!(budget.used(),24);
        drop(output);assert_eq!(budget.used(),0);
        // Source right is outside the stored PixMap bounds.
        b[42+56..42+58].copy_from_slice(&5u16.to_be_bytes());
        let refusal=rrrah_core::MemoryBudget::new(32);
        assert!(render_rectangles(&b,false,&refusal,||false).is_err());
        assert_eq!(refusal.peak(),0);
    }
    #[test]
    fn default_highlight_reset_preserves_paint_but_highlight_mode_refuses() {
        let b=picture();
        let budget=rrrah_core::MemoryBudget::new(48);
        let reference=render_rectangles(&b,false,&budget,||false).unwrap();
        let mut reset=b.clone();reset.splice(40..40,[0,0x1e]);
        let output=render_rectangles(&reset,false,&budget,||false).unwrap();
        assert_eq!(&*reference,&*output);
        drop(reference);drop(output);assert_eq!(budget.used(),0);
        reset[41]=0x1c;
        let refusal=rrrah_core::MemoryBudget::new(24);
        assert!(render_rectangles(&reset,false,&refusal,||false).is_err());
        assert_eq!(refusal.peak(),0);
    }
    #[test]
    fn exact_fill_budget_ownership_and_unsupported_preflight() {
        let b=picture();let budget=rrrah_core::MemoryBudget::new(24);
        let out=render_rectangles(&b,false,&budget,||false).unwrap();
        assert_eq!(&*out,&[255,255,255,0,255,0,0,255,255,0,0,255,255,255,255,0,255,255,255,0,255,255,255,0]);
        let retained=out.clone();drop(out);assert_eq!(budget.used(),24);drop(retained);assert_eq!(budget.used(),0);
        assert!(render_rectangles(&b,false,&rrrah_core::MemoryBudget::new(23),||false).is_err());
        let mut bad_header=b.clone();bad_header[35]=1; // mismatched fixed-point bottom
        let header_budget=rrrah_core::MemoryBudget::new(24);
        assert!(render_rectangles(&bad_header,false,&header_budget,||false).is_err());
        assert_eq!(header_budget.peak(),0);
        let mut unsupported=b;unsupported[49]=0x30;
        let rejected=rrrah_core::MemoryBudget::new(24);
        assert!(render_rectangles(&unsupported,false,&rejected,||false).is_err());assert_eq!(rejected.peak(),0);
        assert!(render_rectangles(&[],false,&rejected,||true).is_err());assert_eq!(rejected.peak(),0);
    }
    #[test]
    fn rectangular_clip_changes_apply_to_later_paints() {
        let base=picture();
        let mut b=base[..40].to_vec();
        b.extend_from_slice(&[0,1,0,10,0,0,0,2,0,2,0,3]);
        b.extend_from_slice(&base[40..]);
        let budget=rrrah_core::MemoryBudget::new(24);
        let out=render_rectangles(&b,false,&budget,||false).unwrap();
        assert_eq!(&out[4..8],&[255,255,255,0]);
        assert_eq!(&out[8..12],&[255,0,0,255]);
        drop(out);
        b.truncate(b.len()-2);
        b.extend_from_slice(&[0,1,0,10,0,0,0,1,0,1,0,2,0,0x39,0,0xff]);
        let out=render_rectangles(&b,false,&budget,||false).unwrap();
        assert_eq!(&out[4..8],&[255,0,0,255]);
        drop(out);assert_eq!(budget.used(),0);
        let mut bad=b;bad[43]=12;
        let fresh=rrrah_core::MemoryBudget::new(24);
        assert!(render_rectangles(&bad,false,&fresh,||false).is_err());
        assert_eq!(fresh.peak(),0);
    }

    #[test]
    fn extended_72dpi_matches_v2_and_other_resolution_refuses_before_allocation() {
        let mut b=picture();
        let mut v1=b[..10].to_vec();v1.extend_from_slice(&[0x11,1]);
        let refused=rrrah_core::MemoryBudget::new(24);
        assert!(render_rectangles(&v1,false,&refused,||false).is_err());
        assert_eq!(refused.peak(),0);
        let budget=rrrah_core::MemoryBudget::new(48);
        let original=render_rectangles(&b,false,&budget,||false).unwrap();
        let mut info=vec![255,254,0,0];
        info.extend_from_slice(&(72i32*65536).to_be_bytes());
        info.extend_from_slice(&(72i32*65536).to_be_bytes());
        for value in [0i16,0,2,3] {info.extend_from_slice(&value.to_be_bytes());}
        info.extend_from_slice(&[0;4]);b[16..40].copy_from_slice(&info);
        let extended=render_rectangles(&b,false,&budget,||false).unwrap();
        assert_eq!(&*original,&*extended);
        drop(extended);
        b[36..40].copy_from_slice(&[0,0,127,255]);
        let unused_tail=render_rectangles(&b,false,&budget,||false).unwrap();
        assert_eq!(&*original,&*unused_tail);
        drop(unused_tail);
        b[36..40].copy_from_slice(&[255;4]);
        let unused_tail=render_rectangles(&b,false,&budget,||false).unwrap();
        assert_eq!(&*original,&*unused_tail);
        drop(original);drop(unused_tail);assert_eq!(budget.used(),0);
        b[20..24].copy_from_slice(&(144i32*65536).to_be_bytes());
        let rejected=rrrah_core::MemoryBudget::new(24);
        assert!(render_rectangles(&b,false,&rejected,||false).is_err());
        assert_eq!(rejected.peak(),0);
    }

    #[test]
    fn cancellation_during_allocated_fill_releases_output_and_allows_retry() {
        let mut b=picture();
        b[6..8].copy_from_slice(&1000i16.to_be_bytes());
        b[32..36].copy_from_slice(&(1000i32*65536).to_be_bytes());
        b[54..56].copy_from_slice(&1000i16.to_be_bytes());
        let budget=rrrah_core::MemoryBudget::new(12001);
        let competing=budget.try_buffer(1,0u8).unwrap().freeze();
        let checks=std::cell::Cell::new(0usize);
        let cancelled=|| {
            if budget.used()>1 {checks.set(checks.get()+1);}
            checks.get()>=10
        };
        assert!(render_rectangles(&b,false,&budget,cancelled).is_err());
        assert_eq!(checks.get(),10);
        assert_eq!(budget.used(),1);
        assert_eq!(budget.peak(),12001);
        let output=render_rectangles(&b,false,&budget,||false).unwrap();
        assert_eq!(output.len(),12000);
        for row in output.chunks_exact(12) {
            assert_eq!(&row[..4],&[255,255,255,0]);
            assert_eq!(&row[4..],&[255,0,0,255,255,0,0,255]);
        }
        drop(output);assert_eq!(budget.used(),1);drop(competing);assert_eq!(budget.used(),0);
    }

    #[test]
    fn authored_file_fixture_renders_full_rgba_and_releases_budget() {
        let bytes=include_bytes!("../../../tests/fixtures/pict/red-rectangle-v2.pict");
        let budget=rrrah_core::MemoryBudget::new(24);
        let output=render_rectangles(bytes,true,&budget,||false).unwrap();
        assert_eq!(&*output,&[255,255,255,0,255,0,0,255,255,0,0,255,255,255,255,0,255,255,255,0,255,255,255,0]);
        drop(output);assert_eq!(budget.used(),0);
    }

}

/// Public adapter for file-data-fork PICT candidates. Untagged color remains
/// unspecified; no silent conversion to sRGB or fallback to a partial image.
pub(crate) fn decode(bytes: &[u8], request: &crate::DecodeRequest) -> Result<rrrah_core::DecodedRaster, crate::RasterDecodeError> {
    request.check_cancelled()?;
    if request.image_index!=0 {return Err(crate::DecodeError::UnsupportedImageIndex {index:request.image_index}.into());}
    let header=canvas_header(bytes,true).map_err(|e|crate::RasterDecodeError::InvalidPict(format!("{e:?}")))?;
    let fallback=rrrah_core::MemoryBudget::new(512*1024*1024);
    let budget=request.memory_budget.as_ref().unwrap_or(&fallback);
    let rendered=render_rectangles(bytes,true,budget,||request.check_cancelled().is_err());
    request.check_cancelled()?;
    let pixels=rendered?;
    Ok(rrrah_core::DecodedRaster::new(header.width,header.height,
        rrrah_core::RasterPixels::Rgba8(pixels.into()),
        if request.assume_untagged_srgb { rrrah_core::RasterColorSpace::AssumedSrgb }
        else { rrrah_core::RasterColorSpace::Unspecified })?)
}
#[cfg(test)]
mod public_tests {
    #[test]
    fn authored_unpacked_public_fixtures_match_authored_pixels() {
        let manifest:serde_json::Value=serde_json::from_str(include_str!("../../../tests/fixtures/pict/unpacked-manifest.json")).unwrap();
        for case in manifest["cases"].as_array().unwrap() {
            let expected:Vec<u8>=case["expected_rgba8"].as_array().unwrap().iter()
                .map(|value|value.as_u64().unwrap() as u8).collect();
            let budget=rrrah_core::MemoryBudget::new(4096);
            let mut request=crate::DecodeRequest::new(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/pict").join(case["file"].as_str().unwrap()));
            request.memory_budget=Some(budget.clone());
            let crate::DecodedImage::Raster(image)=crate::decode_image(&request).unwrap() else {panic!()};
            assert_eq!(image.width(),case["width"].as_u64().unwrap() as u32);
            assert_eq!(image.height(),case["height"].as_u64().unwrap() as u32);
            let rrrah_core::RasterPixels::Rgba8(p)=image.pixels() else {panic!()};
            assert_eq!(&**p,&expected);
            if case["sips_matches"]==true {
                assert_eq!(case["sips_matches"],true);
                let oracle:Vec<u8>=case["sips_rgba8"].as_array().unwrap().iter()
                    .map(|value|value.as_u64().unwrap() as u8).collect();
                assert_eq!(&**p,&oracle);
            }
            drop(image);assert_eq!(budget.used(),0);
            if case["file"]=="drop-pad-cropped-odd.pict" {
                let mut bytes=std::fs::read(&request.path).unwrap();
                let pad=bytes.len()-3;
                assert_eq!(bytes[pad],0);
                bytes[pad]=1;
                let refusal=rrrah_core::MemoryBudget::new(4096);
                assert!(super::render_rectangles(&bytes,true,&refusal,||false).is_err());
                assert_eq!(refusal.peak(),0);
                assert_eq!(refusal.used(),0);
            }
        }
    }
    #[test]
    #[ignore = "requires external 96dpi PICT/TwelveMonkeys corpus"]
    fn external_96dpi_cropped_public_pixels_match_oracle() {
        let root=std::env::var_os("RRRAH_PICT_CORPUS").expect("RRRAH_PICT_CORPUS required");
        for name in ["16bit.pict","32bit.pict"] {
            let root=std::path::Path::new(&root);
            let oracle=std::fs::read(root.join(format!("{name}.rgba"))).unwrap();
            let budget=rrrah_core::MemoryBudget::new(4*1024*1024);
            let mut request=crate::DecodeRequest::new(root.join(name));request.memory_budget=Some(budget.clone());
            let crate::DecodedImage::Raster(image)=crate::decode_image(&request).unwrap() else {panic!()};
            assert_eq!((image.width(),image.height()),(269,269));
            let rrrah_core::RasterPixels::Rgba8(p)=image.pixels() else {panic!()};
            assert_eq!(&**p,&oracle);
            assert_eq!(budget.used(),269*269*4);
            drop(image);assert_eq!(budget.used(),0);
        }
    }
    #[test]
    #[ignore = "requires downloaded TwelveMonkeys PICT corpus"]
    fn external_mire_public_render_matches_independent_oracle_and_releases_scratch() {
        let root = std::env::var_os("RRRAH_PICT_CORPUS").expect("RRRAH_PICT_CORPUS required");
        for name in ["mire16.pict", "mire32.pict"] {
            let bytes = std::fs::read(std::path::Path::new(&root).join(name)).unwrap();
            let oracle=std::fs::read(std::path::Path::new(&root).join(format!("{name}.rgba"))).unwrap();
            let budget = rrrah_core::MemoryBudget::new(65536);
            let mut request=crate::DecodeRequest::new(std::path::Path::new(&root).join(name));
            request.memory_budget=Some(budget.clone());
            let crate::DecodedImage::Raster(image)=crate::decode_image(&request).unwrap() else {panic!()};
            let rrrah_core::RasterPixels::Rgba8(pixels)=image.pixels() else {panic!()};
            assert_eq!(&**pixels,&oracle);
            assert_eq!(budget.used(),16384);
            assert_eq!(budget.peak(),bytes.len() as u64+16384+if name=="mire16.pict" {128} else {192});
            drop(image);assert_eq!(budget.used(),0);
            let scratch=if name=="mire16.pict" {128} else {192};
            let exact=rrrah_core::MemoryBudget::new(bytes.len() as u64+16384+scratch);
            request.memory_budget=Some(exact.clone());
            let competitor=exact.try_buffer(1,0u8).unwrap().freeze();
            assert!(matches!(crate::decode_image(&request),Err(crate::RasterDecodeError::Source(crate::DecodeError::Memory(_)))));
            assert_eq!(exact.used(),1);
            drop(competitor);
            let crate::DecodedImage::Raster(retry)=crate::decode_image(&request).unwrap() else {panic!()};
            let rrrah_core::RasterPixels::Rgba8(retry_pixels)=retry.pixels() else {panic!()};
            assert_eq!(&**retry_pixels,&oracle);
            drop(retry);assert_eq!(exact.used(),0);
            let cancel_root=rrrah_core::MemoryBudget::new(16384+scratch);
            let allocated_checks=std::cell::Cell::new(0);
            let cancelled=|| {
                if cancel_root.used()==cancel_root.limit() {
                    allocated_checks.set(allocated_checks.get()+1);
                }
                allocated_checks.get()>4
            };
            assert!(matches!(super::render_rectangles(&bytes,true,&cancel_root,cancelled),
                Err(crate::RasterDecodeError::Source(crate::DecodeError::Cancelled))));
            assert_eq!(cancel_root.used(),0);
            let retry=super::render_rectangles(&bytes,true,&cancel_root,||false).unwrap();
            assert_eq!(&*retry,&oracle);
            drop(retry);assert_eq!(cancel_root.used(),0);
            let mut unsupported=bytes.clone();unsupported[635]=1;
            let refusal=rrrah_core::MemoryBudget::new(0);
            assert!(super::render_rectangles(&unsupported,true,&refusal,||false).is_err());
            assert_eq!(refusal.peak(),0);
            for length in 568..bytes.len() {
                assert!(super::render_rectangles(&bytes[..length],true,&budget,||false).is_err());
                assert_eq!(budget.used(),0);
            }
        }
    }
    #[test]
    fn content_signature_routes_misleading_raw_and_unknown_suffixes() {
        let bytes = include_bytes!("../../../tests/fixtures/pict/red-rectangle-v2.pict");
        assert!(super::has_magic(bytes));
        for length in 0..552 { assert!(!super::has_magic(&bytes[..length])); }
        assert!(!super::has_magic(&[0; 552]));
        let directory = std::env::temp_dir().join(format!("rrrah-pict-sniff-{}-{}",
            std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&directory).unwrap();
        for extension in ["cr3", "unknown", "png"] {
            let path = directory.join(format!("renamed.{extension}"));
            std::fs::write(&path, bytes).unwrap();
            let budget = rrrah_core::MemoryBudget::new(4096);
            let mut request = crate::DecodeRequest::new(path);
            request.memory_budget = Some(budget.clone());
            assert_eq!(crate::image_source_kind(&request).unwrap(), crate::ImageSourceKind::Raster);
            let crate::DecodedImage::Raster(image) = crate::decode_image(&request).unwrap() else { panic!() };
            assert_eq!((image.width(), image.height()), (3, 2));
            assert_eq!(image.color_space(), &rrrah_core::RasterColorSpace::Unspecified);
            drop(image);
            assert_eq!(budget.used(), 0);
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn public_file_route_preserves_rgba_and_unspecified_color() {
        let path=std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pict/red-rectangle-v2.pict");
        let budget=rrrah_core::MemoryBudget::new(2048);
        let mut request=crate::DecodeRequest::new(&path);request.memory_budget=Some(budget.clone());
        assert!(crate::is_supported_image_path(&path));
        let image=crate::decode_raster(&request).unwrap();
        assert_eq!(image.color_space(),&rrrah_core::RasterColorSpace::Unspecified);
        let rrrah_core::RasterPixels::Rgba8(pixels)=image.pixels() else {panic!()};
        assert_eq!(&pixels[4..8],&[255,0,0,255]);
        assert_eq!(&pixels[..4],&[255,255,255,0]);
        assert_eq!(budget.used(),24);
        drop(image);assert_eq!(budget.used(),0);
    }
    #[test]
    fn public_output_pressure_is_typed_and_retry_keeps_budget_clean() {
        let path=std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pict/red-rectangle-v2.pict");
        let source=std::fs::metadata(&path).unwrap().len();
        let budget=rrrah_core::MemoryBudget::new(source+24);
        let held=budget.try_buffer(1,0u8).unwrap().freeze();
        let mut request=crate::DecodeRequest::new(&path);request.memory_budget=Some(budget.clone());
        assert!(matches!(crate::decode_raster(&request),Err(crate::RasterDecodeError::Source(crate::DecodeError::Memory(_)))));
        assert_eq!(budget.used(),1);drop(held);
        let output=crate::decode_raster(&request).unwrap();assert_eq!(budget.used(),24);
        drop(output);assert_eq!(budget.used(),0);
    }

    #[test]
    fn common_image_route_requires_explicit_color_assumption_for_display() {
        let path=std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pict/red-rectangle-v2.pict");
        let budget=rrrah_core::MemoryBudget::new(2048);
        let mut request=crate::DecodeRequest::new(&path);request.memory_budget=Some(budget.clone());
        let crate::DecodedImage::Raster(unknown)=crate::decode_image(&request).unwrap() else {panic!()};
        assert!(crate::prepare_raster_for_display_with_budget(&unknown,Some(&budget)).is_err());
        assert_eq!(budget.used(),24);drop(unknown);assert_eq!(budget.used(),0);
        request.assume_untagged_srgb=true;
        let crate::DecodedImage::Raster(assumed)=crate::decode_image(&request).unwrap() else {panic!()};
        assert_eq!(assumed.color_space(),&rrrah_core::RasterColorSpace::AssumedSrgb);
        let ready=crate::prepare_raster_for_display_with_budget(&assumed,Some(&budget)).unwrap();
        let rrrah_core::RasterPixels::Rgba32Float(pixels)=ready.pixels() else {panic!()};
        assert_eq!(&pixels[4..8],&[1.,0.,0.,1.]);
        assert_eq!(pixels[3],0.);
        assert_eq!(budget.used(),120);
        drop(ready);drop(assumed);assert_eq!(budget.used(),0);
    }

}
