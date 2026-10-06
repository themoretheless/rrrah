//! Structural XCF parsing and native pixel-level primitives; full flattening is pending.
//! Header parsing borrows input and allocates no buffers.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XcfHeader {
    pub version: u16,
    pub width: u32,
    pub height: u32,
    /// 0 RGB, 1 grayscale, 2 indexed; preserved without interpreting pixels.
    pub base_type: u32,
    pub precision: u32,
    pub sample_bytes: u8,
    pub offset_bytes: u8,
    pub properties_offset: usize,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("invalid or unsupported XCF header: {0}")]
pub struct XcfHeaderError(pub &'static str);

#[derive(Debug, thiserror::Error)]
pub enum XcfPixelError {
    #[error(transparent)]
    Structure(#[from] XcfHeaderError),
    #[error(transparent)]
    Memory(#[from] rrrah_core::BufferError),
    #[error("unsupported XCF layer index {requested}; file has {count} layers")]
    UnsupportedLayerIndex { requested: usize, count: usize },
    #[error("XCF pixel decode cancelled")]
    Cancelled,
}

fn xcf_compression(bytes: &[u8], cancelled: &mut dyn FnMut() -> bool) -> Result<u8, XcfPixelError> {
    let mut compression = None;
    for property in xcf_properties(bytes)? {
        if cancelled() {
            return Err(XcfPixelError::Cancelled);
        }
        let property = property?;
        if property.kind == 17 {
            if compression.is_some() || property.payload.len() != 1 || property.payload[0] > 2 {
                return Err(XcfHeaderError("compression property").into());
            }
            compression = Some(property.payload[0]);
        }
    }
    Ok(compression.unwrap_or(1))
}

/// Selected native layer samples and optional native mask. Metadata borrows the
/// source file; pixels retain their managed reservation until the last owner drops.
/// No flattening, color conversion, effects or blend evaluation is performed.
#[derive(Debug)]
pub struct XcfDecodedLayer<'a> {
    pub header: XcfHeader,
    pub layer: XcfLayer<'a>,
    pub pixels: rrrah_core::SharedBuffer<u8>,
    pub mask: Option<rrrah_core::SharedBuffer<u8>>,
}

/// Decode one layer and its stored mask with a shared output/scratch budget.
/// Indexed samples remain indices; callers must explicitly interpret the palette
/// and version-specific alpha. Source bytes are borrowed and outside this budget.
pub fn decode_xcf_layer<'a>(
    bytes: &'a [u8],
    index: usize,
    budget: &rrrah_core::MemoryBudget,
    mut cancelled: impl FnMut() -> bool,
) -> Result<XcfDecodedLayer<'a>, XcfPixelError> {
    if cancelled() {
        return Err(XcfPixelError::Cancelled);
    }
    let header = parse_xcf_header(bytes)?;
    let tables = parse_xcf_object_tables(bytes)?;
    let count = tables.layers.iter().count();
    let offset = tables
        .layers
        .iter()
        .nth(index)
        .ok_or(XcfPixelError::UnsupportedLayerIndex {
            requested: index,
            count,
        })?;
    let compression = xcf_compression(bytes, &mut cancelled)?;
    let mut layer = parse_xcf_layer(bytes, offset)?;
    for property in layer.properties.by_ref() {
        property?;
        if cancelled() {
            return Err(XcfPixelError::Cancelled);
        }
    }
    layer.attributes()?;
    layer.composition_attributes()?;
    layer.group()?;
    let sample_bytes = if layer.kind >= 4 { 1 } else { header.sample_bytes };
    let bpp = [3, 4, 1, 2, 1, 2][layer.kind as usize] * sample_bytes;
    let level = parse_xcf_hierarchy(bytes, layer.hierarchy_offset, layer.width, layer.height, bpp)?;
    let pixels = decode_xcf_level(bytes, &level, compression, budget, &mut cancelled)?;
    let mask = if layer.mask_offset == 0 {
        None
    } else {
        let mut channel = parse_xcf_channel(bytes, layer.mask_offset, layer.width, layer.height)?;
        for property in channel.properties.by_ref() {
            property?;
            if cancelled() {
                return Err(XcfPixelError::Cancelled);
            }
        }
        let level = parse_xcf_hierarchy(
            bytes,
            channel.hierarchy_offset,
            channel.width,
            channel.height,
            header.sample_bytes,
        )?;
        Some(decode_xcf_level(
            bytes,
            &level,
            compression,
            budget,
            &mut cancelled,
        )?)
    };
    if cancelled() {
        return Err(XcfPixelError::Cancelled);
    }
    Ok(XcfDecodedLayer {
        header,
        layer,
        pixels,
        mask,
    })
}

pub(crate) fn has_magic(bytes: &[u8]) -> bool {
    bytes.starts_with(b"gimp xcf ")
}

pub(crate) fn decode_raster(
    bytes: &[u8],
    request: &crate::DecodeRequest,
) -> Result<rrrah_core::DecodedRaster, crate::raster::RasterDecodeError> {
    use rrrah_core::{DecodedRaster, RasterColorSpace, RasterPixels};
    request.check_cancelled()?;
    if request.image_index != 0 {
        return Err(crate::DecodeError::UnsupportedImageIndex {
            index: request.image_index,
        }
        .into());
    }
    let fallback = rrrah_core::MemoryBudget::new(crate::raster::MAX_RASTER_BYTES);
    let budget = request.memory_budget.as_ref().unwrap_or(&fallback);
    let image = flatten_xcf_legacy_normal(bytes, budget, || {
        request
            .cancellation
            .as_ref()
            .is_some_and(crate::GenerationToken::is_cancelled)
    })
    .map_err(|error| match error {
        XcfPixelError::Memory(error) => {
            crate::raster::RasterDecodeError::Source(crate::DecodeError::Memory(error))
        }
        XcfPixelError::Cancelled => crate::raster::RasterDecodeError::Source(crate::DecodeError::Cancelled),
        other => crate::raster::RasterDecodeError::InvalidXcf(other.to_string()),
    })?;
    let reservation = image
        .icc_profile
        .map(|profile| budget.try_reserve(profile.len() as u64))
        .transpose()
        .map_err(crate::DecodeError::from)?;
    let color = if let Some(profile) = image.icc_profile {
        RasterColorSpace::Icc(profile.to_vec())
    } else if request.assume_untagged_srgb {
        RasterColorSpace::AssumedSrgb
    } else {
        RasterColorSpace::Unspecified
    };
    let raster = DecodedRaster::new(
        image.width,
        image.height,
        RasterPixels::Rgba8(image.pixels.into()),
        color,
    )?;
    let raster = match reservation {
        Some(reservation) => raster
            .with_color_profile_reservation(reservation)
            .map_err(crate::DecodeError::from)?,
        None => raster,
    };
    request.check_cancelled()?;
    Ok(raster)
}

/// Encoded-space straight RGBA result. The borrowed ICC profile is preserved,
/// not applied; callers must perform color preparation before display.
#[derive(Debug)]
pub struct XcfFlattenedImage<'a> {
    pub width: u32,
    pub height: u32,
    pub pixels: rrrah_core::SharedBuffer<u8>,
    pub icc_profile: Option<&'a [u8]>,
}

/// Flatten legacy 8-bit files using only normal source-over in encoded space.
/// Groups, effects, modern composition choices and other blend modes are refused
/// before allocating the canvas. This is not a complete XCF renderer.
pub fn flatten_xcf_legacy_normal<'a>(
    bytes: &'a [u8],
    budget: &rrrah_core::MemoryBudget,
    mut cancelled: impl FnMut() -> bool,
) -> Result<XcfFlattenedImage<'a>, XcfPixelError> {
    if cancelled() {
        return Err(XcfPixelError::Cancelled);
    }
    let header = parse_xcf_header(bytes)?;
    if header.version > 3 || header.sample_bytes != 1 || (header.version == 0 && header.base_type == 2) {
        return Err(XcfHeaderError("legacy flatten precision/version").into());
    }
    xcf_compression(bytes, &mut cancelled)?;
    let tables = parse_xcf_object_tables(bytes)?;
    let icc_profile = xcf_icc_profile(bytes)?;
    let palette = xcf_palette(bytes)?.unwrap_or(&[]);
    for offset in tables.layers.iter() {
        if cancelled() {
            return Err(XcfPixelError::Cancelled);
        }
        let layer = parse_xcf_layer(bytes, offset)?;
        let group = layer.group()?;
        let composition = layer.composition_attributes()?;
        layer.attributes()?;
        if group.is_group
            || group.item_path().next().is_some()
            || layer.effects.iter().next().is_some()
            || composition.blend_mode != 0
            || !matches!(composition.composite_mode, None | Some(-1 | 0 | 1))
            || !matches!(composition.composite_space, None | Some(-2 | 0 | 2))
            || !matches!(composition.blend_space, None | Some(0))
            || layer.kind / 2 != header.base_type
        {
            return Err(XcfHeaderError("unsupported legacy layer composition").into());
        }
    }
    let length = usize::try_from(u64::from(header.width) * u64::from(header.height) * 4)
        .map_err(|_| XcfHeaderError("flatten size"))?;
    if length as u64 > crate::raster::MAX_RASTER_BYTES / 4 {
        return Err(XcfHeaderError("flatten canvas cap").into());
    }
    let mut canvas = budget.try_buffer(length, 0f32)?;
    let count = tables.layers.iter().count();
    // XCF stores layers topmost first; composition visits them bottommost first.
    for index in (0..count).rev() {
        if cancelled() {
            return Err(XcfPixelError::Cancelled);
        }
        let offset = tables.layers.iter().nth(index).unwrap();
        let layer = parse_xcf_layer(bytes, offset)?;
        let attributes = layer.attributes()?;
        if !attributes.visible || attributes.opacity == 0.0 {
            continue;
        }
        let decoded = decode_xcf_layer(bytes, index, budget, &mut cancelled)?;
        let attributes = decoded.layer.attributes()?;
        let rgba = prepare_xcf_legacy_rgba8(
            decoded.pixels,
            decoded.layer.kind,
            palette,
            budget,
            &mut cancelled,
        )?;
        composite_xcf_normal_rgba8(
            &mut canvas,
            header.width,
            header.height,
            &rgba,
            decoded.layer.width,
            decoded.layer.height,
            decoded.mask.as_deref(),
            attributes,
            &mut cancelled,
        )?;
    }
    let mut output = budget.try_buffer(length, 0u8)?;
    for (index, (source, destination)) in canvas.chunks_exact(4).zip(output.chunks_exact_mut(4)).enumerate() {
        if index % 4096 == 0 && cancelled() {
            return Err(XcfPixelError::Cancelled);
        }
        let alpha = source[3];
        for channel in 0..3 {
            destination[channel] = if alpha == 0.0 {
                0
            } else {
                (source[channel] / alpha * 255.0).clamp(0.0, 255.0).round() as u8
            };
        }
        destination[3] = (alpha * 255.0).clamp(0.0, 255.0).round() as u8;
    }
    if cancelled() {
        return Err(XcfPixelError::Cancelled);
    }
    Ok(XcfFlattenedImage {
        width: header.width,
        height: header.height,
        pixels: output.freeze(),
        icc_profile,
    })
}

/// Borrow the image colormap without allocating or changing its encoded RGB values.
/// Missing and empty palettes remain distinct; indexed expansion rejects empty palettes.
pub fn xcf_palette(bytes: &[u8]) -> Result<Option<&[[u8; 3]]>, XcfHeaderError> {
    let mut palette = None;
    for property in xcf_properties(bytes)? {
        let property = property?;
        if property.kind == 1 {
            if palette.is_some() {
                return Err(XcfHeaderError("duplicate palette"));
            }
            // XcfProperties validated count, actual payload and historic length.
            let (colors, remainder) = property.payload[4..].as_chunks::<3>();
            if !remainder.is_empty() {
                return Err(XcfHeaderError("palette length"));
            }
            palette = Some(colors);
        }
    }
    Ok(palette)
}

/// Borrow the embedded image ICC profile after validating parasite record bounds.
/// This preserves source color metadata; it performs no profile transformation.
pub fn xcf_icc_profile(bytes: &[u8]) -> Result<Option<&[u8]>, XcfHeaderError> {
    let mut profile = None;
    let mut records = 0usize;
    for property in xcf_properties(bytes)? {
        let property = property?;
        if property.kind != 21 {
            continue;
        }
        let mut position = 0;
        while position < property.payload.len() {
            records += 1;
            if records > 4096 {
                return Err(XcfHeaderError("parasite count"));
            }
            let length =
                u32::from_be_bytes(read_at(property.payload, &mut position, 4)?.try_into().unwrap()) as usize;
            if length == 0 || length > 1024 * 1024 {
                return Err(XcfHeaderError("parasite name length"));
            }
            let name = read_at(property.payload, &mut position, length)?;
            if name.last() != Some(&0) {
                return Err(XcfHeaderError("parasite name terminator"));
            }
            let _flags = read_at(property.payload, &mut position, 4)?;
            let length =
                u32::from_be_bytes(read_at(property.payload, &mut position, 4)?.try_into().unwrap()) as usize;
            let data = read_at(property.payload, &mut position, length)?;
            if name == b"icc-profile\0" {
                if profile.is_some() {
                    return Err(XcfHeaderError("duplicate ICC profile"));
                }
                if data.len() < 128
                    || data.len() > 16 * 1024 * 1024
                    || data.get(36..40) != Some(b"acsp")
                    || u32::from_be_bytes(data[..4].try_into().unwrap()) as usize != data.len()
                {
                    return Err(XcfHeaderError("ICC profile header"));
                }
                profile = Some(data);
            }
        }
    }
    Ok(profile)
}

/// Admission-checked tree plan, not a rendered group composite.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct XcfLayerNode {
    pub offset: u64,
    /// None denotes a canvas child; otherwise an index into this plan.
    pub parent: Option<u32>,
    pub sibling: u32,
    pub is_group: bool,
}

pub fn parse_xcf_layer_tree(
    bytes: &[u8],
    budget: &rrrah_core::MemoryBudget,
    mut cancelled: impl FnMut() -> bool,
) -> Result<rrrah_core::SharedBuffer<XcfLayerNode>, XcfPixelError> {
    let tables = parse_xcf_object_tables(bytes)?;
    let count = tables.layers.iter().count();
    if cancelled() {
        return Err(XcfPixelError::Cancelled);
    }
    let mut nodes = budget.try_buffer(count, XcfLayerNode::default())?;
    let mut root_index = 0u32;
    for (index, offset) in tables.layers.iter().enumerate() {
        if cancelled() {
            return Err(XcfPixelError::Cancelled);
        }
        let layer = parse_xcf_layer(bytes, offset)?;
        let group = layer.group()?;
        let mut path = group.item_path().peekable();
        let mut parent = None;
        let sibling = if let Some(first) = path.next() {
            let mut component = first;
            while path.peek().is_some() {
                let found = nodes[..index]
                    .iter()
                    .position(|node| node.parent == parent && node.sibling == component)
                    .ok_or(XcfHeaderError("missing group parent"))?;
                if !nodes[found].is_group {
                    return Err(XcfHeaderError("non-group parent").into());
                }
                parent = Some(found as u32);
                component = path.next().unwrap();
            }
            component
        } else {
            root_index
        };
        if nodes[..index]
            .iter()
            .any(|node| node.parent == parent && node.sibling == sibling)
        {
            return Err(XcfHeaderError("duplicate group position").into());
        }
        if parent.is_none() {
            root_index = root_index.checked_add(1).ok_or(XcfHeaderError("root count"))?;
        }
        nodes[index] = XcfLayerNode {
            offset,
            parent,
            sibling,
            is_group: group.is_group,
        };
    }
    if cancelled() {
        return Err(XcfPixelError::Cancelled);
    }
    Ok(nodes.freeze())
}

/// Normalize stored components to f32 without gamma/ICC conversion or clamping.
/// Float HDR/negative values are preserved; nonfinite/unrepresentable samples
/// are refused. High-precision pre-v12 byte order is not inferred.
pub fn decode_xcf_samples(
    source: &[u8],
    header: XcfHeader,
    budget: &rrrah_core::MemoryBudget,
    mut cancelled: impl FnMut() -> bool,
) -> Result<rrrah_core::SharedBuffer<f32>, XcfPixelError> {
    if header.sample_bytes > 1 && header.version < 12 {
        return Err(XcfHeaderError("ambiguous sample byte order").into());
    }
    let (size, category) = match header.precision {
        100 | 150 | 175 => (1usize, 0),
        200 | 250 | 275 => (2, 1),
        300 | 350 | 375 => (4, 2),
        500 | 550 | 575 => (2, 3),
        600 | 650 | 675 => (4, 4),
        700 | 750 | 775 => (8, 5),
        _ => return Err(XcfHeaderError("sample precision").into()),
    };
    if size != usize::from(header.sample_bytes) || source.is_empty() || source.len() % size != 0 {
        return Err(XcfHeaderError("sample length").into());
    }
    let count = source.len() / size;
    if (count as u64)
        .checked_mul(4)
        .is_none_or(|n| n > crate::raster::MAX_RASTER_BYTES)
    {
        return Err(XcfHeaderError("sample size").into());
    }
    if cancelled() {
        return Err(XcfPixelError::Cancelled);
    }
    let mut output = budget.try_buffer(count, 0.0f32)?;
    for (index, (sample, target)) in source.chunks_exact(size).zip(output.iter_mut()).enumerate() {
        if index % 1024 == 0 && cancelled() {
            return Err(XcfPixelError::Cancelled);
        }
        let value = match category {
            0 => sample[0] as f32 / 255.0,
            1 => u16::from_be_bytes(sample.try_into().unwrap()) as f32 / 65535.0,
            2 => (u32::from_be_bytes(sample.try_into().unwrap()) as f64 / u32::MAX as f64) as f32,
            3 => crate::sample::half_to_f32(u16::from_be_bytes(sample.try_into().unwrap())),
            4 => f32::from_be_bytes(sample.try_into().unwrap()),
            5 => f64::from_be_bytes(sample.try_into().unwrap()) as f32,
            _ => unreachable!(),
        };
        if !value.is_finite() {
            return Err(XcfHeaderError("nonfinite sample").into());
        }
        *target = value;
    }
    if cancelled() {
        return Err(XcfPixelError::Cancelled);
    }
    Ok(output.freeze())
}

/// Prepare legacy 8-bit storage for composition. Already-RGBA storage transfers
/// shared ownership without another allocation; other modes retain their source
/// reservation while the managed expansion is created.
pub fn prepare_xcf_legacy_rgba8(
    source: rrrah_core::SharedBuffer<u8>,
    kind: u32,
    palette: &[[u8; 3]],
    budget: &rrrah_core::MemoryBudget,
    mut cancelled: impl FnMut() -> bool,
) -> Result<rrrah_core::SharedBuffer<u8>, XcfPixelError> {
    if kind == 1 {
        if source.is_empty() || source.len() % 4 != 0 || source.len() as u64 > crate::raster::MAX_RASTER_BYTES
        {
            return Err(XcfHeaderError("layer sample length").into());
        }
        if cancelled() {
            return Err(XcfPixelError::Cancelled);
        }
        return Ok(source);
    }
    expand_xcf_legacy_u8_pixels(&source, kind, palette, budget, cancelled)
}

/// Expand native 8-bit layer samples to straight RGBA8 without color conversion.
/// Palette contains RGB triples; indexed alpha retains legacy binary semantics.
/// Higher precision samples must use a separate qualified conversion path.
pub fn expand_xcf_legacy_u8_pixels(
    source: &[u8],
    kind: u32,
    palette: &[[u8; 3]],
    budget: &rrrah_core::MemoryBudget,
    mut cancelled: impl FnMut() -> bool,
) -> Result<rrrah_core::SharedBuffer<u8>, XcfPixelError> {
    let stride = *[3usize, 4, 1, 2, 1, 2]
        .get(kind as usize)
        .ok_or(XcfHeaderError("layer color mode"))?;
    if source.is_empty() || source.len() % stride != 0 {
        return Err(XcfHeaderError("layer sample length").into());
    }
    let pixels = source.len() / stride;
    let length = pixels.checked_mul(4).ok_or(XcfHeaderError("layer size"))?;
    if length as u64 > crate::raster::MAX_RASTER_BYTES {
        return Err(XcfHeaderError("layer size").into());
    }
    if cancelled() {
        return Err(XcfPixelError::Cancelled);
    }
    if kind >= 4 {
        if palette.is_empty() || palette.len() > 256 {
            return Err(XcfHeaderError("layer palette").into());
        }
        for (index, sample) in source.chunks_exact(stride).enumerate() {
            if index % 1024 == 0 && cancelled() {
                return Err(XcfPixelError::Cancelled);
            }
            if usize::from(sample[0]) >= palette.len() {
                return Err(XcfHeaderError("palette index").into());
            }
        }
    }
    let mut output = budget.try_buffer(length, 0u8)?;
    for (index, (sample, rgba)) in source
        .chunks_exact(stride)
        .zip(output.chunks_exact_mut(4))
        .enumerate()
    {
        if index % 1024 == 0 && cancelled() {
            return Err(XcfPixelError::Cancelled);
        }
        match kind {
            0 | 1 => rgba[..3].copy_from_slice(&sample[..3]),
            2 | 3 => rgba[..3].fill(sample[0]),
            4 | 5 => rgba[..3].copy_from_slice(&palette[usize::from(sample[0])]),
            _ => unreachable!(),
        }
        rgba[3] = match kind {
            1 => sample[3],
            3 => sample[1],
            5 => {
                if sample[1] < 128 {
                    0
                } else {
                    255
                }
            }
            _ => 255,
        };
    }
    if cancelled() {
        return Err(XcfPixelError::Cancelled);
    }
    Ok(output.freeze())
}

/// Normal source-over in the caller-selected component space, without color
/// conversion. Source is straight RGBA8; canvas is premultiplied RGBA32F.
/// The caller owns/budgets both buffers and must validate the XCF blend and
/// composite modes before choosing this operation. This is not a file renderer.
/// Cancellation may leave partial canvas writes; publish only after success.
pub fn composite_xcf_normal_rgba8(
    canvas: &mut [f32],
    canvas_width: u32,
    canvas_height: u32,
    source: &[u8],
    width: u32,
    height: u32,
    mask: Option<&[u8]>,
    attributes: XcfLayerAttributes,
    mut cancelled: impl FnMut() -> bool,
) -> Result<(), XcfPixelError> {
    let length = |w: u32, h: u32, stride: usize| {
        usize::try_from(w)
            .ok()
            .and_then(|w| usize::try_from(h).ok().and_then(|h| w.checked_mul(h)))
            .and_then(|n| n.checked_mul(stride))
            .ok_or(XcfHeaderError("composite size"))
    };
    if canvas_width == 0
        || canvas_height == 0
        || width == 0
        || height == 0
        || canvas.len() != length(canvas_width, canvas_height, 4)?
        || source.len() != length(width, height, 4)?
        || mask.is_some_and(|m| Some(m.len()) != length(width, height, 1).ok())
        || (attributes.apply_mask && mask.is_none())
        || !attributes.opacity.is_finite()
        || !(0.0..=1.0).contains(&attributes.opacity)
    {
        return Err(XcfHeaderError("composite inputs").into());
    }
    if cancelled() {
        return Err(XcfPixelError::Cancelled);
    }
    if !attributes.visible || attributes.opacity == 0.0 {
        return Ok(());
    }
    let x0 = 0i64.max(-i64::from(attributes.offset_x));
    let y0 = 0i64.max(-i64::from(attributes.offset_y));
    let x1 = i64::from(width).min(i64::from(canvas_width) - i64::from(attributes.offset_x));
    let y1 = i64::from(height).min(i64::from(canvas_height) - i64::from(attributes.offset_y));
    for y in y0..y1 {
        if cancelled() {
            return Err(XcfPixelError::Cancelled);
        }
        for x in x0..x1 {
            let pixel = y as usize * width as usize + x as usize;
            let from = pixel * 4;
            let target = ((y + i64::from(attributes.offset_y)) as usize * canvas_width as usize
                + (x + i64::from(attributes.offset_x)) as usize)
                * 4;
            let mut alpha = source[from + 3] as f32 / 255.0 * attributes.opacity;
            if attributes.apply_mask {
                alpha *= mask.unwrap()[pixel] as f32 / 255.0;
            }
            let remaining = 1.0 - alpha;
            for c in 0..3 {
                canvas[target + c] = source[from + c] as f32 / 255.0 * alpha + canvas[target + c] * remaining;
            }
            canvas[target + 3] = alpha + canvas[target + 3] * remaining;
        }
    }
    if cancelled() {
        return Err(XcfPixelError::Cancelled);
    }
    Ok(())
}

/// Normal source-over with straight float source and premultiplied float canvas.
/// Components remain in the caller-selected space, preserving finite HDR and
/// negative RGB. Source alpha/mask must be in [0,1]. No allocations are made.
/// Caller must provide a valid premultiplied canvas and qualify XCF modes/color.
/// Cancellation can leave partial writes; do not publish a cancelled canvas.
pub fn composite_xcf_normal_rgba32f(
    canvas: &mut [f32],
    canvas_width: u32,
    canvas_height: u32,
    source: &[f32],
    width: u32,
    height: u32,
    mask: Option<&[f32]>,
    attributes: XcfLayerAttributes,
    cancelled: impl FnMut() -> bool,
) -> Result<(), XcfPixelError> {
    composite_xcf_float(
        canvas,
        canvas_width,
        canvas_height,
        source,
        width,
        height,
        mask,
        attributes,
        false,
        cancelled,
    )
}

/// Composite an already-premultiplied group without unpremultiplication/copying.
/// Group opacity and mask scale RGB and alpha together. Caller-space and
/// cancellation contracts are the same as the straight float operation.
pub fn composite_xcf_normal_premultiplied_rgba32f(
    canvas: &mut [f32],
    canvas_width: u32,
    canvas_height: u32,
    source: &[f32],
    width: u32,
    height: u32,
    mask: Option<&[f32]>,
    attributes: XcfLayerAttributes,
    cancelled: impl FnMut() -> bool,
) -> Result<(), XcfPixelError> {
    composite_xcf_float(
        canvas,
        canvas_width,
        canvas_height,
        source,
        width,
        height,
        mask,
        attributes,
        true,
        cancelled,
    )
}

fn composite_xcf_float(
    canvas: &mut [f32],
    canvas_width: u32,
    canvas_height: u32,
    source: &[f32],
    width: u32,
    height: u32,
    mask: Option<&[f32]>,
    attributes: XcfLayerAttributes,
    premultiplied: bool,
    mut cancelled: impl FnMut() -> bool,
) -> Result<(), XcfPixelError> {
    let count = |w: u32, h: u32| {
        usize::try_from(w)
            .ok()
            .and_then(|w| usize::try_from(h).ok().and_then(|h| w.checked_mul(h)))
            .ok_or(XcfHeaderError("composite size"))
    };
    let source_count = count(width, height)?;
    let canvas_count = count(canvas_width, canvas_height)?;
    if source_count == 0
        || canvas_count == 0
        || source_count.checked_mul(4) != Some(source.len())
        || canvas_count.checked_mul(4) != Some(canvas.len())
        || mask.is_some_and(|m| m.len() != source_count)
        || (attributes.apply_mask && mask.is_none())
        || !attributes.opacity.is_finite()
        || !(0.0..=1.0).contains(&attributes.opacity)
    {
        return Err(XcfHeaderError("composite inputs").into());
    }
    if cancelled() {
        return Err(XcfPixelError::Cancelled);
    }
    for (index, rgba) in source.chunks_exact(4).enumerate() {
        if index % 1024 == 0 && cancelled() {
            return Err(XcfPixelError::Cancelled);
        }
        if rgba.iter().any(|v| !v.is_finite())
            || !(0.0..=1.0).contains(&rgba[3])
            || mask.is_some_and(|m| !m[index].is_finite() || !(0.0..=1.0).contains(&m[index]))
        {
            return Err(XcfHeaderError("composite sample").into());
        }
    }
    if !attributes.visible || attributes.opacity == 0.0 {
        return Ok(());
    }
    let x0 = 0i64.max(-i64::from(attributes.offset_x));
    let y0 = 0i64.max(-i64::from(attributes.offset_y));
    let x1 = i64::from(width).min(i64::from(canvas_width) - i64::from(attributes.offset_x));
    let y1 = i64::from(height).min(i64::from(canvas_height) - i64::from(attributes.offset_y));
    for y in y0..y1 {
        if cancelled() {
            return Err(XcfPixelError::Cancelled);
        }
        for x in x0..x1 {
            let pixel = y as usize * width as usize + x as usize;
            let source_index = pixel * 4;
            let target = ((y + i64::from(attributes.offset_y)) as usize * canvas_width as usize
                + (x + i64::from(attributes.offset_x)) as usize)
                * 4;
            let mut factor = attributes.opacity;
            if attributes.apply_mask {
                factor *= mask.unwrap()[pixel];
            }
            let alpha = source[source_index + 3] * factor;
            let color_factor = if premultiplied { factor } else { alpha };
            for component in 0..3 {
                canvas[target + component] = source[source_index + component] * color_factor
                    + canvas[target + component] * (1.0 - alpha);
            }
            canvas[target + 3] = alpha + canvas[target + 3] * (1.0 - alpha);
        }
    }
    if cancelled() {
        return Err(XcfPixelError::Cancelled);
    }
    Ok(())
}

/// Assemble a validated main level into managed interleaved storage. This does
/// not interpret sample values, color profiles, masks or blend/composite modes.
pub fn decode_xcf_level(
    bytes: &[u8],
    level: &XcfLevel<'_>,
    compression: u8,
    budget: &rrrah_core::MemoryBudget,
    mut cancelled: impl FnMut() -> bool,
) -> Result<rrrah_core::SharedBuffer<u8>, XcfPixelError> {
    if cancelled() {
        return Err(XcfPixelError::Cancelled);
    }
    if level.width == 0 || level.height == 0 || !(1..=32).contains(&level.bytes_per_pixel) {
        return Err(XcfHeaderError("level geometry").into());
    }
    let stride = usize::from(level.bytes_per_pixel);
    let width = usize::try_from(level.width).map_err(|_| XcfHeaderError("level size"))?;
    let height = usize::try_from(level.height).map_err(|_| XcfHeaderError("level size"))?;
    let length = width
        .checked_mul(height)
        .and_then(|n| n.checked_mul(stride))
        .ok_or(XcfHeaderError("level size"))?;
    if length as u64 > crate::raster::MAX_RASTER_BYTES {
        return Err(XcfHeaderError("level size").into());
    }
    let across = width.div_ceil(64);
    let down = height.div_ceil(64);
    if level.tiles.iter().next().is_none() {
        let output = budget.try_buffer(length, 0u8)?;
        if cancelled() {
            return Err(XcfPixelError::Cancelled);
        }
        return Ok(output.freeze());
    }
    if level.tiles.iter().count() != across * down {
        return Err(XcfHeaderError("tile count").into());
    }
    let mut output = budget.try_buffer(length, 0u8)?;
    let mut tile = budget.try_buffer(width.min(64) * height.min(64) * stride, 0u8)?;
    let mut offsets = level.tiles.iter().peekable();
    let mut index = 0;
    while let Some(offset) = offsets.next() {
        if cancelled() {
            return Err(XcfPixelError::Cancelled);
        }
        let x = index % across * 64;
        let y = index / across * 64;
        let tile_width = (width - x).min(64);
        let tile_height = (height - y).min(64);
        let tile_length = tile_width * tile_height * stride;
        let offset = usize::try_from(offset).map_err(|_| XcfHeaderError("tile offset"))?;
        let end = offsets.peek().copied().unwrap_or(bytes.len() as u64);
        let end = usize::try_from(end).map_err(|_| XcfHeaderError("tile offset"))?;
        let input = bytes.get(offset..end).ok_or(XcfHeaderError("tile offset"))?;
        decode_xcf_tile(
            compression,
            input,
            tile_width as u8,
            tile_height as u8,
            level.bytes_per_pixel,
            &mut tile[..tile_length],
        )?;
        for row in 0..tile_height {
            let target = ((y + row) * width + x) * stride;
            let source = row * tile_width * stride;
            output[target..target + tile_width * stride]
                .copy_from_slice(&tile[source..source + tile_width * stride]);
        }
        index += 1;
    }
    if cancelled() {
        return Err(XcfPixelError::Cancelled);
    }
    Ok(output.freeze())
}

#[derive(Debug)]
pub struct XcfLevel<'a> {
    pub width: u32,
    pub height: u32,
    pub bytes_per_pixel: u8,
    pub tiles: XcfOffsets<'a>,
    pub end: usize,
}

/// Validate the main pixel level against its caller's layer/channel dimensions.
/// Unused lower-resolution levels are skipped, never used as a preview fallback.
pub fn parse_xcf_hierarchy(
    bytes: &[u8],
    offset: u64,
    width: u32,
    height: u32,
    bytes_per_pixel: u8,
) -> Result<XcfLevel<'_>, XcfHeaderError> {
    let header = parse_xcf_header(bytes)?;
    let offset_width = usize::from(header.offset_bytes);
    let mut position = usize::try_from(offset).map_err(|_| XcfHeaderError("hierarchy offset"))?;
    if position < header.properties_offset
        || width == 0
        || height == 0
        || !(1..=32).contains(&bytes_per_pixel)
    {
        return Err(XcfHeaderError("hierarchy geometry"));
    }
    let plane = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(u64::from(bytes_per_pixel)))
        .ok_or(XcfHeaderError("hierarchy size"))?;
    if plane > crate::raster::MAX_RASTER_BYTES {
        return Err(XcfHeaderError("hierarchy size"));
    }
    let dimensions = read_at(bytes, &mut position, 12)?;
    for (value, expected) in dimensions
        .chunks_exact(4)
        .zip([width, height, u32::from(bytes_per_pixel)])
    {
        if u32::from_be_bytes(value.try_into().unwrap()) != expected {
            return Err(XcfHeaderError("hierarchy dimensions"));
        }
    }
    let level_offset = offset_value(read_at(bytes, &mut position, offset_width)?);
    if level_offset < header.properties_offset as u64 || level_offset >= bytes.len() as u64 {
        return Err(XcfHeaderError("level offset"));
    }
    let mut terminated = false;
    for _ in 0..64 {
        let dummy = offset_value(read_at(bytes, &mut position, offset_width)?);
        if dummy == 0 {
            terminated = true;
            break;
        }
        if dummy < header.properties_offset as u64 || dummy >= bytes.len() as u64 {
            return Err(XcfHeaderError("dummy level offset"));
        }
    }
    if !terminated {
        return Err(XcfHeaderError("level count"));
    }
    position = usize::try_from(level_offset).map_err(|_| XcfHeaderError("level offset"))?;
    let dimensions = read_at(bytes, &mut position, 8)?;
    if u32::from_be_bytes(dimensions[..4].try_into().unwrap()) != width
        || u32::from_be_bytes(dimensions[4..].try_into().unwrap()) != height
    {
        return Err(XcfHeaderError("level dimensions"));
    }
    let count = u64::from(width.div_ceil(64)) * u64::from(height.div_ceil(64));
    let first_position = position;
    if offset_value(read_at(bytes, &mut position, offset_width)?) == 0 {
        return Ok(XcfLevel {
            width,
            height,
            bytes_per_pixel,
            tiles: XcfOffsets {
                data: &bytes[first_position..first_position],
                width: offset_width,
            },
            end: position,
        });
    }
    position = first_position;
    let table_bytes = usize::try_from(count)
        .ok()
        .and_then(|count| count.checked_mul(offset_width))
        .ok_or(XcfHeaderError("tile table size"))?;
    let raw = read_at(bytes, &mut position, table_bytes)?;
    if offset_value(read_at(bytes, &mut position, offset_width)?) != 0 {
        return Err(XcfHeaderError("tile table terminator"));
    }
    let tiles = XcfOffsets {
        data: raw,
        width: offset_width,
    };
    if tiles
        .iter()
        .any(|offset| offset < position as u64 || offset >= bytes.len() as u64)
    {
        return Err(XcfHeaderError("tile offset"));
    }
    let mut previous = None;
    for offset in tiles.iter() {
        if previous.is_some_and(|prior| offset <= prior) {
            return Err(XcfHeaderError("tile offset order"));
        }
        previous = Some(offset);
    }
    Ok(XcfLevel {
        width,
        height,
        bytes_per_pixel,
        tiles,
        end: position,
    })
}

/// Decode one at-most-64x64 tile into caller-owned storage, returning consumed
/// source bytes. Publish output only after success: errors may leave partial data.
/// No pixel allocation is performed; zlib's internal scratch is not budgeted here.
pub fn decode_xcf_tile(
    compression: u8,
    input: &[u8],
    width: u8,
    height: u8,
    bytes_per_pixel: u8,
    output: &mut [u8],
) -> Result<usize, XcfHeaderError> {
    if width == 0 || height == 0 || width > 64 || height > 64 || !(1..=32).contains(&bytes_per_pixel) {
        return Err(XcfHeaderError("tile geometry"));
    }
    let pixels = usize::from(width) * usize::from(height);
    let stride = usize::from(bytes_per_pixel);
    if output.len() != pixels * stride {
        return Err(XcfHeaderError("tile output length"));
    }
    match compression {
        0 => {
            output.copy_from_slice(
                input
                    .get(..output.len())
                    .ok_or(XcfHeaderError("truncated tile"))?,
            );
            Ok(output.len())
        }
        1 => {
            let mut position = 0;
            for channel in 0..stride {
                let mut written = 0;
                while written < pixels {
                    if position > output.len() * 2 + 1024 {
                        return Err(XcfHeaderError("RLE work limit"));
                    }
                    let opcode = read_at(input, &mut position, 1)?[0];
                    let run = match opcode {
                        127 | 128 => usize::from(u16::from_be_bytes(
                            read_at(input, &mut position, 2)?.try_into().unwrap(),
                        )),
                        0..=126 => usize::from(opcode) + 1,
                        _ => 256 - usize::from(opcode),
                    };
                    if run > pixels - written {
                        return Err(XcfHeaderError("RLE channel overflow"));
                    }
                    if opcode <= 127 {
                        let value = read_at(input, &mut position, 1)?[0];
                        for pixel in written..written + run {
                            output[pixel * stride + channel] = value;
                        }
                    } else {
                        let values = read_at(input, &mut position, run)?;
                        for (index, value) in values.iter().enumerate() {
                            output[(written + index) * stride + channel] = *value;
                        }
                    }
                    written += run;
                }
            }
            Ok(position)
        }
        2 => {
            let mut decoder = flate2::Decompress::new(true);
            let status = decoder
                .decompress(input, output, flate2::FlushDecompress::Finish)
                .map_err(|_| XcfHeaderError("invalid zlib tile"))?;
            if status != flate2::Status::StreamEnd || decoder.total_out() != output.len() as u64 {
                return Err(XcfHeaderError("zlib tile length"));
            }
            usize::try_from(decoder.total_in()).map_err(|_| XcfHeaderError("tile input length"))
        }
        _ => Err(XcfHeaderError("tile compression")),
    }
}

#[derive(Debug)]
pub struct XcfLayer<'a> {
    pub width: u32,
    pub height: u32,
    pub kind: u32,
    pub name: &'a [u8],
    pub properties: XcfProperties<'a>,
    pub hierarchy_offset: u64,
    pub mask_offset: u64,
    pub effects: XcfOffsets<'a>,
    pub end: usize,
    properties_start: usize,
}

/// Basic layer attributes only; this does not qualify blending or composition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XcfLayerAttributes {
    pub visible: bool,
    pub opacity: f32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub apply_mask: bool,
}

/// Stored composition choices. Unknown values are preserved, never normalized
/// into supported modes. A renderer must explicitly qualify them before use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XcfCompositionAttributes {
    pub blend_mode: u32,
    pub composite_mode: Option<i32>,
    pub composite_space: Option<i32>,
    pub blend_space: Option<i32>,
}

/// Group membership is distinct from pixel dimensions and global layer order.
#[derive(Debug)]
pub struct XcfLayerGroup<'a> {
    pub is_group: bool,
    item_path: &'a [u8],
}
impl XcfLayerGroup<'_> {
    pub fn item_path(&self) -> impl Iterator<Item = u32> + '_ {
        self.item_path
            .chunks_exact(4)
            .map(|word| u32::from_be_bytes(word.try_into().unwrap()))
    }
}

impl XcfLayer<'_> {
    pub fn group(&self) -> Result<XcfLayerGroup<'_>, XcfHeaderError> {
        let mut result = XcfLayerGroup {
            is_group: false,
            item_path: &[],
        };
        let mut seen_path = false;
        let properties = XcfProperties {
            bytes: self.properties.bytes,
            position: self.properties_start,
            count: 0,
            done: false,
        };
        for property in properties {
            let property = property?;
            match property.kind {
                29 => {
                    if !property.payload.is_empty() || result.is_group {
                        return Err(XcfHeaderError("group marker"));
                    }
                    result.is_group = true;
                }
                30 => {
                    if seen_path || property.payload.len() % 4 != 0 || property.payload.len() > 256 * 4 {
                        return Err(XcfHeaderError("group item path"));
                    }
                    seen_path = true;
                    result.item_path = property.payload;
                }
                _ => {}
            }
        }
        Ok(result)
    }

    pub fn composition_attributes(&self) -> Result<XcfCompositionAttributes, XcfHeaderError> {
        let mut result = XcfCompositionAttributes {
            blend_mode: 0,
            composite_mode: None,
            composite_space: None,
            blend_space: None,
        };
        let properties = XcfProperties {
            bytes: self.properties.bytes,
            position: self.properties_start,
            count: 0,
            done: false,
        };
        for property in properties {
            let property = property?;
            if matches!(property.kind, 7 | 35..=37) {
                let raw: [u8; 4] = property
                    .payload
                    .try_into()
                    .map_err(|_| XcfHeaderError("composition attribute length"))?;
                match property.kind {
                    7 => result.blend_mode = u32::from_be_bytes(raw),
                    35 => result.composite_mode = Some(i32::from_be_bytes(raw)),
                    36 => result.composite_space = Some(i32::from_be_bytes(raw)),
                    37 => result.blend_space = Some(i32::from_be_bytes(raw)),
                    _ => unreachable!(),
                }
            }
        }
        Ok(result)
    }

    pub fn attributes(&self) -> Result<XcfLayerAttributes, XcfHeaderError> {
        let mut result = XcfLayerAttributes {
            visible: true,
            opacity: 1.0,
            offset_x: 0,
            offset_y: 0,
            apply_mask: self.mask_offset != 0,
        };
        let mut float_opacity = None;
        let properties = XcfProperties {
            bytes: self.properties.bytes,
            position: self.properties_start,
            count: 0,
            done: false,
        };
        for property in properties {
            let property = property?;
            match property.kind {
                6 | 8 | 11 | 33 => {
                    let raw: [u8; 4] = property
                        .payload
                        .try_into()
                        .map_err(|_| XcfHeaderError("layer attribute length"))?;
                    let word = u32::from_be_bytes(raw);
                    match property.kind {
                        6 => {
                            if word > 255 {
                                return Err(XcfHeaderError("layer opacity"));
                            }
                            result.opacity = word as f32 / 255.0;
                        }
                        8 | 11 => {
                            if word > 1 {
                                return Err(XcfHeaderError("layer boolean"));
                            }
                            if property.kind == 8 {
                                result.visible = word != 0;
                            } else {
                                result.apply_mask = word != 0 && self.mask_offset != 0;
                            }
                        }
                        33 => {
                            let opacity = f32::from_be_bytes(raw);
                            if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
                                return Err(XcfHeaderError("layer float opacity"));
                            }
                            float_opacity = Some(opacity);
                        }
                        _ => unreachable!(),
                    }
                }
                15 => {
                    let raw: [u8; 8] = property
                        .payload
                        .try_into()
                        .map_err(|_| XcfHeaderError("layer offset length"))?;
                    result.offset_x = i32::from_be_bytes(raw[..4].try_into().unwrap());
                    result.offset_y = i32::from_be_bytes(raw[4..].try_into().unwrap());
                }
                _ => {}
            }
        }
        if let Some(opacity) = float_opacity {
            result.opacity = opacity;
        }
        Ok(result)
    }
}

fn read_at<'a>(bytes: &'a [u8], position: &mut usize, length: usize) -> Result<&'a [u8], XcfHeaderError> {
    let end = position
        .checked_add(length)
        .ok_or(XcfHeaderError("record length"))?;
    let result = bytes
        .get(*position..end)
        .ok_or(XcfHeaderError("truncated record"))?;
    *position = end;
    Ok(result)
}

/// A channel record, also used for layer masks. Pixel interpretation is separate.
#[derive(Debug)]
pub struct XcfChannel<'a> {
    pub width: u32,
    pub height: u32,
    pub name: &'a [u8],
    pub properties: XcfProperties<'a>,
    pub hierarchy_offset: u64,
    pub end: usize,
}

/// Parse a channel with the dimensions of its parent canvas or layer.
/// Borrows metadata and performs no allocation or color conversion.
pub fn parse_xcf_channel(
    bytes: &[u8],
    offset: u64,
    expected_width: u32,
    expected_height: u32,
) -> Result<XcfChannel<'_>, XcfHeaderError> {
    let header = parse_xcf_header(bytes)?;
    let mut position = usize::try_from(offset).map_err(|_| XcfHeaderError("channel offset"))?;
    if position < header.properties_offset {
        return Err(XcfHeaderError("channel offset"));
    }
    let dimensions = read_at(bytes, &mut position, 8)?;
    let width = u32::from_be_bytes(dimensions[..4].try_into().unwrap());
    let height = u32::from_be_bytes(dimensions[4..].try_into().unwrap());
    if width == 0 || height == 0 || width != expected_width || height != expected_height {
        return Err(XcfHeaderError("channel dimensions"));
    }
    let plane = u64::from(width) * u64::from(height);
    if plane
        .checked_mul(u64::from(header.sample_bytes))
        .is_none_or(|size| size > crate::raster::MAX_RASTER_BYTES)
    {
        return Err(XcfHeaderError("channel size"));
    }
    let length = u32::from_be_bytes(read_at(bytes, &mut position, 4)?.try_into().unwrap()) as usize;
    if length > 1024 * 1024 {
        return Err(XcfHeaderError("channel name length"));
    }
    let encoded = read_at(bytes, &mut position, length)?;
    let name = if encoded.is_empty() {
        encoded
    } else {
        if encoded.last() != Some(&0) {
            return Err(XcfHeaderError("channel name terminator"));
        }
        &encoded[..encoded.len() - 1]
    };
    let properties_start = position;
    let mut properties = XcfProperties {
        bytes,
        position,
        count: 0,
        done: false,
    };
    for property in properties.by_ref() {
        property?;
    }
    position = properties.position();
    let hierarchy_offset = offset_value(read_at(bytes, &mut position, usize::from(header.offset_bytes))?);
    if hierarchy_offset < header.properties_offset as u64 || hierarchy_offset >= bytes.len() as u64 {
        return Err(XcfHeaderError("channel hierarchy offset"));
    }
    Ok(XcfChannel {
        width,
        height,
        name,
        properties: XcfProperties {
            bytes,
            position: properties_start,
            count: 0,
            done: false,
        },
        hierarchy_offset,
        end: position,
    })
}

/// Parse one referenced layer record; pixel and mask objects remain undecoded.
pub fn parse_xcf_layer(bytes: &[u8], offset: u64) -> Result<XcfLayer<'_>, XcfHeaderError> {
    let header = parse_xcf_header(bytes)?;
    let mut position = usize::try_from(offset).map_err(|_| XcfHeaderError("layer offset"))?;
    if position < header.properties_offset {
        return Err(XcfHeaderError("layer offset"));
    }
    let dimensions = read_at(bytes, &mut position, 12)?;
    let width = u32::from_be_bytes(dimensions[..4].try_into().unwrap());
    let height = u32::from_be_bytes(dimensions[4..8].try_into().unwrap());
    let kind = u32::from_be_bytes(dimensions[8..].try_into().unwrap());
    if kind > 5 {
        return Err(XcfHeaderError("layer color mode"));
    }
    let plane = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4 * u64::from(header.sample_bytes)))
        .ok_or(XcfHeaderError("layer size"))?;
    if plane > crate::raster::MAX_RASTER_BYTES {
        return Err(XcfHeaderError("layer size"));
    }
    let name_length = u32::from_be_bytes(read_at(bytes, &mut position, 4)?.try_into().unwrap()) as usize;
    if name_length > 1024 * 1024 {
        return Err(XcfHeaderError("layer name length"));
    }
    let encoded_name = read_at(bytes, &mut position, name_length)?;
    let name = if encoded_name.is_empty() {
        encoded_name
    } else {
        if encoded_name.last() != Some(&0) {
            return Err(XcfHeaderError("layer name terminator"));
        }
        &encoded_name[..encoded_name.len() - 1]
    };
    let properties_start = position;
    let mut properties = XcfProperties {
        bytes,
        position,
        count: 0,
        done: false,
    };
    for property in properties.by_ref() {
        property?;
    }
    position = properties.position();
    let width_offsets = usize::from(header.offset_bytes);
    let hierarchy_offset = offset_value(read_at(bytes, &mut position, width_offsets)?);
    let mask_offset = offset_value(read_at(bytes, &mut position, width_offsets)?);
    let effects_start = position;
    let mut effects_end = position;
    if header.version >= 20 {
        for count in 0..=4096 {
            let reference = offset_value(read_at(bytes, &mut position, width_offsets)?);
            if reference == 0 {
                break;
            }
            if count == 4096 {
                return Err(XcfHeaderError("effect count"));
            }
            if reference >= bytes.len() as u64 {
                return Err(XcfHeaderError("effect offset"));
            }
            effects_end = position;
        }
    }
    for reference in [hierarchy_offset, mask_offset] {
        if reference != 0 && (reference >= bytes.len() as u64 || reference < header.properties_offset as u64)
        {
            return Err(XcfHeaderError("layer object offset"));
        }
    }
    Ok(XcfLayer {
        width,
        height,
        kind,
        name,
        properties: XcfProperties {
            bytes,
            position: properties_start,
            count: 0,
            done: false,
        },
        hierarchy_offset,
        mask_offset,
        effects: XcfOffsets {
            data: &bytes[effects_start..effects_end],
            width: width_offsets,
        },
        end: position,
        properties_start,
    })
}

#[derive(Debug)]
pub struct XcfObjectTables<'a> {
    pub layers: XcfOffsets<'a>,
    pub channels: XcfOffsets<'a>,
    pub end: usize,
}

#[derive(Debug)]
pub struct XcfOffsets<'a> {
    data: &'a [u8],
    width: usize,
}
impl XcfOffsets<'_> {
    pub fn iter(&self) -> impl Iterator<Item = u64> + '_ {
        self.data.chunks_exact(self.width).map(offset_value)
    }
}
fn offset_value(data: &[u8]) -> u64 {
    if data.len() == 8 {
        u64::from_be_bytes(data.try_into().unwrap())
    } else {
        u64::from(u32::from_be_bytes(data.try_into().unwrap()))
    }
}

/// Parse both terminated object tables. This validates references, not objects.
pub fn parse_xcf_object_tables(bytes: &[u8]) -> Result<XcfObjectTables<'_>, XcfHeaderError> {
    let header = parse_xcf_header(bytes)?;
    let mut properties = xcf_properties(bytes)?;
    for property in properties.by_ref() {
        property?;
    }
    let width = usize::from(header.offset_bytes);
    let mut position = properties.position();
    let mut count = 0usize;
    let mut table = || -> Result<XcfOffsets<'_>, XcfHeaderError> {
        let start = position;
        loop {
            let end = position
                .checked_add(width)
                .ok_or(XcfHeaderError("object table length"))?;
            let raw = bytes
                .get(position..end)
                .ok_or(XcfHeaderError("truncated object table"))?;
            let value = offset_value(raw);
            if value == 0 {
                let data = &bytes[start..position];
                position = end;
                return Ok(XcfOffsets { data, width });
            }
            count += 1;
            if count > 4096 {
                return Err(XcfHeaderError("object count"));
            }
            if value >= bytes.len() as u64 {
                return Err(XcfHeaderError("object offset"));
            }
            if bytes[start..position]
                .chunks_exact(width)
                .any(|previous| offset_value(previous) == value)
            {
                return Err(XcfHeaderError("duplicate object offset"));
            }
            position = end;
        }
    };
    let layers = table()?;
    let channels = table()?;
    // Object records cannot overlap their enclosing header/property/tables.
    if layers
        .iter()
        .chain(channels.iter())
        .any(|offset| offset < position as u64)
    {
        return Err(XcfHeaderError("object offset"));
    }
    if channels
        .iter()
        .any(|offset| layers.iter().any(|layer| layer == offset))
    {
        return Err(XcfHeaderError("duplicate object offset"));
    }
    Ok(XcfObjectTables {
        layers,
        channels,
        end: position,
    })
}

#[derive(Debug, PartialEq, Eq)]
pub struct XcfProperty<'a> {
    pub kind: u32,
    pub payload: &'a [u8],
}

/// Borrowed, bounded image-property iterator. Errors terminate iteration.
#[derive(Debug)]
pub struct XcfProperties<'a> {
    bytes: &'a [u8],
    position: usize,
    count: usize,
    done: bool,
}

pub fn xcf_properties(bytes: &[u8]) -> Result<XcfProperties<'_>, XcfHeaderError> {
    let header = parse_xcf_header(bytes)?;
    Ok(XcfProperties {
        bytes,
        position: header.properties_offset,
        count: 0,
        done: false,
    })
}

impl XcfProperties<'_> {
    /// Offset after consumed properties; meaningful for object-table parsing
    /// only after the end marker has been consumed successfully.
    pub fn position(&self) -> usize {
        self.position
    }
}

impl<'a> Iterator for XcfProperties<'a> {
    type Item = Result<XcfProperty<'a>, XcfHeaderError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let result = (|| {
            let header = self
                .bytes
                .get(
                    self.position
                        ..self
                            .position
                            .checked_add(8)
                            .ok_or(XcfHeaderError("property length"))?,
                )
                .ok_or(XcfHeaderError("truncated property"))?;
            let kind = u32::from_be_bytes(header[..4].try_into().unwrap());
            let declared = u32::from_be_bytes(header[4..].try_into().unwrap()) as usize;
            self.position += 8;
            if kind == 0 {
                if declared != 0 {
                    return Err(XcfHeaderError("end property length"));
                }
                self.done = true;
                return Ok(None);
            }
            self.count += 1;
            if self.count > 4096 {
                return Err(XcfHeaderError("property count"));
            }
            let length = if kind == 1 {
                let count = u32::from_be_bytes(
                    self.bytes
                        .get(self.position..self.position + 4)
                        .ok_or(XcfHeaderError("truncated palette"))?
                        .try_into()
                        .unwrap(),
                ) as usize;
                if count > 256 {
                    return Err(XcfHeaderError("palette count"));
                }
                let actual = count * 3 + 4;
                // Historic GIMP writers incorrectly declared n+4, but wrote 3n+4.
                if declared != actual && declared != count + 4 {
                    return Err(XcfHeaderError("palette length"));
                }
                actual
            } else {
                declared
            };
            let end = self
                .position
                .checked_add(length)
                .ok_or(XcfHeaderError("property length"))?;
            let payload = self
                .bytes
                .get(self.position..end)
                .ok_or(XcfHeaderError("truncated property"))?;
            self.position = end;
            Ok(Some(XcfProperty { kind, payload }))
        })();
        match result {
            Ok(Some(property)) => Some(Ok(property)),
            Ok(None) => None,
            Err(error) => {
                self.done = true;
                Some(Err(error))
            }
        }
    }
}

pub fn parse_xcf_header(bytes: &[u8]) -> Result<XcfHeader, XcfHeaderError> {
    let bad = XcfHeaderError;
    if bytes.get(..9) != Some(b"gimp xcf ") || bytes.get(13) != Some(&0) {
        return Err(bad("signature"));
    }
    let version = match bytes.get(9..13) {
        Some(b"file") => 0,
        Some([b'v', a, b, c]) if [a, b, c].iter().all(|value| value.is_ascii_digit()) => {
            u16::from(*a - b'0') * 100 + u16::from(*b - b'0') * 10 + u16::from(*c - b'0')
        }
        _ => return Err(bad("version")),
    };
    if version > 23 {
        return Err(bad("version"));
    }
    let word = |offset| -> Result<u32, XcfHeaderError> {
        let data: [u8; 4] = bytes
            .get(offset..offset + 4)
            .ok_or(bad("truncated header"))?
            .try_into()
            .unwrap();
        Ok(u32::from_be_bytes(data))
    };
    let width = word(14)?;
    let height = word(18)?;
    let base_type = word(22)?;
    if base_type > 2 {
        return Err(bad("color mode"));
    }
    let precision = if version >= 4 { word(26)? } else { 150 };
    let sample_bytes = match (version, precision) {
        (0..=3, 150) | (4, 0) => 1,
        (4, 1 | 3) => 2,
        (4, 2 | 4) => 4,
        (5..=6, 100 | 150) => 1,
        (5..=6, 200 | 250 | 400 | 450) => 2,
        (5..=6, 300 | 350 | 500 | 550) => 4,
        (7..=23, 100 | 150 | 175) => 1,
        (7..=23, 200 | 250 | 275 | 500 | 550 | 575) => 2,
        (7..=23, 300 | 350 | 375 | 600 | 650 | 675) => 4,
        (7..=23, 700 | 750 | 775) => 8,
        _ => return Err(bad("precision")),
    };
    // Bound a possible four-component full-canvas working plane before any
    // pixel allocation. This is a parser admission cap, not total RSS accounting.
    let plane = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4 * u64::from(sample_bytes)))
        .ok_or(bad("canvas size"))?;
    if width == 0 || height == 0 || plane > crate::raster::MAX_RASTER_BYTES {
        return Err(bad("canvas size"));
    }
    Ok(XcfHeader {
        version,
        width,
        height,
        base_type,
        precision,
        sample_bytes,
        offset_bytes: if version >= 11 { 8 } else { 4 },
        properties_offset: if version >= 4 { 30 } else { 26 },
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn valid_indexed_v1_files_preserve_native_palette_and_bounded_rgba() {
        for (source, kind, native, rgba) in [
            (
                include_bytes!("../../../tests/fixtures/xcf/indexed-v1-kind-4.xcf").as_slice(),
                4,
                &[1, 0][..],
                &[0, 255, 0, 255, 255, 0, 0, 255][..],
            ),
            (
                include_bytes!("../../../tests/fixtures/xcf/indexed-v1-kind-5.xcf").as_slice(),
                5,
                &[1, 127, 0, 128][..],
                &[0, 255, 0, 0, 255, 0, 0, 255][..],
            ),
        ] {
            let header = parse_xcf_header(source).unwrap();
            assert_eq!((header.version, header.base_type), (1, 2));
            let tables = parse_xcf_object_tables(source).unwrap();
            let layer = parse_xcf_layer(source, tables.layers.iter().next().unwrap()).unwrap();
            assert_eq!(layer.kind, kind);
            let palette = xcf_palette(source).unwrap().unwrap();
            assert_eq!(palette, &[[255, 0, 0], [0, 255, 0]]);
            let level = parse_xcf_hierarchy(
                source,
                layer.hierarchy_offset,
                2,
                1,
                if kind == 4 { 1 } else { 2 },
            )
            .unwrap();
            // Borrowed fixture bytes are outside this pixel-buffer budget.
            let root = rrrah_core::MemoryBudget::new(native.len() as u64 + 8);
            let pixels = decode_xcf_level(source, &level, 0, &root, || false).unwrap();
            assert_eq!(&*pixels, native);
            assert_eq!(root.used(), native.len() as u64);
            let too_small = root.child(7);
            assert!(matches!(
                prepare_xcf_legacy_rgba8(pixels.clone(), kind, palette, &too_small, || false),
                Err(XcfPixelError::Memory(_))
            ));
            assert_eq!(too_small.used(), 0);
            assert_eq!(root.used(), native.len() as u64);
            let output = root.child(8);
            assert!(matches!(
                prepare_xcf_legacy_rgba8(pixels.clone(), kind, palette, &output, || true),
                Err(XcfPixelError::Cancelled)
            ));
            assert_eq!(output.peak(), 0);
            let expanded = prepare_xcf_legacy_rgba8(pixels, kind, palette, &output, || false).unwrap();
            assert_eq!(&*expanded, rgba);
            assert_eq!(root.used(), 8);
            let held = expanded.clone();
            drop(expanded);
            assert_eq!(root.used(), 8);
            drop(held);
            assert_eq!(root.used(), 0);
            assert_eq!(output.used(), 0);
        }
    }
    use super::*;
    fn header(version: u16, precision: u32, color: u32, width: u32, height: u32) -> Vec<u8> {
        let tag = if version == 0 {
            "file".into()
        } else {
            format!("v{version:03}")
        };
        let mut bytes = format!("gimp xcf {tag}\0").into_bytes();
        for value in [width, height, color] {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        if version >= 4 {
            bytes.extend_from_slice(&precision.to_be_bytes());
        }
        bytes
    }

    #[test]
    fn managed_level_assembly_keeps_edge_rows_and_rolls_back_failures() {
        let mut bytes = header(0, 150, 0, 65, 65);
        let hierarchy = bytes.len() as u64;
        let level_offset = bytes.len() + 20;
        let payload = level_offset + 8 + 20;
        for word in [65u32, 65, 3, level_offset as u32, 0, 65, 65] {
            bytes.extend_from_slice(&word.to_be_bytes());
        }
        let mut tiles = Vec::new();
        let mut offsets = Vec::new();
        for (x, y, w, h) in [(0, 0, 64, 64), (64, 0, 1, 64), (0, 64, 64, 1), (64, 64, 1, 1)] {
            offsets.push((payload + tiles.len()) as u32);
            for row in y..y + h {
                for col in x..x + w {
                    tiles.extend_from_slice(&[col as u8, row as u8, (col ^ row) as u8]);
                }
            }
        }
        for offset in offsets.into_iter().chain([0]) {
            bytes.extend_from_slice(&offset.to_be_bytes());
        }
        bytes.extend_from_slice(&tiles);
        let level = parse_xcf_hierarchy(&bytes, hierarchy, 65, 65, 3).unwrap();
        let budget = rrrah_core::MemoryBudget::new(32768);
        let pixels = decode_xcf_level(&bytes, &level, 0, &budget, || false).unwrap();
        for y in 0..65 {
            for x in 0..65 {
                let index = (y * 65 + x) * 3;
                assert_eq!(&pixels[index..index + 3], &[x as u8, y as u8, (x ^ y) as u8]);
            }
        }
        assert_eq!(budget.used(), 65 * 65 * 3);
        let retained = pixels.clone();
        drop(pixels);
        assert_eq!(budget.used(), 65 * 65 * 3);
        drop(retained);
        assert_eq!(budget.used(), 0);
        let small = rrrah_core::MemoryBudget::new(65 * 65 * 3);
        assert!(matches!(
            decode_xcf_level(&bytes, &level, 0, &small, || false),
            Err(XcfPixelError::Memory(_))
        ));
        assert_eq!(small.used(), 0);
        let mut polls = 0;
        assert!(matches!(
            decode_xcf_level(&bytes, &level, 0, &budget, || {
                polls += 1;
                polls == 3
            }),
            Err(XcfPixelError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
        assert!(decode_xcf_level(&bytes[..bytes.len() - 1], &level, 0, &budget, || false).is_err());
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn hierarchy_checks_main_level_and_edge_tile_tables() {
        for version in [0, 11] {
            let mut bytes = header(version, 150, 0, 65, 1);
            let hierarchy = bytes.len() as u64;
            let size = if version == 0 { 4 } else { 8 };
            let level = bytes.len() + 12 + size * 2;
            let payload = level + 8 + size * 3;
            for value in [65u32, 1, 3] {
                bytes.extend_from_slice(&value.to_be_bytes());
            }
            let pointer = |bytes: &mut Vec<u8>, value: u64| {
                if size == 4 {
                    bytes.extend_from_slice(&(value as u32).to_be_bytes());
                } else {
                    bytes.extend_from_slice(&value.to_be_bytes());
                }
            };
            pointer(&mut bytes, level as u64);
            pointer(&mut bytes, 0);
            bytes.extend_from_slice(&65u32.to_be_bytes());
            bytes.extend_from_slice(&1u32.to_be_bytes());
            for value in [payload as u64, (payload + 192) as u64, 0] {
                pointer(&mut bytes, value);
            }
            bytes.extend_from_slice(&[73; 195]);
            let parsed = parse_xcf_hierarchy(&bytes, hierarchy, 65, 1, 3).unwrap();
            assert_eq!(
                parsed.tiles.iter().collect::<Vec<_>>(),
                [payload as u64, (payload + 192) as u64]
            );
            assert_eq!(parsed.end, payload);
            let mut overlapping = bytes.clone();
            overlapping[level + 8 + size..level + 8 + size * 2]
                .copy_from_slice(&(payload as u64).to_be_bytes()[8 - size..]);
            assert!(matches!(
                parse_xcf_hierarchy(&overlapping, hierarchy, 65, 1, 3),
                Err(XcfHeaderError("tile offset order"))
            ));
            let mut short_first = bytes.clone();
            short_first[level + 8 + size..level + 8 + size * 2]
                .copy_from_slice(&((payload + 191) as u64).to_be_bytes()[8 - size..]);
            let short_level = parse_xcf_hierarchy(&short_first, hierarchy, 65, 1, 3).unwrap();
            let budget = rrrah_core::MemoryBudget::new(1024);
            assert!(decode_xcf_level(&short_first, &short_level, 0, &budget, || false).is_err());
            assert_eq!(budget.used(), 0);
            assert!(parse_xcf_hierarchy(&bytes, hierarchy, 64, 1, 3).is_err());
            assert!(parse_xcf_hierarchy(&bytes, hierarchy, 65, 1, 4).is_err());
            let mut corrupt = bytes.clone();
            corrupt[level..level + 4].copy_from_slice(&64u32.to_be_bytes());
            assert!(matches!(
                parse_xcf_hierarchy(&corrupt, hierarchy, 65, 1, 3),
                Err(XcfHeaderError("level dimensions"))
            ));
            for end in 0..payload + 193 {
                assert!(parse_xcf_hierarchy(&bytes[..end], hierarchy, 65, 1, 3).is_err());
            }
        }
    }

    #[test]
    fn tile_codecs_match_interleaved_pixels_and_leave_source_untouched() {
        use std::io::Write;
        let expected = [10, 20, 30, 255, 11, 21, 31, 255];
        let rle = [254, 10, 11, 254, 20, 21, 254, 30, 31, 1, 255];
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&expected).unwrap();
        let zipped = encoder.finish().unwrap();
        for (compression, source) in [
            (0, expected.as_slice()),
            (1, rle.as_slice()),
            (2, zipped.as_slice()),
        ] {
            let mut with_tail = source.to_vec();
            with_tail.extend_from_slice(&[42; 8]);
            let original = with_tail.clone();
            let mut output = [0; 8];
            assert_eq!(
                decode_xcf_tile(compression, &with_tail, 2, 1, 4, &mut output).unwrap(),
                source.len()
            );
            assert_eq!(output, expected);
            assert_eq!(with_tail, original);
            for end in 0..source.len() {
                assert!(decode_xcf_tile(compression, &source[..end], 2, 1, 4, &mut output).is_err());
            }
        }
    }

    #[test]
    fn long_rle_runs_and_invalid_lengths_are_bounded() {
        let mut output = [0; 256];
        assert_eq!(
            decode_xcf_tile(1, &[127, 1, 0, 73], 64, 4, 1, &mut output).unwrap(),
            4
        );
        assert!(output.iter().all(|value| *value == 73));
        let mut literals = vec![128, 1, 0];
        literals.extend(0..=255);
        assert_eq!(decode_xcf_tile(1, &literals, 64, 4, 1, &mut output).unwrap(), 259);
        assert!(output.iter().enumerate().all(|(i, value)| *value == i as u8));
        assert!(decode_xcf_tile(1, &[127, 1, 1, 73], 64, 4, 1, &mut output).is_err());
        assert!(decode_xcf_tile(0, &[0; 256], 65, 4, 1, &mut output).is_err());
        assert!(decode_xcf_tile(3, &[0; 256], 64, 4, 1, &mut output).is_err());
        let zeros = [128, 0, 0].repeat(1000);
        assert_eq!(
            decode_xcf_tile(1, &zeros, 64, 4, 1, &mut output),
            Err(XcfHeaderError("RLE work limit"))
        );
    }

    #[test]
    fn embedded_icc_is_borrowed_and_invalid_parasites_never_become_no_profile() {
        let mut icc = vec![0; 128];
        icc[..4].copy_from_slice(&128u32.to_be_bytes());
        icc[36..40].copy_from_slice(b"acsp");
        let fixture = |profiles: &[Vec<u8>]| {
            let mut bytes = header(0, 150, 0, 1, 1);
            let mut payload = Vec::new();
            for profile in profiles {
                payload.extend_from_slice(&12u32.to_be_bytes());
                payload.extend_from_slice(b"icc-profile\0");
                payload.extend_from_slice(&1u32.to_be_bytes());
                payload.extend_from_slice(&(profile.len() as u32).to_be_bytes());
                payload.extend_from_slice(profile);
            }
            bytes.extend_from_slice(&21u32.to_be_bytes());
            bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            bytes.extend_from_slice(&payload);
            bytes.extend_from_slice(&[0; 8]);
            bytes
        };
        let bytes = fixture(&[icc.clone()]);
        let found = xcf_icc_profile(&bytes).unwrap().unwrap();
        assert_eq!(found, icc);
        assert_eq!(found.as_ptr(), bytes[58..].as_ptr());
        assert!(xcf_icc_profile(&fixture(&[icc.clone(), icc.clone()])).is_err());
        for end in 0..bytes.len() {
            assert!(xcf_icc_profile(&bytes[..end]).is_err());
        }
        let mut invalid = icc.clone();
        invalid[36] = 0;
        assert!(xcf_icc_profile(&fixture(&[invalid])).is_err());
        assert!(xcf_icc_profile(&fixture(&[vec![]])).is_err());
        assert_eq!(xcf_icc_profile(&fixture(&[])).unwrap(), None);
        let mut invalid = bytes;
        invalid[49] = 1;
        assert!(xcf_icc_profile(&invalid).is_err());
    }

    #[test]
    fn managed_tree_resolves_groups_and_rolls_back_missing_parent_or_cancellation() {
        let fixture = |parent: u32, group: bool| {
            let mut bytes = header(0, 150, 0, 1, 1);
            bytes.extend_from_slice(&[0; 8]);
            let table = bytes.len();
            bytes.extend_from_slice(&[0; 16]);
            for index in 0..2 {
                let offset = bytes.len() as u32;
                bytes[table + index * 4..table + index * 4 + 4].copy_from_slice(&offset.to_be_bytes());
                for word in [1u32, 1, 1, 0] {
                    bytes.extend_from_slice(&word.to_be_bytes());
                }
                if index == 0 && group {
                    bytes.extend_from_slice(&29u32.to_be_bytes());
                    bytes.extend_from_slice(&0u32.to_be_bytes());
                } else if index == 1 {
                    for word in [30u32, 8, parent, 0] {
                        bytes.extend_from_slice(&word.to_be_bytes());
                    }
                }
                bytes.extend_from_slice(&[0; 16]);
            }
            bytes
        };
        let budget = rrrah_core::MemoryBudget::new(1024);
        let bytes = fixture(0, true);
        let tree = parse_xcf_layer_tree(&bytes, &budget, || false).unwrap();
        assert_eq!(tree.len(), 2);
        assert_eq!((tree[0].parent, tree[0].is_group), (None, true));
        assert_eq!((tree[1].parent, tree[1].sibling), (Some(0), 0));
        drop(tree);
        assert_eq!(budget.used(), 0);
        for bytes in [fixture(1, true), fixture(0, false)] {
            assert!(parse_xcf_layer_tree(&bytes, &budget, || false).is_err());
            assert_eq!(budget.used(), 0);
        }
        let mut polls = 0;
        assert!(matches!(
            parse_xcf_layer_tree(&bytes, &budget, || {
                polls += 1;
                polls == 3
            }),
            Err(XcfPixelError::Cancelled)
        ));
        assert_eq!(budget.used(), 0);
        let small = rrrah_core::MemoryBudget::new(1);
        assert!(parse_xcf_layer_tree(&bytes, &small, || false).is_err());
        assert_eq!(small.peak(), 0);
    }

    #[test]
    fn premultiplied_group_scales_color_once_and_matches_straight_source_over() {
        let root = rrrah_core::MemoryBudget::new(32);
        let mut group = root.try_buffer(4, 0.0f32).unwrap();
        let child = [4.0, -2.0, 0.0, 0.5];
        let child_attributes = XcfLayerAttributes {
            visible: true,
            opacity: 1.0,
            offset_x: 0,
            offset_y: 0,
            apply_mask: false,
        };
        composite_xcf_normal_rgba32f(&mut group, 1, 1, &child, 1, 1, None, child_attributes, || false)
            .unwrap();
        assert_eq!(&*group, &[2.0, -1.0, 0.0, 0.5]);
        let mut canvas = root.try_buffer(4, 0.0f32).unwrap();
        canvas.copy_from_slice(&[0.0, 0.0, 2.0, 1.0]);
        let group_attributes = XcfLayerAttributes {
            opacity: 0.5,
            apply_mask: true,
            ..child_attributes
        };
        composite_xcf_normal_premultiplied_rgba32f(
            &mut canvas,
            1,
            1,
            &group,
            1,
            1,
            Some(&[0.5]),
            group_attributes,
            || false,
        )
        .unwrap();
        assert_eq!(&*canvas, &[0.5, -0.25, 1.75, 1.0]);
        let mut reference = [0.0, 0.0, 2.0, 1.0];
        composite_xcf_normal_rgba32f(
            &mut reference,
            1,
            1,
            &child,
            1,
            1,
            Some(&[0.5]),
            group_attributes,
            || false,
        )
        .unwrap();
        assert_eq!(&*canvas, &reference);
        assert_eq!(root.peak(), 32);
        drop(group);
        drop(canvas);
        assert_eq!(root.used(), 0);
    }

    #[test]
    fn float_composite_preserves_hdr_and_validates_all_samples_before_writes() {
        let attributes = XcfLayerAttributes {
            visible: true,
            opacity: 1.0,
            offset_x: 0,
            offset_y: 0,
            apply_mask: true,
        };
        let root = rrrah_core::MemoryBudget::new(64);
        let mut canvas = root.try_buffer(8, 0.0f32).unwrap();
        canvas.copy_from_slice(&[0.0, 0.0, 2.0, 1.0, 0.0, 0.0, 0.0, 0.0]);
        let mut source = root.try_buffer(8, 0.0f32).unwrap();
        source.copy_from_slice(&[4.0, -2.0, 0.0, 0.5, 8.0, 0.0, 0.0, 1.0]);
        composite_xcf_normal_rgba32f(
            &mut canvas,
            2,
            1,
            &source,
            2,
            1,
            Some(&[1.0, 0.25]),
            attributes,
            || false,
        )
        .unwrap();
        assert_eq!(&*canvas, &[2.0, -1.0, 1.0, 1.0, 2.0, 0.0, 0.0, 0.25]);
        assert_eq!(root.peak(), 64);
        let before = canvas.to_vec();
        for invalid in [f32::NAN, f32::INFINITY, -0.01, 1.01] {
            source[7] = invalid;
            assert!(
                composite_xcf_normal_rgba32f(
                    &mut canvas,
                    2,
                    1,
                    &source,
                    2,
                    1,
                    Some(&[1.0, 1.0]),
                    attributes,
                    || false
                )
                .is_err()
            );
            assert_eq!(&*canvas, before.as_slice());
            source[7] = 1.0;
            assert!(
                composite_xcf_normal_rgba32f(
                    &mut canvas,
                    2,
                    1,
                    &source,
                    2,
                    1,
                    Some(&[1.0, invalid]),
                    attributes,
                    || false
                )
                .is_err()
            );
            assert_eq!(&*canvas, before.as_slice());
        }
        let mut polls = 0;
        assert!(matches!(
            composite_xcf_normal_rgba32f(
                &mut canvas,
                2,
                1,
                &source,
                2,
                1,
                Some(&[1.0, 1.0]),
                attributes,
                || {
                    polls += 1;
                    polls == 3
                }
            ),
            Err(XcfPixelError::Cancelled)
        ));
        assert_eq!(&*canvas, before.as_slice());
        drop(source);
        drop(canvas);
        assert_eq!(root.used(), 0);
    }

    #[test]
    fn every_half_pattern_matches_independent_python_ieee_oracle_or_is_refused() {
        use std::io::Read;
        let compressed = include_bytes!("../../../tests/fixtures/xcf/finite-half-f32-be.zlib");
        let mut expected = Vec::new();
        flate2::read::ZlibDecoder::new(compressed.as_slice())
            .read_to_end(&mut expected)
            .unwrap();
        assert_eq!(expected.len(), 63488 * 4);
        let source = (0..=u16::MAX)
            .filter(|bits| bits & 0x7c00 != 0x7c00)
            .flat_map(u16::to_be_bytes)
            .collect::<Vec<_>>();
        let header = parse_xcf_header(&header(12, 500, 0, 1, 1)).unwrap();
        let root = rrrah_core::MemoryBudget::new(expected.len() as u64);
        let output = decode_xcf_samples(&source, header, &root, || false).unwrap();
        for (index, (value, word)) in output.iter().zip(expected.chunks_exact(4)).enumerate() {
            assert_eq!(
                value.to_bits(),
                u32::from_be_bytes(word.try_into().unwrap()),
                "finite sample {index}"
            );
        }
        drop(output);
        assert_eq!(root.used(), 0);
        for bits in (0..=u16::MAX).filter(|bits| bits & 0x7c00 == 0x7c00) {
            assert!(decode_xcf_samples(&bits.to_be_bytes(), header, &root, || false).is_err());
            assert_eq!(root.used(), 0);
        }
    }

    #[test]
    fn high_precision_samples_preserve_hdr_and_reject_ambiguous_or_nonfinite_values() {
        let root = rrrah_core::MemoryBudget::new(16);
        for (precision, samples, expected) in [
            (150, vec![0, 255], vec![0.0, 1.0]),
            (250, vec![0, 0, 255, 255], vec![0.0, 1.0]),
            (
                350,
                [0u32, u32::MAX].into_iter().flat_map(u32::to_be_bytes).collect(),
                vec![0.0, 1.0],
            ),
            (
                550,
                [0x4400u16, 0xb800]
                    .into_iter()
                    .flat_map(u16::to_be_bytes)
                    .collect(),
                vec![4.0, -0.5],
            ),
            (
                650,
                [8.0f32, -0.25].into_iter().flat_map(f32::to_be_bytes).collect(),
                vec![8.0, -0.25],
            ),
            (
                750,
                [16.0f64, 0.5].into_iter().flat_map(f64::to_be_bytes).collect(),
                vec![16.0, 0.5],
            ),
        ] {
            let parsed = parse_xcf_header(&header(12, precision, 0, 1, 1)).unwrap();
            let output = decode_xcf_samples(&samples, parsed, &root, || false).unwrap();
            assert_eq!(&*output, expected.as_slice());
            assert_eq!(root.used(), 8);
            drop(output);
            assert_eq!(root.used(), 0);
            if parsed.sample_bytes > 1 {
                assert!(
                    decode_xcf_samples(
                        &samples,
                        XcfHeader {
                            version: 11,
                            ..parsed
                        },
                        &root,
                        || false
                    )
                    .is_err()
                );
                assert!(decode_xcf_samples(&samples[..samples.len() - 1], parsed, &root, || false).is_err());
            }
        }
        let parsed = parse_xcf_header(&header(12, 600, 0, 1, 1)).unwrap();
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(decode_xcf_samples(&value.to_be_bytes(), parsed, &root, || false).is_err());
            assert_eq!(root.used(), 0);
        }
        let mut polls = 0;
        assert!(matches!(
            decode_xcf_samples(&1.0f32.to_be_bytes(), parsed, &root, || {
                polls += 1;
                polls == 3
            }),
            Err(XcfPixelError::Cancelled)
        ));
        assert_eq!(root.used(), 0);
        let small = rrrah_core::MemoryBudget::new(3);
        assert!(decode_xcf_samples(&1.0f32.to_be_bytes(), parsed, &small, || false).is_err());
        assert_eq!(small.peak(), 0);
    }

    #[test]
    fn group_membership_borrows_paths_and_rejects_duplicate_or_unbounded_records() {
        let check = |entries: &[(u32, Vec<u8>)]| {
            let mut bytes = Vec::new();
            for (kind, payload) in entries {
                bytes.extend_from_slice(&kind.to_be_bytes());
                bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
                bytes.extend_from_slice(payload);
            }
            bytes.extend_from_slice(&[0; 8]);
            let mut layer = XcfLayer {
                width: 1,
                height: 1,
                kind: 1,
                name: b"",
                properties: XcfProperties {
                    bytes: &bytes,
                    position: 0,
                    count: 0,
                    done: false,
                },
                hierarchy_offset: 0,
                mask_offset: 0,
                effects: XcfOffsets { data: &[], width: 4 },
                end: bytes.len(),
                properties_start: 0,
            };
            for property in layer.properties.by_ref() {
                property.unwrap();
            }
            layer.group().map(|group| {
                if !group.item_path.is_empty() {
                    let pointer = group.item_path.as_ptr() as usize;
                    let start = bytes.as_ptr() as usize;
                    assert!(pointer >= start && pointer + group.item_path.len() <= start + bytes.len());
                }
                (group.is_group, group.item_path().collect::<Vec<_>>())
            })
        };
        assert_eq!(check(&[]).unwrap(), (false, vec![]));
        let path = [2u32, 0, 3]
            .into_iter()
            .flat_map(u32::to_be_bytes)
            .collect::<Vec<_>>();
        assert_eq!(
            check(&[(29, vec![]), (30, path.clone())]).unwrap(),
            (true, vec![2, 0, 3])
        );
        assert_eq!(check(&[(30, path.clone())]).unwrap(), (false, vec![2, 0, 3]));
        for entries in [
            vec![(29, vec![0])],
            vec![(29, vec![]), (29, vec![])],
            vec![(30, vec![0; 3])],
            vec![(30, vec![0; 1028])],
            vec![(30, path.clone()), (30, path)],
        ] {
            assert!(check(&entries).is_err());
        }
    }

    #[test]
    fn rgba_preparation_transfers_shared_owner_at_full_ram_capacity() {
        let root = rrrah_core::MemoryBudget::new(4);
        let mut pixels = root.try_buffer(4, 0u8).unwrap();
        pixels.copy_from_slice(&[10, 20, 30, 40]);
        let source = pixels.freeze();
        let prepared = prepare_xcf_legacy_rgba8(source.clone(), 1, &[], &root, || false).unwrap();
        assert!(source.ptr_eq(&prepared));
        assert_eq!(root.used(), 4);
        assert_eq!(root.peak(), 4);
        drop(source);
        assert_eq!(&*prepared, &[10, 20, 30, 40]);
        assert_eq!(root.used(), 4);
        assert!(matches!(
            prepare_xcf_legacy_rgba8(prepared, 1, &[], &root, || true),
            Err(XcfPixelError::Cancelled)
        ));
        assert_eq!(root.used(), 0);

        let root = rrrah_core::MemoryBudget::new(6);
        let source = root.try_buffer(3, 7u8).unwrap().freeze();
        assert!(prepare_xcf_legacy_rgba8(source.clone(), 0, &[], &root, || false).is_err());
        assert_eq!(root.used(), 3);
        assert_eq!(root.peak(), 3);
        drop(source);
        assert_eq!(root.used(), 0);

        let root = rrrah_core::MemoryBudget::new(7);
        let source = root.try_buffer(3, 7u8).unwrap().freeze();
        let prepared = prepare_xcf_legacy_rgba8(source, 0, &[], &root, || false).unwrap();
        assert_eq!(&*prepared, &[7, 7, 7, 255]);
        assert_eq!(root.peak(), 7);
        assert_eq!(root.used(), 4);
        drop(prepared);
        assert_eq!(root.used(), 0);
    }

    #[test]
    fn legacy_u8_expansion_covers_modes_palette_alpha_and_budget_rollback() {
        let root = rrrah_core::MemoryBudget::new(16);
        let palette = [[10, 20, 30], [40, 50, 60]];
        for (kind, source, expected) in [
            (0, vec![1, 2, 3], vec![1, 2, 3, 255]),
            (1, vec![1, 2, 3, 4], vec![1, 2, 3, 4]),
            (2, vec![7], vec![7, 7, 7, 255]),
            (3, vec![7, 9], vec![7, 7, 7, 9]),
            (4, vec![1], vec![40, 50, 60, 255]),
            (5, vec![0, 127, 1, 128], vec![10, 20, 30, 0, 40, 50, 60, 255]),
        ] {
            let output = expand_xcf_legacy_u8_pixels(&source, kind, &palette, &root, || false).unwrap();
            assert_eq!(&*output, expected.as_slice());
            assert_eq!(root.used(), expected.len() as u64);
            drop(output);
            assert_eq!(root.used(), 0);
        }
        assert!(expand_xcf_legacy_u8_pixels(&[2], 4, &palette, &root, || false).is_err());
        assert_eq!(root.used(), 0);
        let small = rrrah_core::MemoryBudget::new(3);
        assert!(expand_xcf_legacy_u8_pixels(&[1], 2, &[], &small, || false).is_err());
        assert_eq!(small.peak(), 0);
        let mut polls = 0;
        assert!(matches!(
            expand_xcf_legacy_u8_pixels(&[7], 2, &[], &root, || {
                polls += 1;
                polls == 3
            }),
            Err(XcfPixelError::Cancelled)
        ));
        assert_eq!(root.used(), 0);
        assert!(expand_xcf_legacy_u8_pixels(&[1, 2], 0, &[], &root, || false).is_err());
        assert!(expand_xcf_legacy_u8_pixels(&[0], 6, &[], &root, || false).is_err());
    }

    #[test]
    fn normal_composite_preserves_alpha_mask_clipping_and_input_validation() {
        let attrs = XcfLayerAttributes {
            visible: true,
            opacity: 0.5,
            offset_x: -1,
            offset_y: 0,
            apply_mask: true,
        };
        let source = [255, 0, 0, 255, 255, 0, 0, 255];
        let mut canvas = [0.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 1.0];
        composite_xcf_normal_rgba8(&mut canvas, 2, 1, &source, 2, 1, Some(&[0, 255]), attrs, || false)
            .unwrap();
        assert_eq!(canvas, [0.5, 0.0, 0.5, 1.0, 0.0, 1.0, 0.0, 1.0]);
        let mut transparent = [0.0; 8];
        composite_xcf_normal_rgba8(
            &mut transparent,
            2,
            1,
            &source,
            2,
            1,
            Some(&[0, 255]),
            XcfLayerAttributes { offset_x: 0, ..attrs },
            || false,
        )
        .unwrap();
        assert_eq!(transparent, [0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.5]);
        let before = canvas;
        for shift in [i32::MIN, i32::MAX] {
            composite_xcf_normal_rgba8(
                &mut canvas,
                2,
                1,
                &source,
                2,
                1,
                Some(&[255, 255]),
                XcfLayerAttributes {
                    offset_x: shift,
                    ..attrs
                },
                || false,
            )
            .unwrap();
            assert_eq!(canvas, before);
        }
        for mask in [None, Some(&[0][..]), Some(&[0, 0, 0][..])] {
            assert!(
                composite_xcf_normal_rgba8(&mut canvas, 2, 1, &source, 2, 1, mask, attrs, || false).is_err()
            );
            assert_eq!(canvas, before);
        }
        assert!(matches!(
            composite_xcf_normal_rgba8(&mut canvas, 2, 1, &source, 2, 1, Some(&[255, 255]), attrs, || {
                true
            }),
            Err(XcfPixelError::Cancelled)
        ));
        assert_eq!(canvas, before);
        assert!(
            composite_xcf_normal_rgba8(
                &mut canvas,
                2,
                1,
                &source,
                2,
                1,
                Some(&[255, 255]),
                XcfLayerAttributes {
                    opacity: f32::NAN,
                    ..attrs
                },
                || false
            )
            .is_err()
        );
        assert_eq!(canvas, before);
    }

    #[test]
    fn composition_attributes_preserve_auto_and_unknown_values_without_fallback() {
        let decode = |entries: &[(u32, Vec<u8>)]| {
            let mut bytes = Vec::new();
            for (kind, payload) in entries {
                bytes.extend_from_slice(&kind.to_be_bytes());
                bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
                bytes.extend_from_slice(payload);
            }
            bytes.extend_from_slice(&[0; 8]);
            let mut layer = XcfLayer {
                width: 1,
                height: 1,
                kind: 1,
                name: b"",
                properties: XcfProperties {
                    bytes: &bytes,
                    position: 0,
                    count: 0,
                    done: false,
                },
                hierarchy_offset: 0,
                mask_offset: 0,
                effects: XcfOffsets { data: &[], width: 4 },
                end: bytes.len(),
                properties_start: 0,
            };
            for property in layer.properties.by_ref() {
                property.unwrap();
            }
            let first = layer.composition_attributes();
            if let Ok(value) = first {
                assert_eq!(layer.composition_attributes().unwrap(), value);
            }
            first
        };
        assert_eq!(
            decode(&[]).unwrap(),
            XcfCompositionAttributes {
                blend_mode: 0,
                composite_mode: None,
                composite_space: None,
                blend_space: None
            }
        );
        assert_eq!(
            decode(&[
                (7, 3u32.to_be_bytes().to_vec()),
                (35, (-2i32).to_be_bytes().to_vec()),
                (36, 4i32.to_be_bytes().to_vec()),
                (37, (-3i32).to_be_bytes().to_vec())
            ])
            .unwrap(),
            XcfCompositionAttributes {
                blend_mode: 3,
                composite_mode: Some(-2),
                composite_space: Some(4),
                blend_space: Some(-3)
            }
        );
        assert_eq!(
            decode(&[
                (7, u32::MAX.to_be_bytes().to_vec()),
                (35, i32::MIN.to_be_bytes().to_vec())
            ])
            .unwrap()
            .composite_mode,
            Some(i32::MIN)
        );
        for kind in [7, 35, 36, 37] {
            for length in [0, 3, 5, 8] {
                assert!(decode(&[(kind, vec![0; length])]).is_err());
            }
        }
    }

    #[test]
    fn layer_attributes_preserve_signed_offsets_and_float_opacity_precedence() {
        let attributes = |entries: &[(u32, Vec<u8>)], mask| {
            let mut bytes = Vec::new();
            for (kind, payload) in entries {
                bytes.extend_from_slice(&kind.to_be_bytes());
                bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
                bytes.extend_from_slice(payload);
            }
            bytes.extend_from_slice(&[0; 8]);
            let mut layer = XcfLayer {
                width: 1,
                height: 1,
                kind: 1,
                name: b"",
                properties: XcfProperties {
                    bytes: &bytes,
                    position: 0,
                    count: 0,
                    done: false,
                },
                hierarchy_offset: 0,
                mask_offset: mask,
                effects: XcfOffsets { data: &[], width: 4 },
                end: bytes.len(),
                properties_start: 0,
            };
            for property in layer.properties.by_ref() {
                property.unwrap();
            }
            layer.attributes()
        };
        assert_eq!(
            attributes(&[], 0).unwrap(),
            XcfLayerAttributes {
                visible: true,
                opacity: 1.0,
                offset_x: 0,
                offset_y: 0,
                apply_mask: false
            }
        );
        let mut offsets = (-12i32).to_be_bytes().to_vec();
        offsets.extend_from_slice(&i32::MAX.to_be_bytes());
        let props = [
            (33, 0.25f32.to_be_bytes().to_vec()),
            (6, 255u32.to_be_bytes().to_vec()),
            (15, offsets),
            (8, 0u32.to_be_bytes().to_vec()),
            (11, 1u32.to_be_bytes().to_vec()),
        ];
        assert_eq!(
            attributes(&props, 99).unwrap(),
            XcfLayerAttributes {
                visible: false,
                opacity: 0.25,
                offset_x: -12,
                offset_y: i32::MAX,
                apply_mask: true
            }
        );
        assert!(!attributes(&props, 0).unwrap().apply_mask);
        assert!(attributes(&[], 99).unwrap().apply_mask);
        for value in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
            assert!(attributes(&[(33, value.to_be_bytes().to_vec())], 0).is_err());
        }
        for kind in [6, 8, 11, 15, 33] {
            assert!(attributes(&[(kind, vec![0; 3])], 0).is_err());
        }
        assert!(attributes(&[(6, 256u32.to_be_bytes().to_vec())], 0).is_err());
        for kind in [8, 11] {
            assert!(attributes(&[(kind, 2u32.to_be_bytes().to_vec())], 0).is_err());
        }
    }

    #[test]
    fn empty_main_level_uses_one_null_pointer_and_no_tile_scratch() {
        for version in [0, 11] {
            let mut bytes = header(version, 150, 0, 65, 1);
            let hierarchy = bytes.len() as u64;
            let w = if version < 11 { 4 } else { 8 };
            let level = hierarchy + 12 + 2 * w as u64;
            for word in [65u32, 1, 3] {
                bytes.extend_from_slice(&word.to_be_bytes());
            }
            for pointer in [level, 0] {
                bytes.extend_from_slice(&pointer.to_be_bytes()[8 - w..]);
            }
            for word in [65u32, 1] {
                bytes.extend_from_slice(&word.to_be_bytes());
            }
            bytes.extend_from_slice(&vec![0; w]);
            let parsed = parse_xcf_hierarchy(&bytes, hierarchy, 65, 1, 3).unwrap();
            assert_eq!(parsed.end, bytes.len());
            assert_eq!(parsed.tiles.iter().count(), 0);
            let budget = rrrah_core::MemoryBudget::new(195);
            let pixels = decode_xcf_level(&bytes, &parsed, 1, &budget, || false).unwrap();
            assert!(pixels.iter().all(|v| *v == 0));
            assert_eq!(budget.peak(), 195);
            drop(pixels);
            assert_eq!(budget.used(), 0);
            let mut polls = 0;
            assert!(matches!(
                decode_xcf_level(&bytes, &parsed, 1, &budget, || {
                    polls += 1;
                    polls == 2
                }),
                Err(XcfPixelError::Cancelled)
            ));
            assert_eq!(budget.used(), 0);
            assert!(parse_xcf_hierarchy(&bytes[..bytes.len() - 1], hierarchy, 65, 1, 3).is_err());
        }
    }

    #[test]
    fn channel_hierarchy_decodes_under_shared_budget_and_releases_last_owner() {
        for version in [0, 11] {
            let mut bytes = header(version, 150, 0, 2, 2);
            let channel_offset = bytes.len() as u64;
            for word in [2u32, 2, 0] {
                bytes.extend_from_slice(&word.to_be_bytes());
            }
            bytes.extend_from_slice(&[0; 8]);
            let w = if version < 11 { 4 } else { 8 };
            let hierarchy = (bytes.len() + w) as u64;
            let level = hierarchy + 12 + 2 * w as u64;
            let tile = level + 8 + 2 * w as u64;
            bytes.extend_from_slice(&hierarchy.to_be_bytes()[8 - w..]);
            for word in [2u32, 2, 1] {
                bytes.extend_from_slice(&word.to_be_bytes());
            }
            for pointer in [level, 0] {
                bytes.extend_from_slice(&pointer.to_be_bytes()[8 - w..]);
            }
            for word in [2u32, 2] {
                bytes.extend_from_slice(&word.to_be_bytes());
            }
            for pointer in [tile, 0] {
                bytes.extend_from_slice(&pointer.to_be_bytes()[8 - w..]);
            }
            bytes.extend_from_slice(&[0, 64, 128, 255]);
            let root = rrrah_core::MemoryBudget::new(bytes.len() as u64 + 8);
            let mut source = root.try_buffer(bytes.len(), 0u8).unwrap();
            source.copy_from_slice(&bytes);
            let source = source.freeze();
            let channel = parse_xcf_channel(&source, channel_offset, 2, 2).unwrap();
            let level = parse_xcf_hierarchy(&source, channel.hierarchy_offset, 2, 2, 1).unwrap();
            let pixels = decode_xcf_level(&source, &level, 0, &root, || false).unwrap();
            assert_eq!(&*pixels, &[0, 64, 128, 255]);
            assert_eq!(root.used(), bytes.len() as u64 + 4);
            let retained = pixels.clone();
            drop(pixels);
            drop(source);
            assert_eq!(root.used(), 4);
            drop(retained);
            assert_eq!(root.used(), 0);
            assert_eq!(root.peak(), bytes.len() as u64 + 8);
        }
    }

    #[test]
    fn channel_records_validate_parent_dimensions_and_borrow_metadata() {
        for version in [0, 11, 23] {
            let mut bytes = header(version, 150, 0, 2, 2);
            let offset = bytes.len() as u64;
            for word in [2u32, 2, 5] {
                bytes.extend_from_slice(&word.to_be_bytes());
            }
            bytes.extend_from_slice(b"mask\0");
            bytes.extend_from_slice(&[0; 8]);
            let pointer_width = if version < 11 { 4 } else { 8 };
            let target = (bytes.len() + pointer_width) as u64;
            let pointer_start = bytes.len();
            bytes.extend_from_slice(&target.to_be_bytes()[8 - pointer_width..]);
            bytes.push(42);
            let channel = parse_xcf_channel(&bytes, offset, 2, 2).unwrap();
            assert_eq!(channel.name, b"mask");
            assert_eq!(channel.name.as_ptr(), bytes[offset as usize + 12..].as_ptr());
            assert_eq!(channel.hierarchy_offset, target);
            assert_eq!(channel.end, target as usize);
            assert_eq!(channel.properties.count(), 0);
            assert!(parse_xcf_channel(&bytes, offset, 1, 2).is_err());
            for end in 0..bytes.len() {
                assert!(parse_xcf_channel(&bytes[..end], offset, 2, 2).is_err());
            }
            for invalid in [0, 1, bytes.len() as u64, u64::MAX] {
                let mut broken = bytes.clone();
                broken[pointer_start..pointer_start + pointer_width]
                    .copy_from_slice(&invalid.to_be_bytes()[8 - pointer_width..]);
                assert!(parse_xcf_channel(&broken, offset, 2, 2).is_err());
            }
            let mut broken = bytes.clone();
            broken[offset as usize + 16] = 1;
            assert_eq!(
                parse_xcf_channel(&broken, offset, 2, 2).unwrap_err(),
                XcfHeaderError("channel name terminator")
            );
            let mut broken = bytes;
            broken[offset as usize + 8..offset as usize + 12].copy_from_slice(&u32::MAX.to_be_bytes());
            assert_eq!(
                parse_xcf_channel(&broken, offset, 2, 2).unwrap_err(),
                XcfHeaderError("channel name length")
            );
        }
    }

    #[test]
    fn layer_records_borrow_names_and_preserve_mask_references() {
        for version in [0, 11, 23] {
            let mut bytes = header(version, 150, 0, 2, 2);
            let offset = bytes.len() as u64;
            for word in [2u32, 2, 1, 5] {
                bytes.extend_from_slice(&word.to_be_bytes());
            }
            bytes.extend_from_slice(b"name\0");
            bytes.extend_from_slice(&[0; 8]);
            let offset_width = if version < 11 { 4 } else { 8 };
            let target =
                (bytes.len() + offset_width * 2 + if version >= 20 { offset_width } else { 0 }) as u64;
            for value in [target, target + 1] {
                if offset_width == 4 {
                    bytes.extend_from_slice(&(value as u32).to_be_bytes());
                } else {
                    bytes.extend_from_slice(&value.to_be_bytes());
                }
            }
            if version >= 20 {
                bytes.extend_from_slice(&[0; 8]);
            }
            bytes.extend_from_slice(&[42, 43]);
            let layer = parse_xcf_layer(&bytes, offset).unwrap();
            assert_eq!(layer.name, b"name");
            assert_eq!(layer.name.as_ptr(), bytes[offset as usize + 16..].as_ptr());
            assert_eq!((layer.hierarchy_offset, layer.mask_offset), (target, target + 1));
            assert_eq!(layer.end, target as usize);
            assert_eq!(layer.effects.iter().count(), 0);
            assert!(layer.properties.count() == 0);
            for end in 0..bytes.len() {
                assert!(parse_xcf_layer(&bytes[..end], offset).is_err());
            }
        }
    }

    #[test]
    fn excessive_layer_sizes_and_name_lengths_fail_before_payload_reads() {
        for (dimensions, reason) in [
            ([u32::MAX, u32::MAX, 1, 5], "layer size"),
            ([2, 2, 6, 5], "layer color mode"),
            ([2, 2, 1, u32::MAX], "layer name length"),
        ] {
            let mut bytes = header(0, 150, 0, 2, 2);
            let offset = bytes.len() as u64;
            for word in dimensions {
                bytes.extend_from_slice(&word.to_be_bytes());
            }
            assert!(matches!(parse_xcf_layer(&bytes, offset), Err(XcfHeaderError(found)) if found == reason));
        }
    }

    #[test]
    fn object_tables_are_borrowed_and_validate_both_offset_widths() {
        for version in [0, 11] {
            let mut bytes = header(version, 150, 0, 1, 1);
            bytes.extend_from_slice(&[0; 8]);
            let width = if version == 0 { 4 } else { 8 };
            let record = (bytes.len() + width * 4) as u64;
            for value in [record, 0, record + 1, 0] {
                if width == 4 {
                    bytes.extend_from_slice(&(value as u32).to_be_bytes());
                } else {
                    bytes.extend_from_slice(&value.to_be_bytes());
                }
            }
            bytes.extend_from_slice(&[1, 2]);
            let tables = parse_xcf_object_tables(&bytes).unwrap();
            assert_eq!(tables.layers.iter().collect::<Vec<_>>(), [record]);
            assert_eq!(tables.channels.iter().collect::<Vec<_>>(), [record + 1]);
            assert_eq!(tables.end, record as usize);
            for end in 0..bytes.len() {
                assert!(parse_xcf_object_tables(&bytes[..end]).is_err());
            }
        }
    }
    #[test]
    fn duplicate_and_overlapping_object_offsets_are_rejected() {
        for offsets in [
            [50u32, 50, 0, 0],
            [50, 0, 50, 0],
            [1, 0, 0, 0],
            [u32::MAX, 0, 0, 0],
        ] {
            let mut bytes = header(0, 150, 0, 1, 1);
            bytes.extend_from_slice(&[0; 8]);
            for offset in offsets {
                bytes.extend_from_slice(&offset.to_be_bytes());
            }
            bytes.extend_from_slice(&[0; 8]);
            assert!(parse_xcf_object_tables(&bytes).is_err());
        }
    }

    #[test]
    fn borrowed_properties_accept_historic_palette_size_and_stop_at_end() {
        for declared in [6u32, 10] {
            let mut bytes = header(0, 150, 2, 10, 20);
            bytes.extend_from_slice(&1u32.to_be_bytes());
            bytes.extend_from_slice(&declared.to_be_bytes());
            bytes.extend_from_slice(&2u32.to_be_bytes());
            bytes.extend_from_slice(&[255, 0, 0, 0, 255, 0]);
            bytes.extend_from_slice(&[0; 8]);
            let expected_end = bytes.len();
            bytes.extend_from_slice(&[42; 8]); // Object tables are not properties.
            let mut properties = xcf_properties(&bytes).unwrap();
            let property = properties.next().unwrap().unwrap();
            assert_eq!(property.kind, 1);
            assert_eq!(property.payload.len(), 10);
            assert_eq!(property.payload.as_ptr(), bytes[34..].as_ptr());
            let palette = xcf_palette(&bytes).unwrap().unwrap();
            assert_eq!(palette, &[[255, 0, 0], [0, 255, 0]]);
            assert_eq!(palette.as_ptr().cast::<u8>(), bytes[38..].as_ptr());
            let budget = rrrah_core::MemoryBudget::new(8);
            let expanded = expand_xcf_legacy_u8_pixels(&[1, 0], 4, palette, &budget, || false).unwrap();
            assert_eq!(&*expanded, &[0, 255, 0, 255, 255, 0, 0, 255]);
            drop(expanded);
            assert_eq!(budget.used(), 0);
            assert!(properties.next().is_none());
            assert_eq!(properties.position(), expected_end);
            assert!(properties.next().is_none());
        }
    }

    #[test]
    fn palette_extraction_refuses_duplicates_and_trailing_malformed_properties() {
        let mut bytes = header(0, 150, 2, 1, 1);
        let property = [
            1u32.to_be_bytes().as_slice(),
            7u32.to_be_bytes().as_slice(),
            1u32.to_be_bytes().as_slice(),
            &[1, 2, 3],
        ]
        .concat();
        bytes.extend_from_slice(&property);
        let end = bytes.len();
        bytes.extend_from_slice(&[0; 8]);
        assert_eq!(xcf_palette(&bytes).unwrap().unwrap(), &[[1, 2, 3]]);
        for length in 0..bytes.len() {
            assert!(xcf_palette(&bytes[..length]).is_err());
        }
        bytes.truncate(end);
        bytes.extend_from_slice(&property);
        bytes.extend_from_slice(&[0; 8]);
        assert_eq!(xcf_palette(&bytes), Err(XcfHeaderError("duplicate palette")));
        bytes.truncate(end);
        bytes.extend_from_slice(&[17u32.to_be_bytes(), 1u32.to_be_bytes()].concat());
        assert!(
            xcf_palette(&bytes).is_err(),
            "valid palette must not hide truncated following metadata"
        );
    }

    #[test]
    fn property_truncation_and_count_limit_are_terminal_errors() {
        let mut bytes = header(0, 150, 0, 1, 1);
        bytes.extend_from_slice(&17u32.to_be_bytes());
        bytes.extend_from_slice(&1u32.to_be_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&[0; 8]);
        for end in 26..bytes.len() {
            let mut properties = xcf_properties(&bytes[..end]).unwrap();
            let mut failed = false;
            for value in properties.by_ref() {
                failed |= value.is_err();
            }
            assert!(failed);
            assert!(properties.next().is_none());
        }
        let mut excessive = header(0, 150, 0, 1, 1);
        for _ in 0..4097 {
            excessive.extend_from_slice(&[0, 0, 0, 99, 0, 0, 0, 0]);
        }
        let mut properties = xcf_properties(&excessive).unwrap();
        for _ in 0..4096 {
            assert!(properties.next().unwrap().is_ok());
        }
        assert_eq!(properties.next(), Some(Err(XcfHeaderError("property count"))));
        assert!(properties.next().is_none());
    }

    #[test]
    fn version_specific_precision_and_offset_layouts() {
        for (version, precision, size, offsets) in [
            (0, 150, 1, 4),
            (4, 3, 2, 4),
            (6, 500, 4, 4),
            (7, 500, 2, 4),
            (11, 650, 4, 8),
            (23, 775, 8, 8),
        ] {
            for color in 0..=2 {
                let bytes = header(version, precision, color, 10, 20);
                let parsed = parse_xcf_header(&bytes).unwrap();
                assert_eq!(
                    (parsed.sample_bytes, parsed.offset_bytes, parsed.base_type),
                    (size, offsets, color)
                );
                assert_eq!(parsed.properties_offset, bytes.len());
                for end in 0..bytes.len() {
                    assert!(parse_xcf_header(&bytes[..end]).is_err());
                }
            }
        }
    }
    #[test]
    fn malformed_and_excessive_headers_refuse_without_panics() {
        for (version, precision, color, width, height) in [
            (24, 150, 0, 1, 1),
            (7, 400, 0, 1, 1),
            (4, 150, 0, 1, 1),
            (0, 150, 3, 1, 1),
            (0, 150, 0, 0, 1),
            (23, 775, 0, u32::MAX, u32::MAX),
            (23, 775, 0, 65536, 65536),
        ] {
            assert!(parse_xcf_header(&header(version, precision, color, width, height)).is_err());
        }
        let mut bytes = header(11, 150, 0, 1, 1);
        bytes[10] = b'-';
        assert_eq!(parse_xcf_header(&bytes), Err(XcfHeaderError("version")));
    }
}

#[cfg(test)]
mod selected_layer_tests {
    use super::*;

    #[test]
    #[ignore = "requires pinned external GIMP gimp-2-6-file.xcf fixture"]
    fn selected_gimp_layers_preserve_mask_and_rollback_partial_cancellation() {
        let bytes = std::fs::read(std::env::var("RRRAH_XCF_LAYER_SOURCE").unwrap()).unwrap();
        let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
        assert!(matches!(
            flatten_xcf_legacy_normal(&bytes, &budget, || false),
            Err(XcfPixelError::Structure(XcfHeaderError(
                "unsupported legacy layer composition"
            )))
        ));
        assert_eq!(budget.peak(), 0);
        let count = parse_xcf_object_tables(&bytes).unwrap().layers.iter().count();
        assert_eq!(count, 2);
        let mut masks = 0;
        for index in 0..count {
            let decoded = decode_xcf_layer(&bytes, index, &budget, || false).unwrap();
            let retained = decoded.pixels.len() + decoded.mask.as_ref().map_or(0, |m| m.len());
            assert_eq!(budget.used(), retained as u64);
            if let Some(mask) = &decoded.mask {
                masks += 1;
                assert_eq!(mask.len(), 25 * 251);
                assert!(mask.iter().all(|&sample| sample == 0));
            }
            drop(decoded);
            assert_eq!(budget.used(), 0);
            assert!(matches!(
                decode_xcf_layer(&bytes, index, &budget, || budget.used() > 0),
                Err(XcfPixelError::Cancelled)
            ));
            assert_eq!(budget.used(), 0);
        }
        assert_eq!(masks, 1);
    }

    #[test]
    fn common_raster_route_preserves_xcf_pixels_profile_and_budget() {
        use rrrah_core::{RasterColorSpace, RasterPixels};
        let directory = std::env::temp_dir().join(format!(
            "rrrah-xcf-route-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        for (bytes, profiled) in [
            (
                include_bytes!("../../../tests/fixtures/xcf/normal-overlay-v1.xcf").as_slice(),
                false,
            ),
            (
                include_bytes!("../../../tests/fixtures/xcf/profiled-normal-overlay-v1.xcf").as_slice(),
                true,
            ),
        ] {
            for name in ["overlay.XCF", "misleading.cr3"] {
                let path = directory.join(name);
                std::fs::write(&path, bytes).unwrap();
                assert!(crate::is_supported_image_path(&path));
                let budget = rrrah_core::MemoryBudget::new(4096);
                let mut request = crate::DecodeRequest::new(&path);
                request.memory_budget = Some(budget.clone());
                assert_eq!(
                    crate::image_source_kind(&request).unwrap(),
                    crate::ImageSourceKind::Raster
                );
                let crate::DecodedImage::Raster(raster) = crate::decode_image(&request).unwrap() else {
                    panic!("XCF routed as sensor")
                };
                let RasterPixels::Rgba8(pixels) = raster.pixels() else {
                    panic!("wrong XCF precision")
                };
                assert_eq!(&**pixels, &[127, 128, 0, 255, 0, 0, 255, 255]);
                let retained = if profiled { 596 } else { 8 };
                assert_eq!(budget.used(), retained);
                if profiled {
                    assert!(
                        matches!(raster.color_space(), RasterColorSpace::Icc(profile) if profile.as_slice() == xcf_icc_profile(bytes).unwrap().unwrap())
                    );
                    let prepared = crate::prepare_raster_for_display(&raster).unwrap();
                    assert_eq!(prepared.color_space(), &RasterColorSpace::LinearSrgb);
                } else {
                    assert_eq!(raster.color_space(), &RasterColorSpace::Unspecified);
                    assert!(crate::prepare_raster_for_display(&raster).is_err());
                }
                let clone = raster.clone();
                drop(raster);
                assert_eq!(budget.used(), retained);
                drop(clone);
                assert_eq!(budget.used(), 0);
                if !profiled {
                    request.assume_untagged_srgb = true;
                    let raster = crate::decode_raster(&request).unwrap();
                    assert_eq!(raster.color_space(), &RasterColorSpace::AssumedSrgb);
                    let prepared =
                        crate::prepare_raster_for_display_with_budget(&raster, Some(&budget)).unwrap();
                    assert_eq!(prepared.color_space(), &RasterColorSpace::LinearSrgb);
                    drop(prepared);
                    drop(raster);
                    assert_eq!(budget.used(), 0);
                } else {
                    let tight = rrrah_core::MemoryBudget::new(595);
                    request.memory_budget = Some(tight.clone());
                    assert!(matches!(
                        decode_raster(bytes, &request),
                        Err(crate::RasterDecodeError::Source(crate::DecodeError::Memory(_)))
                    ));
                    assert_eq!(tight.used(), 0);
                    request.memory_budget = Some(budget.clone());
                }
                request.image_index = 1;
                assert!(matches!(
                    crate::decode_image(&request),
                    Err(crate::RasterDecodeError::Source(
                        crate::DecodeError::UnsupportedImageIndex { index: 1 }
                    ))
                ));
                assert_eq!(budget.used(), 0);
            }
        }
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn legacy_composition_defaults_are_explicit_and_unknown_choices_refuse_before_allocation() {
        let fixture = include_bytes!("../../../tests/fixtures/xcf/normal-overlay-v1.xcf");
        let offset = parse_xcf_object_tables(fixture)
            .unwrap()
            .layers
            .iter()
            .next()
            .unwrap();
        let layer = parse_xcf_layer(fixture, offset).unwrap();
        let property = layer
            .properties
            .into_iter()
            .map(Result::unwrap)
            .find(|p| p.kind == 7)
            .unwrap();
        let payload = property.payload.as_ptr() as usize - fixture.as_ptr() as usize;
        for (kind, value, accepted) in [
            (35u32, -1i32, true),
            (35, 0, true),
            (35, 1, true),
            (36, -2, true),
            (36, 0, true),
            (36, 2, true),
            (37, 0, true),
            (7, 3, false),
            (35, 2, false),
            (36, 1, false),
            (37, 2, false),
            (35, i32::MIN, false),
            (36, 123, false),
        ] {
            let mut bytes = fixture.to_vec();
            bytes[payload - 8..payload - 4].copy_from_slice(&kind.to_be_bytes());
            bytes[payload..payload + 4].copy_from_slice(&value.to_be_bytes());
            let budget = rrrah_core::MemoryBudget::new(1024);
            let result = flatten_xcf_legacy_normal(&bytes, &budget, || false);
            if accepted {
                let image = result.unwrap();
                assert_eq!(&*image.pixels, &[127, 128, 0, 255, 0, 0, 255, 255]);
                drop(image);
            } else {
                assert!(matches!(
                    result,
                    Err(XcfPixelError::Structure(XcfHeaderError(
                        "unsupported legacy layer composition"
                    )))
                ));
                assert_eq!(budget.peak(), 0);
            }
            assert_eq!(budget.used(), 0);
        }
    }

    #[test]
    fn legacy_flatten_masks_visibility_and_negative_offset_keep_managed_ownership() {
        for (bytes, expected) in [
            (
                include_bytes!("../../../tests/fixtures/xcf/normal-mask-enabled-v1.xcf").as_slice(),
                [255, 0, 0, 255, 0, 128, 127, 255],
            ),
            (
                include_bytes!("../../../tests/fixtures/xcf/normal-mask-disabled-v1.xcf").as_slice(),
                [127, 128, 0, 255, 0, 128, 127, 255],
            ),
            (
                include_bytes!("../../../tests/fixtures/xcf/normal-mask-negative-offset-v1.xcf").as_slice(),
                [127, 128, 0, 255, 0, 0, 255, 255],
            ),
        ] {
            let budget = rrrah_core::MemoryBudget::new(1024);
            let image = flatten_xcf_legacy_normal(bytes, &budget, || false).unwrap();
            assert_eq!(&*image.pixels, &expected);
            assert_eq!(budget.used(), 8);
            drop(image);
            assert_eq!(budget.used(), 0);
            let mut reached_mask = false;
            assert!(matches!(
                flatten_xcf_legacy_normal(bytes, &budget, || {
                    // Canvas (32), retained layer (8), newly reserved mask (2).
                    if budget.used() == 42 {
                        reached_mask = true;
                    }
                    reached_mask
                }),
                Err(XcfPixelError::Cancelled)
            ));
            assert!(reached_mask);
            assert_eq!(budget.used(), 0);
        }
    }

    #[test]
    fn hidden_layer_pixels_do_not_consume_flatten_working_budget() {
        for (index, bytes) in [
            include_bytes!("../../../tests/fixtures/xcf/normal-mask-hidden-v1.xcf").as_slice(),
            include_bytes!("../../../tests/fixtures/xcf/normal-mask-zero-opacity-v1.xcf").as_slice(),
        ]
        .into_iter()
        .enumerate()
        {
            let offset = parse_xcf_object_tables(bytes)
                .unwrap()
                .layers
                .iter()
                .next()
                .unwrap();
            let attributes = parse_xcf_layer(bytes, offset).unwrap().attributes().unwrap();
            assert_eq!(attributes.visible, index == 1);
            assert_eq!(attributes.opacity, if index == 0 { 1.0 } else { 0.0 });
            let budget = rrrah_core::MemoryBudget::new(64);
            // Explicit native extraction still admits and decodes the hidden plane.
            assert!(matches!(
                decode_xcf_layer(bytes, 0, &budget, || false),
                Err(XcfPixelError::Memory(_))
            ));
            assert_eq!(budget.used(), 0);
            assert_eq!(budget.peak(), 0);
            // Flattening needs only the smaller visible plane and canvas.
            let image = flatten_xcf_legacy_normal(bytes, &budget, || false).unwrap();
            assert_eq!(&*image.pixels, &[255, 0, 0, 255, 0, 0, 255, 255]);
            assert_eq!(budget.used(), 8);
            assert!(budget.peak() <= 64);
            drop(image);
            assert_eq!(budget.used(), 0);
        }
    }

    #[test]
    fn legacy_half_opacity_mask_preserves_uncovered_and_composited_pixels() {
        let bytes = include_bytes!("../../../tests/fixtures/xcf/normal-mask-half-opacity-v1.xcf");
        let budget = rrrah_core::MemoryBudget::new(1024);
        let image = flatten_xcf_legacy_normal(bytes, &budget, || false).unwrap();
        assert_eq!(&*image.pixels, &[255, 0, 0, 255, 0, 64, 191, 255]);
        assert_eq!(budget.used(), 8);
        drop(image);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn hidden_only_canvas_still_checks_global_compression_before_allocation() {
        let fixture = include_bytes!("../../../tests/fixtures/xcf/normal-mask-hidden-v1.xcf");
        let bottom = parse_xcf_object_tables(fixture)
            .unwrap()
            .layers
            .iter()
            .nth(1)
            .unwrap();
        let visibility = parse_xcf_layer(fixture, bottom)
            .unwrap()
            .properties
            .map(Result::unwrap)
            .find(|p| p.kind == 8)
            .unwrap()
            .payload;
        let visibility_offset = visibility.as_ptr() as usize - fixture.as_ptr() as usize;
        let compression = xcf_properties(fixture)
            .unwrap()
            .map(Result::unwrap)
            .find(|p| p.kind == 17)
            .unwrap()
            .payload;
        let compression_offset = compression.as_ptr() as usize - fixture.as_ptr() as usize;
        let mut bytes = fixture.to_vec();
        bytes[visibility_offset..visibility_offset + 4].copy_from_slice(&0u32.to_be_bytes());
        let budget = rrrah_core::MemoryBudget::new(1024);
        let image = flatten_xcf_legacy_normal(&bytes, &budget, || false).unwrap();
        assert_eq!(&*image.pixels, &[0; 8]);
        drop(image);
        assert_eq!(budget.used(), 0);
        for invalid in [3, 255] {
            bytes[compression_offset] = invalid;
            let denied = rrrah_core::MemoryBudget::new(0);
            assert!(matches!(
                flatten_xcf_legacy_normal(&bytes, &denied, || false),
                Err(XcfPixelError::Structure(XcfHeaderError("compression property")))
            ));
            assert_eq!(denied.peak(), 0);
        }
    }

    #[test]
    fn legacy_normal_flatten_composes_bottom_to_top() {
        let bytes = include_bytes!("../../../tests/fixtures/xcf/normal-overlay-v1.xcf");
        let budget = rrrah_core::MemoryBudget::new(1024);
        let image = flatten_xcf_legacy_normal(bytes, &budget, || false).unwrap();
        assert_eq!(&*image.pixels, &[127, 128, 0, 255, 0, 0, 255, 255]);
        assert_eq!(budget.used(), 8);
        drop(image);
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn legacy_flatten_preserves_indexed_pixels_and_releases_working_canvas() {
        for (bytes, expected) in [
            (
                include_bytes!("../../../tests/fixtures/xcf/indexed-v1-kind-4.xcf").as_slice(),
                [0, 255, 0, 255, 255, 0, 0, 255],
            ),
            (
                include_bytes!("../../../tests/fixtures/xcf/indexed-v1-kind-5.xcf").as_slice(),
                [0, 0, 0, 0, 255, 0, 0, 255],
            ),
        ] {
            let budget = rrrah_core::MemoryBudget::new(1024);
            let image = flatten_xcf_legacy_normal(bytes, &budget, || false).unwrap();
            assert_eq!((image.width, image.height), (2, 1));
            assert_eq!(&*image.pixels, &expected);
            assert_eq!(budget.used(), 8);
            drop(image);
            assert_eq!(budget.used(), 0);
            assert!(matches!(
                flatten_xcf_legacy_normal(bytes, &budget, || budget.used() > 0),
                Err(XcfPixelError::Cancelled)
            ));
            assert_eq!(budget.used(), 0);
            let small = rrrah_core::MemoryBudget::new(32);
            assert!(matches!(
                flatten_xcf_legacy_normal(bytes, &small, || false),
                Err(XcfPixelError::Memory(_))
            ));
            assert_eq!(small.used(), 0);
        }
    }

    #[test]
    fn selected_indexed_layers_preserve_native_bytes_and_managed_ownership() {
        for (bytes, expected) in [
            (
                include_bytes!("../../../tests/fixtures/xcf/indexed-v1-kind-4.xcf").as_slice(),
                &[1, 0][..],
            ),
            (
                include_bytes!("../../../tests/fixtures/xcf/indexed-v1-kind-5.xcf").as_slice(),
                &[1, 127, 0, 128][..],
            ),
        ] {
            let budget = rrrah_core::MemoryBudget::new(1024);
            let decoded = decode_xcf_layer(bytes, 0, &budget, || false).unwrap();
            assert_eq!(&*decoded.pixels, expected);
            assert_eq!((decoded.layer.width, decoded.layer.height), (2, 1));
            assert!(decoded.mask.is_none());
            assert_eq!(budget.used(), expected.len() as u64);
            let last = decoded.pixels.clone();
            drop(decoded);
            assert_eq!(budget.used(), expected.len() as u64);
            drop(last);
            assert_eq!(budget.used(), 0);
            for index in [1, usize::MAX] {
                assert!(matches!(decode_xcf_layer(bytes, index, &budget, || false),
                    Err(XcfPixelError::UnsupportedLayerIndex { requested, count: 1 }) if requested == index));
                assert_eq!(budget.used(), 0);
            }
            let denied = rrrah_core::MemoryBudget::new(0);
            assert!(matches!(
                decode_xcf_layer(bytes, 0, &denied, || false),
                Err(XcfPixelError::Memory(_))
            ));
            assert_eq!(denied.used(), 0);
        }
        let budget = rrrah_core::MemoryBudget::new(0);
        assert!(matches!(
            decode_xcf_layer(&[], usize::MAX, &budget, || true),
            Err(XcfPixelError::Cancelled)
        ));
        assert_eq!(budget.peak(), 0);
    }
    #[test]
    fn legacy_grayscale_flatten_preserves_levels_alpha_and_last_owner() {
        for (bytes, expected) in [
            (
                include_bytes!("../../../tests/fixtures/xcf/normal-gray-v1.xcf").as_slice(),
                [17, 17, 17, 255, 239, 239, 239, 255],
            ),
            (
                include_bytes!("../../../tests/fixtures/xcf/normal-gray-alpha-v1.xcf").as_slice(),
                [17, 17, 17, 128, 239, 239, 239, 255],
            ),
        ] {
            let budget = rrrah_core::MemoryBudget::new(1024);
            let image = flatten_xcf_legacy_normal(bytes, &budget, || false).unwrap();
            assert_eq!(&*image.pixels, &expected);
            let owner = image.pixels.clone();
            drop(image);
            assert_eq!(budget.used(), 8);
            drop(owner);
            assert_eq!(budget.used(), 0);
        }
    }
}
