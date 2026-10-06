//! CC0-authored opaque constant fixtures; codec library is pinned in Cargo.lock.
use basis_universal::{BasisTextureFormat, ColorSpace, Compressor, CompressorParams};
fn main() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/basis");
    std::fs::create_dir_all(&root).unwrap();
    let mut rows = Vec::new();
    for (name, format) in [
        ("etc1s", BasisTextureFormat::ETC1S),
        ("uastc", BasisTextureFormat::UASTC4x4),
    ] {
        for linear in [false, true] {
            let mut params = CompressorParams::new();
            params.set_basis_format(format);
            params.set_color_space(if linear {
                ColorSpace::Linear
            } else {
                ColorSpace::Srgb
            });
            params.set_generate_mipmaps(true);
            params.set_mipmap_smallest_dimension(1);
            params.set_print_status_to_stdout(false);
            params.source_image_mut(0).init(&[255u8; 8 * 8 * 4], 8, 8, 4);
            let mut compressor = Compressor::default();
            // Params/source remain owned until both native operations finish.
            #[allow(unsafe_code)]
            unsafe {
                compressor.init(&params);
                compressor.process().unwrap();
            }
            let file = format!("{name}-{}.basis", if linear { "linear" } else { "srgb" });
            let bytes = compressor.basis_file();
            std::fs::write(root.join(&file), bytes).unwrap();
            rows.push(serde_json::json!({"file":file,"source_blake3":blake3::hash(bytes).to_hex().to_string(),"codec":"basis-universal 0.3.1","linear_metrics":linear,"declared_linear":linear && format == BasisTextureFormat::ETC1S,"dimensions":[8,8],"mips":[[8,8],[4,4],[2,2],[1,1]],"reference":"mathematical opaque white, RGBA all 255; independent codec oracle pending"}));
        }
    }
    let colors = [
        [255u8, 0, 0, 255],
        [0, 255, 0, 170],
        [0, 0, 255, 85],
        [255, 255, 0, 0],
    ];
    let mut source = Vec::new();
    for y in 0..5 {
        for x in 0..7 {
            source.extend_from_slice(&colors[(y / 4) * 2 + x / 4]);
        }
    }
    std::fs::write(root.join("colored-alpha.rgba"), &source).unwrap();
    for (name, format) in [
        ("etc1s", BasisTextureFormat::ETC1S),
        ("uastc", BasisTextureFormat::UASTC4x4),
    ] {
        let mut params = CompressorParams::new();
        params.set_basis_format(format);
        params.set_color_space(ColorSpace::Srgb);
        params.set_generate_mipmaps(false);
        params.set_print_status_to_stdout(false);
        params.source_image_mut(0).init(&source, 7, 5, 4);
        let mut compressor = Compressor::default();
        // Params and source outlive native processing.
        #[allow(unsafe_code)]
        unsafe {
            compressor.init(&params);
            compressor.process().unwrap();
        }
        let file = format!("{name}-colored-alpha.basis");
        let bytes = compressor.basis_file();
        std::fs::write(root.join(&file), bytes).unwrap();
        rows.push(serde_json::json!({"file":file,"source_blake3":blake3::hash(bytes).to_hex().to_string(),
            "codec":"basis-universal 0.3.1","declared_linear":false,"dimensions":[7,5],"mips":[[7,5]],
            "reference":"colored-alpha.rgba authored block colors; per-channel absolute error <=8; independent codec oracle pending"}));
    }
    for (name, format) in [
        ("etc1s", BasisTextureFormat::ETC1S),
        ("uastc", BasisTextureFormat::UASTC4x4),
    ] {
        let mut params = CompressorParams::new();
        params.set_basis_format(format);
        params.set_color_space(ColorSpace::Srgb);
        params.set_generate_mipmaps(true);
        params.set_mipmap_smallest_dimension(1);
        params.set_print_status_to_stdout(false);
        let red = [255u8, 0, 0, 255].repeat(64);
        let green = [0u8, 255, 0, 255].repeat(16);
        params.source_image_mut(0).init(&red, 8, 8, 4);
        params.source_image_mut(1).init(&green, 4, 4, 4);
        let mut compressor = Compressor::default();
        // Source and params remain owned through native processing.
        #[allow(unsafe_code)]
        unsafe {
            compressor.init(&params);
            compressor.process().unwrap();
        }
        let file = format!("{name}-multi.basis");
        let bytes = compressor.basis_file();
        std::fs::write(root.join(&file), bytes).unwrap();
        rows.push(serde_json::json!({"file":file,"source_blake3":blake3::hash(bytes).to_hex().to_string(),
            "codec":"basis-universal 0.3.1","declared_linear":false,
            "images":[{"dimensions":[8,8],"color":[255,0,0,255],"mips":[[8,8],[4,4],[2,2],[1,1]]},
                      {"dimensions":[4,4],"color":[0,255,0,255],"mips":[[4,4],[2,2],[1,1]]}],
            "reference":"authored red then green mip chains, image-first selection; per-channel error <=8; independent codec oracle pending"}));
    }
    std::fs::write(
        root.join("manifest.json"),
        serde_json::to_string_pretty(&serde_json::json!({"license":"CC0-1.0","images":rows})).unwrap() + "\n",
    )
    .unwrap();
}
