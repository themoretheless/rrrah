#!/usr/bin/env python3
"""Run the vendor mesh-bound regression with the project's patched dependencies."""
import argparse
from pathlib import Path
import shutil
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-dir", type=Path,
                        default=Path("/tmp/rrrah-required-corpus-target"))
    parser.add_argument("--renderer", action="store_true",
                        help="Run shading texture cancellation in the renderer")
    parser.add_argument("--filter", help="Override the vendor test filter")
    parser.add_argument("--include-ignored", action="store_true",
                        help="Explicitly run ignored external-fixture diagnostics")
    args = parser.parse_args()
    project = Path(__file__).resolve().parents[1]
    # Patched dependency packages are not workspace members. Isolate their
    # unit-test workspace without changing the product workspace or lockfile.
    with tempfile.TemporaryDirectory(prefix="rrrah-mesh-bounds-test-") as tmp:
        root = Path(tmp)
        for name in ("hayro", "hayro-interpret", "moxcms"):
            shutil.copytree(project / "vendor" / name, root / name)
        manifest = root / ("hayro" if args.renderer else "hayro-interpret") / "Cargo.toml"
        with manifest.open("a") as output:
            output.write('\n[workspace]\n\n[patch.crates-io]\n'
                         'moxcms = { path = "../moxcms" }\n'
                         '\n[profile.dev]\nopt-level = 1\n')
        if args.renderer:
            text = manifest.read_text().replace(
                '[patch.crates-io]\n',
                '[patch.crates-io]\nhayro-interpret = { path = "../hayro-interpret" }\n')
            manifest.write_text(text)
        subprocess.run([
            "cargo", "test", "--manifest-path", str(manifest), "--lib",
            args.filter or ("renderer_cancellation_tests" if args.renderer else "sample_bounds_tests"),
            "--offline", "--target-dir",
            str(args.target_dir.resolve()),
            *(["--", "--include-ignored"] if args.include_ignored else []),
        ], check=True)


if __name__ == "__main__":
    main()
