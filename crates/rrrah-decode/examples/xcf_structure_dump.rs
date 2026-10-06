//! Structural/pixel-level diagnostics, not a flattened image exporter.
use rrrah_decode::{
    decode_xcf_level, parse_xcf_channel, parse_xcf_header, parse_xcf_hierarchy, parse_xcf_layer,
    parse_xcf_object_tables, xcf_properties,
};
use std::io::Read;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("usage: xcf_structure_dump FILE [PIXEL_DIRECTORY]")?;
    let pixel_directory = args.next().map(std::path::PathBuf::from);
    if args.next().is_some() {
        return Err("usage: xcf_structure_dump FILE [PIXEL_DIRECTORY]".into());
    }
    if let Some(directory) = &pixel_directory {
        std::fs::create_dir_all(directory)?;
    }
    let budget = rrrah_core::MemoryBudget::new(512 * 1024 * 1024);
    let mut file = std::fs::File::open(&path)?;
    let length = usize::try_from(file.metadata()?.len())?;
    let mut bytes = budget.try_buffer(length, 0u8)?;
    file.read_exact(&mut bytes)?;
    let bytes = bytes.freeze();
    let header = parse_xcf_header(&bytes)?;
    let profile = rrrah_decode::xcf_icc_profile(&bytes)?;
    let palette = rrrah_decode::xcf_palette(&bytes)?;
    println!(
        "{{\"icc_profile_bytes\":{}}}",
        profile.map_or(0, |data| data.len())
    );
    let tables = parse_xcf_object_tables(&bytes)?;
    let tree = rrrah_decode::parse_xcf_layer_tree(&bytes, &budget, || false)?;
    for (index, node) in tree.iter().enumerate() {
        let parent = node.parent.map_or_else(|| "null".to_owned(), |v| v.to_string());
        println!(
            "{{\"tree_node\":{index},\"parent\":{parent},\"sibling\":{},\"is_group\":{}}}",
            node.sibling, node.is_group
        );
    }
    drop(tree);
    let mut compression = 1;
    for property in xcf_properties(&bytes)? {
        let property = property?;
        if property.kind == 17 {
            if property.payload.len() != 1 {
                return Err("invalid compression property".into());
            }
            compression = property.payload[0];
        }
    }
    println!(
        "{{\"version\":{},\"width\":{},\"height\":{},\"compression\":{},\"channels\":{}}}",
        header.version,
        header.width,
        header.height,
        compression,
        tables.channels.iter().count()
    );
    for (index, offset) in tables.layers.iter().enumerate() {
        let mut layer = parse_xcf_layer(&bytes, offset)?;
        let mut property_count = 0;
        for property in layer.properties.by_ref() {
            property?;
            property_count += 1;
        }
        println!("{{\"layer_property_count\":{index},\"count\":{property_count}}}");
        let group = layer.group()?;
        print!(
            "{{\"layer_group\":{index},\"is_group\":{},\"item_path\":[",
            group.is_group
        );
        for (item_index, item) in group.item_path().enumerate() {
            if item_index != 0 {
                print!(",");
            }
            print!("{item}");
        }
        println!("]}}");
        let attributes = layer.attributes()?;
        let composition = layer.composition_attributes()?;
        let optional = |value: Option<i32>| value.map_or_else(|| "null".to_owned(), |v| v.to_string());
        println!(
            "{{\"layer_composition\":{index},\"blend_mode\":{},\"composite_mode\":{},\"composite_space\":{},\"blend_space\":{}}}",
            composition.blend_mode,
            optional(composition.composite_mode),
            optional(composition.composite_space),
            optional(composition.blend_space)
        );
        println!(
            "{{\"layer_attributes\":{index},\"visible\":{},\"opacity\":{},\"offset_x\":{},\"offset_y\":{},\"apply_mask\":{}}}",
            attributes.visible,
            attributes.opacity,
            attributes.offset_x,
            attributes.offset_y,
            attributes.apply_mask
        );
        let sample_bytes = if layer.kind >= 4 { 1 } else { header.sample_bytes };
        let rrrah_decode::XcfDecodedLayer { pixels, mask, .. } =
            rrrah_decode::decode_xcf_layer(&bytes, index, &budget, || false)?;
        let pixels = if layer.kind == 1 && sample_bytes == 1 {
            rrrah_decode::prepare_xcf_legacy_rgba8(pixels, 1, &[], &budget, || false)?
        } else {
            pixels
        };
        println!(
            "{{\"layer\":{},\"width\":{},\"height\":{},\"kind\":{},\"mask\":{},\"effects\":{},\"pixel_blake3\":\"{}\"}}",
            index,
            layer.width,
            layer.height,
            layer.kind,
            layer.mask_offset,
            layer.effects.iter().count(),
            blake3::hash(&pixels)
        );
        if let Some(directory) = &pixel_directory {
            std::fs::write(directory.join(format!("layer-{index}.bin")), &*pixels)?;
        }
        // Separate encoded-RGBA diagnostics preserve the native-plane oracle.
        // Modern indexed-alpha interpretation remains unqualified.
        if (layer.kind == 4 && header.version >= 1) || (layer.kind == 5 && header.version == 1) {
            let rgba = rrrah_decode::prepare_xcf_legacy_rgba8(
                pixels.clone(),
                layer.kind,
                palette.unwrap_or(&[]),
                &budget,
                || false,
            )?;
            println!(
                "{{\"indexed_rgba_layer\":{index},\"pixel_blake3\":\"{}\"}}",
                blake3::hash(&rgba)
            );
            if let Some(directory) = &pixel_directory {
                std::fs::write(directory.join(format!("rgba-layer-{index}.bin")), &*rgba)?;
            }
        }
        drop(pixels);
        if let Some(mask) = mask {
            if let Some(directory) = &pixel_directory {
                std::fs::write(directory.join(format!("mask-{index}.bin")), &*mask)?;
            }
            println!(
                "{{\"mask\":{index},\"width\":{},\"height\":{},\"pixel_blake3\":\"{}\"}}",
                layer.width,
                layer.height,
                blake3::hash(&mask)
            );
        }
    }
    for (index, offset) in tables.channels.iter().enumerate() {
        dump_channel(
            &bytes,
            offset,
            header.width,
            header.height,
            header.sample_bytes,
            compression,
            &budget,
            "channel",
            index,
            pixel_directory.as_deref(),
        )?;
    }
    drop(bytes);
    if budget.used() != 0 {
        return Err("managed allocation retained".into());
    }
    println!("{{\"managed_peak\":{},\"managed_used\":0}}", budget.peak());
    Ok(())
}

fn dump_channel(
    bytes: &[u8],
    offset: u64,
    width: u32,
    height: u32,
    sample_bytes: u8,
    compression: u8,
    budget: &rrrah_core::MemoryBudget,
    kind: &str,
    index: usize,
    pixel_directory: Option<&std::path::Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let channel = parse_xcf_channel(bytes, offset, width, height)?;
    let level = parse_xcf_hierarchy(bytes, channel.hierarchy_offset, width, height, sample_bytes)?;
    let pixels = decode_xcf_level(bytes, &level, compression, budget, || false)?;
    if let Some(directory) = pixel_directory {
        std::fs::write(directory.join(format!("{kind}-{index}.bin")), &*pixels)?;
    }
    println!(
        "{{\"{kind}\":{index},\"width\":{width},\"height\":{height},\"pixel_blake3\":\"{}\"}}",
        blake3::hash(&pixels)
    );
    Ok(())
}
