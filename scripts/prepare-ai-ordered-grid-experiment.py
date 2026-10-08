#!/usr/bin/env python3
"""Copy current sources and activate the ordered-grid experiment outside checkout.

The destination must not exist. This prepares sources only; build and compare
with qualify-ai-upstream.py separately. No production renderer is changed.
"""
import argparse
from pathlib import Path
import shutil


def replace_once(text, old, new):
    if text.count(old) != 1:
        raise ValueError(f"experiment source anchor changed: {old!r}")
    return text.replace(old, new)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    project = Path(__file__).resolve().parents[1]
    root = args.destination.resolve()
    root.mkdir(exist_ok=False)
    for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml"):
        if (project / name).exists():
            shutil.copy2(project / name, root / name)
    for name in ("crates", "vendor"):
        shutil.copytree(project / name, root / name)
    path = root / "vendor/hayro-interpret/src/encode/ordered_grid.rs"
    text = path.read_text().split("#[cfg(test)]\nmod tests")[0]
    text = replace_once(text, "fn sample_ordered(\n    triangles: &[Triangle],",
                        "pub(super) fn sample_ordered<I: IntoIterator<Item = Result<Triangle, ()>>>(\n    triangles: I,\n    components: usize,")
    text = replace_once(text, "    let components = triangles.first()?.p0.colors.len();\n", "")
    text = replace_once(text, "for source in triangles {", "for source in triangles {\n        let source = source.ok()?;")
    path.write_text(text)
    path = root / "vendor/hayro-interpret/src/encode.rs"
    text = replace_once(path.read_text(), "#[cfg(test)]\nmod ordered_grid;", "mod ordered_grid;")
    anchor = "    if cancelled() { return None; }\n    let spill = color_spill_bound(components)?;"
    text = replace_once(text, anchor,
                        "    return ordered_grid::sample_ordered(triangles, components, transform, bounds.unwrap_or(kurbo::Rect::new(0.0, 0.0, 65535.0, 65535.0)), cancelled, admit);\n" + anchor)
    path.write_text(text)
    path = root / "vendor/hayro-interpret/src/shading.rs"
    path.write_text(replace_once(path.read_text(),
                               "generate_patch_grid(map_coordinate, interpolate, buffer, cancelled, false)",
                               "generate_patch_grid(map_coordinate, interpolate, buffer, cancelled, true)"))
    path = root / "crates/rrrah-decode/examples/raster_fixture_dump.rs"
    path.write_text(replace_once(path.read_text(), "    Ok(())\n}",
        '    drop(image);\n    let budget = request.memory_budget.as_ref().unwrap();\n'
        '    eprintln!("managed_peak_bytes={} final_used_bytes={}", budget.peak(), budget.used());\n'
        '    if budget.used() != 0 { return Err("managed raster buffers not released".into()); }\n'
        '    Ok(())\n}'))
    print(root)


if __name__ == "__main__":
    main()
