"""Project fingerprinting.

Given a directory, identify:
- Primary language(s) (by file extension distribution + marker files)
- Frameworks in use (from manifests like package.json)
- PLC vendor (if PLC content present)
- Which profile to activate

Strategy: marker files give strong signals; extension counts disambiguate.
"""

from __future__ import annotations

import json
from collections import Counter
from collections.abc import Iterable
from pathlib import Path

from pydantic import BaseModel, Field


# Directories to ignore when counting extensions
_IGNORE_DIRS = {
    "node_modules", "__pycache__", ".git", "venv", ".venv", "env",
    "dist", "build", "target", ".next", ".nuxt", ".pytest_cache",
    ".ruff_cache", ".mypy_cache", "coverage", "htmlcov",
    ".idea", ".vscode", ".gradle", ".terraform",
}

# Extension → language label. Mirrors v1's coverage (28 extensions) plus
# the JS-family ones we added in M2 for completeness.
_EXT_LANG: dict[str, str] = {
    # JS family
    ".js": "javascript", ".jsx": "javascript", ".mjs": "javascript", ".cjs": "javascript",
    ".ts": "typescript", ".tsx": "typescript",
    # Python
    ".py": "python", ".pyi": "python",
    # JVM family
    ".java": "java",
    ".kt": "kotlin", ".kts": "kotlin",
    ".scala": "scala", ".sc": "scala",
    # Systems languages
    ".go": "go",
    ".rs": "rust",
    ".c": "c", ".h": "c",
    ".cpp": "cpp", ".cc": "cpp", ".cxx": "cpp", ".hpp": "cpp", ".hxx": "cpp",
    ".cs": "c_sharp",
    # Dynamic
    ".rb": "ruby",
    ".php": "php",
    ".swift": "swift",
    ".lua": "lua",
    ".jl": "julia",
    ".dart": "dart",
    # SQL / Shell
    ".sql": "sql",
    ".sh": "shell", ".bash": "shell", ".zsh": "shell",
    # Niche / scientific
    ".m": "matlab",
    ".r": "r",
    ".sas": "sas",
    # Hardware / smart-contract
    ".sol": "solidity",
    ".v": "verilog", ".vh": "verilog", ".sv": "verilog", ".svh": "verilog",
    ".zig": "zig",
    ".mm": "objective_c",
    # COBOL
    ".cob": "cobol", ".cbl": "cobol", ".cpy": "cobol",
    # PLC (text)
    ".st": "structured_text", ".iecst": "structured_text",
    # PLC (vendor XML — needs content inspection to confirm)
    ".l5x": "plc_rockwell",
    ".smc2": "plc_omron",
}

# Language → which profile should handle it.
# Each language gets the most-specific profile available:
#   - Layer 2 (lint) dedicated:  js / python / rust / java / go / cpp
#   - Generic (Tree-sitter AST):  ruby / php / lua / sql / julia / scala /
#                                  kotlin / swift / shell / c_sharp
#   - LLM-only (no grammar):      matlab / r / verilog / sas / cobol /
#                                  solidity / zig / objective_c / dart
#   - PLC (vendor parsers in M4): structured_text + vendor variants
_LANG_PROFILE: dict[str, str] = {
    # --- Layer 1 + Layer 2 (full tooling) ---
    "javascript": "js",
    "typescript": "js",
    "python": "python",
    "rust": "rust",
    "java": "java",
    "go": "go",
    "c": "cpp",       # cppcheck handles both C and C++
    "cpp": "cpp",
    # --- Phase-2 deep profiles (Tree-sitter + dedicated linter) ---
    "kotlin": "kotlin",
    "ruby": "ruby",
    "php": "php",
    "lua": "lua",
    "sql": "sql",
    "shell": "shell",
    # --- Generic profile (Tree-sitter AST only, no dedicated linter yet) ---
    "scala": "generic",
    "c_sharp": "generic",
    "swift": "generic",
    "julia": "generic",
    # --- LLM-only profiles (no Tree-sitter grammar) ---
    "matlab": "matlab",
    "r": "r",
    "verilog": "verilog",
    "systemverilog": "verilog",
    # vhdl uses a different grammar + tool ecosystem; falls through to LLM-only
    "sas": "sas",
    "cobol": "cobol",
    "solidity": "solidity",
    "zig": "zig",
    "objective_c": "objc",
    "dart": "dart",
    # --- PLC (vendor XML formats route to plc profile) ---
    # Must cover every vendor _PLC_XML_SIGS can name, or a confirmed vendor
    # falls through to "auto".
    "structured_text": "plc",
    "plc_rockwell": "plc",
    "plc_omron": "plc",
    "plc_siemens": "plc",
    "plc_beckhoff": "plc",
    "plc_codesys": "plc",
    "plc_abb": "plc",
    "plc_ge": "plc",
}


# Extensions that name their PLC vendor outright — no content peek needed.
_PLC_EXT_VENDOR: dict[str, str] = {
    ".l5x": "rockwell",
    ".smc2": "omron",
}

# How many generic .xml files to open when identifying a vendor.
_PLC_XML_SAMPLE = 20

# Vendor signatures, lowercase, matched against the first 4KB of a generic
# .xml file. Every marker must be a namespace URI or a vendor-specific root
# element — NEVER a generic XML fragment. The old "project xmlns" marker
# matched Maven's `<project xmlns="http://maven.apache.org/POM/4.0.0">`, and a
# bare "omron" matched any document that merely mentioned the company, so any
# Java repo with a pom.xml read as a CODESYS project.
_PLC_XML_SIGS: dict[str, tuple[str, ...]] = {
    "siemens": ("simaticml", "siemens.com/automation"),
    "beckhoff": ("tcpou", "twincat", "beckhoff.com"),
    "codesys": ("codesys.com", "plcopen.org/xml"),
    "rockwell": ("rslogix", "rsl5kfile", "rockwellautomation.com"),
    "abb": ("abb.com/automation",),
    "ge": ("ge-ip.com", "ge.com/automation", "proficy"),
    "omron": ("omron.com", "sysmac"),
}


class ProjectFingerprint(BaseModel):
    """A summary of what a project directory contains."""

    root: str

    # Distribution
    extension_counts: dict[str, int] = Field(default_factory=dict)
    total_files: int = 0

    # Languages, ordered by file-count desc
    languages: list[str] = Field(default_factory=list)
    primary_language: str | None = None

    # Frameworks (from manifest inspection)
    frameworks: list[str] = Field(default_factory=list)

    # PLC-specific
    plc_vendor: str | None = None  # siemens | beckhoff | codesys | rockwell | abb | ge | omron

    # Recommended profile
    suggested_profile: str = "auto"  # auto | js | plc | python — what we'd activate

    # Marker files we found
    markers: list[str] = Field(default_factory=list)


# --- Detection -----------------------------------------------------------------


def detect_project(root: Path | str) -> ProjectFingerprint:
    """Walk root, build a fingerprint."""
    root = Path(root).expanduser().resolve()
    fp = ProjectFingerprint(root=str(root))

    if not root.is_dir():
        return fp

    # Pass 1: walk files, count extensions, note markers
    ext_counter: Counter[str] = Counter()
    total = 0
    has_package_json = False
    has_pyproject = False
    # .l5x / .smc2 name their vendor by extension and are already counted via
    # _EXT_LANG; generic .xml names nothing, so it needs a content peek and is
    # the only bucket that may add to the language counts.
    plc_ext_files: list[Path] = []
    ambiguous_xmls: list[Path] = []

    for p in _walk(root):
        total += 1
        ext = p.suffix.lower()
        if ext:
            ext_counter[ext] += 1

        name = p.name
        if name == "package.json":
            has_package_json = True
            fp.markers.append(str(p.relative_to(root)))
        elif name == "pyproject.toml":
            has_pyproject = True
            fp.markers.append(str(p.relative_to(root)))
        elif name == "tsconfig.json":
            fp.markers.append(str(p.relative_to(root)))
        elif name == "go.mod":
            fp.markers.append(str(p.relative_to(root)))
        elif name == "Cargo.toml":
            fp.markers.append(str(p.relative_to(root)))
        elif ext in _PLC_EXT_VENDOR:
            plc_ext_files.append(p)
            fp.markers.append(str(p.relative_to(root)))
        elif ext == ".xml" and total <= 5000:  # inspect a sample to avoid huge scans
            ambiguous_xmls.append(p)

    fp.total_files = total
    fp.extension_counts = dict(ext_counter)

    # Pass 2: compute language distribution
    lang_counts: Counter[str] = Counter()
    for ext, count in ext_counter.items():
        lang = _EXT_LANG.get(ext)
        if lang:
            lang_counts[lang] += count

    # Pass 3: PLC vendor detection. Runs BEFORE primary_language is decided so
    # vendor-confirmed XMLs join the language counts and PLC competes on file
    # count like everything else — rather than overriding the primary language
    # outright, which let one stray XML repaint a whole repo as PLC.
    if plc_ext_files:
        fp.plc_vendor = _PLC_EXT_VENDOR.get(plc_ext_files[0].suffix.lower())
    if ambiguous_xmls:
        vendor, hits = _detect_plc_vendor(ambiguous_xmls[:_PLC_XML_SAMPLE])
        if vendor:
            fp.plc_vendor = fp.plc_vendor or vendor
            lang_counts[f"plc_{vendor}"] += hits

    fp.languages = [lang for lang, _ in lang_counts.most_common()]
    fp.primary_language = fp.languages[0] if fp.languages else None

    # Pass 4: framework hints from manifests
    if has_package_json:
        fp.frameworks.extend(_detect_js_frameworks(root / "package.json"))

    # Pass 5: suggested profile
    fp.suggested_profile = _suggest_profile(fp)

    return fp


def _walk(root: Path):
    """Yield files, skipping noise dirs."""
    for p in root.rglob("*"):
        if not p.is_file():
            continue
        # Skip if any parent dir is in ignore set
        if any(part in _IGNORE_DIRS for part in p.relative_to(root).parts[:-1]):
            continue
        yield p


def _detect_js_frameworks(package_json_path: Path) -> list[str]:
    """Inspect package.json deps for known frameworks."""
    try:
        data = json.loads(package_json_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return []

    deps = {}
    deps.update(data.get("dependencies") or {})
    deps.update(data.get("devDependencies") or {})

    hits: list[str] = []
    framework_markers = {
        "react": "react",
        "next": "nextjs",
        "vue": "vue",
        "nuxt": "nuxt",
        "@angular/core": "angular",
        "svelte": "svelte",
        "express": "express",
        "@nestjs/core": "nestjs",
        "fastify": "fastify",
        "@prisma/client": "prisma",
        "typeorm": "typeorm",
        "mongoose": "mongoose",
        "sequelize": "sequelize",
    }
    for dep, label in framework_markers.items():
        if dep in deps:
            hits.append(label)
    return hits


def _detect_plc_vendor(xml_paths: list[Path]) -> tuple[str | None, int]:
    """Peek at XML headers to identify the vendor of generic .xml files.

    Returns ``(vendor, confirmed_file_count)`` over the paths given. Counting
    every match — rather than returning on the first hit — is what lets the
    caller weigh PLC against other languages by file count.
    """
    hits: Counter[str] = Counter()
    for path in xml_paths:
        try:
            with path.open("r", encoding="utf-8", errors="ignore") as f:
                head = f.read(4096).lower()
        except OSError:
            continue
        for vendor, markers in _PLC_XML_SIGS.items():
            if any(m in head for m in markers):
                hits[vendor] += 1
                break  # one file votes once
    if not hits:
        return None, 0
    vendor, count = hits.most_common(1)[0]
    return vendor, count


def _suggest_profile(fp: ProjectFingerprint) -> str:
    """Pick the best profile name for the agent to activate.

    Every language competes purely on file count, PLC included. There is
    deliberately no "any PLC file present → plc" short-circuit: that made a
    single .st test fixture repaint an otherwise-Python repo as a PLC project,
    and because auto-detect fingerprints the *directory*, no choice of scan
    target could escape it. Mixed repos where PLC is the minority but still the
    point of the review are served by an explicit `--profile plc`.
    """
    if not fp.primary_language:
        return "auto"
    return _LANG_PROFILE.get(fp.primary_language) or "auto"


def suggest_profile_for_files(paths: Iterable[str | Path]) -> str | None:
    """Pick a profile from an explicit file list, ignoring the directory around it.

    Used for scoped runs (single-file review, pasted snippets) where the
    surrounding repo's language mix says nothing about the file under review.

    Returns None when no path maps to a known language, so callers can fall
    back to a whole-directory fingerprint.
    """
    counts: Counter[str] = Counter()
    for p in paths:
        lang = _EXT_LANG.get(Path(p).suffix.lower())
        if lang:
            counts[lang] += 1
    if not counts:
        return None
    return _LANG_PROFILE.get(counts.most_common(1)[0][0])


# --- Display -------------------------------------------------------------------


def summarize_fingerprint(fp: ProjectFingerprint) -> str:
    """Build a human-readable summary of a fingerprint (for stream output)."""
    lines: list[str] = []
    if fp.primary_language:
        # Compute percentage if extensions known
        lang_files = sum(
            n for ext, n in fp.extension_counts.items()
            if _EXT_LANG.get(ext) == fp.primary_language
        )
        pct = (lang_files / fp.total_files * 100) if fp.total_files else 0
        lines.append(f"Primary language : {fp.primary_language} ({pct:.0f}% of files)")
    else:
        lines.append("Primary language : (unknown)")

    if fp.frameworks:
        lines.append(f"Frameworks       : {', '.join(fp.frameworks)}")
    if fp.plc_vendor:
        lines.append(f"PLC vendor       : {fp.plc_vendor}")
    if fp.markers:
        shown = fp.markers[:3]
        more = f" (+{len(fp.markers) - 3} more)" if len(fp.markers) > 3 else ""
        lines.append(f"Marker files     : {', '.join(shown)}{more}")
    lines.append(f"Suggested profile: {fp.suggested_profile}")
    lines.append(f"Total files      : {fp.total_files}")
    return "\n".join(lines)
