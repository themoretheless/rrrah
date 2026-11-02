"""Smoke tests for project fingerprinting / profile auto-detection.

Regression cover for the "one .st fixture repaints the whole repo as PLC" bug:
a single PLC file (or a single Maven pom.xml) must not outvote the language the
repo is actually written in, and a scoped single-file review must take its
profile from the file itself rather than from the directory around it.

Run directly:  python tests/test_detect_smoke.py
"""

from __future__ import annotations

import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))

from revio.detect.fingerprint import (  # noqa: E402
    _LANG_PROFILE,
    _PLC_XML_SIGS,
    detect_project,
    suggest_profile_for_files,
)

POM_XML = """<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <modelVersion>4.0.0</modelVersion>
  <groupId>com.example</groupId>
  <artifactId>demo</artifactId>
</project>
"""

CODESYS_XML = """<?xml version="1.0" encoding="utf-8"?>
<Project xmlns="http://www.codesys.com/project">
  <POU Name="Main" />
</Project>
"""

ST_SOURCE = """FUNCTION_BLOCK MotorControl
VAR
    Running : BOOL;
END_VAR
IF Start THEN Running := TRUE; END_IF
END_FUNCTION_BLOCK
"""


def _mkrepo(tmp: Path, files: dict[str, str]) -> Path:
    root = tmp / "repo"
    for rel, body in files.items():
        p = root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(body, encoding="utf-8")
    return root


def check(label: str, got, want) -> bool:
    ok = got == want
    mark = "✓" if ok else "✗"
    detail = f"{got!r}" if ok else f"{got!r}  (expected {want!r})"
    print(f"  · {label}: {detail} {mark}")
    return ok


def main() -> int:
    failures = 0

    with tempfile.TemporaryDirectory() as td:
        tmp = Path(td)

        print("\n[1] a lone .st fixture must not hijack a Python repo")
        root = _mkrepo(tmp / "a", {
            **{f"src/mod_{i}.py": "x = 1\n" for i in range(12)},
            "pyproject.toml": "[project]\nname='demo'\n",
            "tests/fixtures/plc_sample/MotorControl.st": ST_SOURCE,
        })
        fp = detect_project(root)
        failures += not check("primary_language", fp.primary_language, "python")
        failures += not check("suggested_profile", fp.suggested_profile, "python")

        print("\n[2] Maven pom.xml must not read as a CODESYS project")
        root = _mkrepo(tmp / "b", {
            **{f"src/main/java/A{i}.java": "class A {}\n" for i in range(8)},
            "pom.xml": POM_XML,
        })
        fp = detect_project(root)
        failures += not check("plc_vendor", fp.plc_vendor, None)
        failures += not check("suggested_profile", fp.suggested_profile, "java")

        print("\n[3] a genuine PLC repo still detects as plc")
        root = _mkrepo(tmp / "c", {
            **{f"pou/Block{i}.st": ST_SOURCE for i in range(6)},
            "README.md": "# plant\n",
        })
        fp = detect_project(root)
        failures += not check("suggested_profile", fp.suggested_profile, "plc")

        print("\n[4] vendor XML wins when it outnumbers other source")
        root = _mkrepo(tmp / "d", {
            **{f"plc/Proj{i}.xml": CODESYS_XML for i in range(9)},
            "tools/helper.py": "x = 1\n",
        })
        fp = detect_project(root)
        failures += not check("plc_vendor", fp.plc_vendor, "codesys")
        failures += not check("suggested_profile", fp.suggested_profile, "plc")

        print("\n[5] .l5x names its vendor by extension, not by content peek")
        root = _mkrepo(tmp / "e", {"line/Main.l5x": "<RSLogix5000Content/>\n"})
        fp = detect_project(root)
        failures += not check("plc_vendor", fp.plc_vendor, "rockwell")
        failures += not check("suggested_profile", fp.suggested_profile, "plc")

        print("\n[6] scoped review takes the profile from the target file")
        failures += not check(
            "scan a .py inside the PLC-tainted repo",
            suggest_profile_for_files(["tests/fixtures/plc_sample/../../../src/app.py"]),
            "python",
        )
        failures += not check(
            "scan the .st itself", suggest_profile_for_files(["pou/Motor.st"]), "plc"
        )
        failures += not check(
            "unknown extension falls back to directory fingerprint",
            suggest_profile_for_files(["notes.md"]),
            None,
        )
        failures += not check(
            "empty scope falls back too", suggest_profile_for_files([]), None
        )

    print("\n[7] every vendor _detect_plc_vendor can name routes to a profile")
    missing = [v for v in _PLC_XML_SIGS if f"plc_{v}" not in _LANG_PROFILE]
    failures += not check("vendors missing from _LANG_PROFILE", missing, [])

    print("\n[8] vendor signatures carry no generic XML fragments")
    banned = ("project xmlns", "xmlns", "<project")
    loose = [
        (v, m) for v, markers in _PLC_XML_SIGS.items() for m in markers if m in banned
    ]
    failures += not check("over-broad markers", loose, [])

    if failures:
        print(f"\n{failures} detection smoke check(s) FAILED.")
        return 1
    print("\nAll detection smoke checks PASSED.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
