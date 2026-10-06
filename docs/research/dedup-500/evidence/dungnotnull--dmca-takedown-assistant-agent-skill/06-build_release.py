# -*- coding: utf-8 -*-
"""Build and release script for dmca-takedown-assistant.

This script creates release packages with proper structure, validation,
and documentation.
"""
from __future__ import annotations

import json
import logging
import shutil
import subprocess
import sys
import tarfile
import tempfile
import zipfile
from datetime import datetime
from pathlib import Path
from typing import List, Optional


logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(levelname)s - %(message)s"
)
logger = logging.getLogger(__name__)


def get_version() -> str:
    """Get current version from config."""
    try:
        sys.path.insert(0, str(Path(__file__).parent.parent / "config"))
        import dmca_config
        return dmca_config.VERSION
    except Exception:
        return "1.1.0"


def validate_project_structure(project_dir: Path) -> bool:
    """Validate required project files exist."""
    required_files = [
        "CLAUDE.md",
        "PROJECT-detail.md",
        "PROJECT-DEVELOPMENT-PHASE-TRACKING.md",
        "README.md",
        "LICENSE",
        "SKILL.md",
        "SECOND-KNOWLEDGE-BRAIN.md",
        "requirements.txt",
        "config/dmca_config.py",
    ]

    required_dirs = [
        "skills",
        "tools",
        "tests",
        "architecture",
        "scripts",
        "references",
        "assets",
    ]

    logger.info("Validating project structure...")

    for filename in required_files:
        filepath = project_dir / filename
        if not filepath.exists():
            logger.error(f"Required file missing: {filename}")
            return False

    for dirname in required_dirs:
        dirpath = project_dir / dirname
        if not dirpath.exists():
            logger.error(f"Required directory missing: {dirname}")
            return False

    logger.info("Project structure validated")
    return True


def run_tests(project_dir: Path) -> bool:
    """Run all tests before building release."""
    logger.info("Running test suite...")

    test_script = project_dir / "scripts" / "run_all_tests.py"
    if not test_script.exists():
        logger.warning("Test runner not found, skipping tests")
        return True

    result = subprocess.run(
        [sys.executable, str(test_script)],
        cwd=project_dir,
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        logger.error(f"Tests failed:\n{result.stdout}")
        return False

    logger.info("All tests passed")
    return True


def create_package_structure(
    project_dir: Path,
    build_dir: Path,
    version: str,
) -> bool:
    """Create clean package structure for release."""
    logger.info(f"Creating package structure for version {version}...")

    # Files to include
    include_patterns = [
        "*.md",
        "*.txt",
        "*.py",
        "config/**/*",
        "skills/**/*",
        "tools/**/*",
        "tests/**/*",
        "architecture/**/*",
        "scripts/**/*",
        "references/**/*",
        "assets/**/*",
    ]

    # Files to exclude
    exclude_patterns = [
        "**/__pycache__/**",
        "**/*.pyc",
        "**/.pytest_cache/**",
        "**/.git/**",
        "**/evidence_store/**",
        "**/logs/**",
        "**/.env",
        "**/dist/**",
        "**/build/**",
    ]

    try:
        # Create build directory
        build_dir.mkdir(parents=True, exist_ok=True)

        # Copy project files
        for pattern in include_patterns:
            for source_path in project_dir.rglob(pattern.split("/")[-1]):
                # Check exclusion patterns
                if any(source_path.match(ep) for ep in exclude_patterns):
                    continue

                # Create destination path
                rel_path = source_path.relative_to(project_dir)
                dest_path = build_dir / rel_path
                dest_path.parent.mkdir(parents=True, exist_ok=True)

                # Copy file
                if source_path.is_file():
                    shutil.copy2(source_path, dest_path)
                    logger.debug(f"Copied: {rel_path}")

        logger.info(f"Package structure created at: {build_dir}")
        return True

    except Exception as e:
        logger.error(f"Failed to create package: {e}")
        return False


def generate_changelog(project_dir: Path, build_dir: Path) -> bool:
    """Include changelog in release package."""
    source_changelog = project_dir / "CHANGELOG.md"
    if not source_changelog.exists():
        logger.warning("CHANGELOG.md not found")
        return True

    dest_changelog = build_dir / "CHANGELOG.md"
    shutil.copy2(source_changelog, dest_changelog)
    logger.info("Changelog included in package")
    return True


def create_version_file(build_dir: Path, version: str) -> bool:
    """Create VERSION file in package."""
    version_file = build_dir / "VERSION"
    try:
        version_file.write_text(f"{version}\n", encoding="utf-8")
        logger.info(f"VERSION file created: {version}")
        return True
    except Exception as e:
        logger.error(f"Failed to create VERSION file: {e}")
        return False


def create_tarball(
    build_dir: Path,
    output_dir: Path,
    version: str,
) -> Optional[Path]:
    """Create compressed tarball of the package."""
    output_name = f"dmca-takedown-assistant-{version}"
    output_path = output_dir / f"{output_name}.tar.gz"

    logger.info(f"Creating tarball: {output_path}")

    try:
        output_dir.mkdir(parents=True, exist_ok=True)

        with tarfile.open(output_path, "w:gz") as tar:
            for item in build_dir.iterdir():
                tar.add(item, arcname=item.name)

        logger.info(f"Tarball created: {output_path}")
        return output_path

    except Exception as e:
        logger.error(f"Failed to create tarball: {e}")
        return None


def create_zipfile(
    build_dir: Path,
    output_dir: Path,
    version: str,
) -> Optional[Path]:
    """Create ZIP archive of the package."""
    output_name = f"dmca-takedown-assistant-{version}"
    output_path = output_dir / f"{output_name}.zip"

    logger.info(f"Creating ZIP archive: {output_path}")

    try:
        output_dir.mkdir(parents=True, exist_ok=True)

        with zipfile.ZipFile(output_path, "w", zipfile.ZIP_DEFLATED) as zipf:
            for item in build_dir.rglob("*"):
                if item.is_file():
                    arcname = item.relative_to(build_dir)
                    zipf.write(item, arcname)

        logger.info(f"ZIP archive created: {output_path}")
        return output_path

    except Exception as e:
        logger.error(f"Failed to create ZIP archive: {e}")
        return None


def generate_checksums(package_paths: List[Path], output_dir: Path) -> bool:
    """Generate SHA256 checksums for packages."""
    import hashlib

    checksum_file = output_dir / "SHA256SUMS"

    logger.info("Generating SHA256 checksums...")

    try:
        with open(checksum_file, "w", encoding="utf-8") as f:
            for pkg_path in package_paths:
                sha256 = hashlib.sha256()
                with open(pkg_path, "rb") as pkg:
                    for chunk in iter(lambda: pkg.read(4096), b""):
                        sha256.update(chunk)

                checksum = sha256.hexdigest()
                f.write(f"{checksum}  {pkg_path.name}\n")
                logger.info(f"{pkg_path.name}: {checksum}")

        logger.info(f"Checksums saved to: {checksum_file}")
        return True

    except Exception as e:
        logger.error(f"Failed to generate checksums: {e}")
        return False


def build_release(
    project_dir: Optional[Path] = None,
    output_dir: Optional[Path] = None,
    formats: List[str] = ["tar.gz", "zip"],
) -> bool:
    """Build release package."""
    if project_dir is None:
        project_dir = Path(__file__).parent.parent

    if output_dir is None:
        output_dir = project_dir / "dist"

    version = get_version()
    logger.info("=" * 60)
    logger.info(f"Building DMCA Takedown Assistant v{version}")
    logger.info("=" * 60)

    # Validate project structure
    if not validate_project_structure(project_dir):
        return False

    # Run tests
    if not run_tests(project_dir):
        logger.error("Build failed: tests did not pass")
        return False

    # Create temporary build directory
    with tempfile.TemporaryDirectory() as temp_dir:
        build_dir = Path(temp_dir) / "package"

        # Create package structure
        if not create_package_structure(project_dir, build_dir, version):
            return False

        # Include changelog
        if not generate_changelog(project_dir, build_dir):
            return False

        # Create VERSION file
        if not create_version_file(build_dir, version):
            return False

        # Create release packages
        package_paths: List[Path] = []

        if "tar.gz" in formats:
            tarball = create_tarball(build_dir, output_dir, version)
            if tarball:
                package_paths.append(tarball)

        if "zip" in formats:
            zipfile = create_zipfile(build_dir, output_dir, version)
            if zipfile:
                package_paths.append(zipfile)

        if not package_paths:
            logger.error("No packages created")
            return False

        # Generate checksums
        if not generate_checksums(package_paths, output_dir):
            return False

    # Success
    logger.info("=" * 60)
    logger.info("Build completed successfully!")
    logger.info("=" * 60)
    logger.info(f"Version: {version}")
    logger.info(f"Output directory: {output_dir}")
    logger.info("Packages created:")
    for pkg in package_paths:
        size_mb = pkg.stat().st_size / (1024 * 1024)
        logger.info(f"  - {pkg.name} ({size_mb:.2f} MB)")

    return True


def main():
    """Main entry point."""
    import argparse

    parser = argparse.ArgumentParser(
        description="Build release package for dmca-takedown-assistant"
    )
    parser.add_argument(
        "--project-dir",
        type=Path,
        help="Project directory (default: auto-detect)",
    )
    parser.add_argument(
        "--output-dir",
        "-o",
        type=Path,
        help="Output directory (default: dist/)",
    )
    parser.add_argument(
        "--format",
        "-f",
        choices=["tar.gz", "zip", "both"],
        default="both",
        help="Package format (default: both)",
    )

    args = parser.parse_args()

    formats = ["tar.gz", "zip"] if args.format == "both" else [args.format]

    success = build_release(
        project_dir=args.project_dir,
        output_dir=args.output_dir,
        formats=formats,
    )

    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()
