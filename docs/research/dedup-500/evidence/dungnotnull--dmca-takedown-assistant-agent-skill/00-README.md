# dmca-takedown-assistant

**Copyright Infringement Detection and DMCA Takedown Assistant Across Border Platforms**

[![Claude Skill](https://img.shields.io/badge/Claude-Skill-blue)](https://claude.ai/claude-code)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Python 3.11+](https://img.shields.io/badge/python-3.11+-blue.svg)](https://www.python.org/downloads/)
[![Status: Production Ready](https://img.shields.io/badge/status-production--ready-brightgreen)](#)
[![Version: 2.0.0](https://img.shields.io/badge/version-2.0.0-orange.svg)](#)

A professional-grade Claude Code harness for **Digital Copyright Enforcement
and DMCA Takedown Automation** - gathers real-time authoritative data, applies
recognized domain methods (perceptual hashing, jurisdiction classification,
jurisdiction-correct notice generation, tamper-evident evidence capture),
integrates academic research, and delivers evidence-backed, risk-disclosed
outputs. A deterministic, offline-capable Python toolchain keeps the
evidence court-defensible and reproducible.

## Features
- Real-time data aggregation from authoritative copyright sources (WIPO,
  US Copyright Office, EU DSM, Lumen, platform agents).
- Perceptual + exact-hash infringement detection (pHash, dHash, aHash,
  SHA-256) with confidence scoring and video keyframe support.
- Jurisdiction resolution for 10 jurisdictions (US DMCA, EU DSM, UK, DE, FR,
  JP, VN, BR, CA, AU) plus a Berne default fallback.
- Jurisdiction-correct takedown notice generation enforcing all 17 USC
  512(c)(3)(A) elements (gate G1).
- Tamper-evident evidence capture with HMAC-SHA256 chain-of-custody sealing
  (gate G4).
- Self-improving knowledge pipeline (weekly academic + daily news crawl).
- 10 quality gates (U1-U6 universal + G1-G4 domain) with auto-fix and
  graceful degradation.
- Full unit-test and structural-validation suite.

## Installation
```bash
pip install -r requirements.txt
```
Install skill files to `~/.claude/skills/` or use via project CLAUDE.md.

## Usage
```bash
# Invoke the analysis harness in Claude Code
/dmca-takedown-assistant [your query]
```

### Python CLI
```bash
# Detect infringement
python tools/dmca_cli.py detect --query ./asset.png --candidate ./suspect.png

# Resolve jurisdiction for infringing URLs
python tools/dmca_cli.py classify https://example.co.uk/page

# Generate a jurisdiction-correct takedown notice from a JSON spec
python tools/dmca_cli.py notice notice_spec.json

# Capture + seal evidence with chain of custody
python tools/dmca_cli.py capture https://example.com/infringing.png --case-id my-case

# Verify a saved evidence case
python tools/dmca_cli.py verify my-case

# End-to-end: detect -> classify -> notice -> capture
python tools/dmca_cli.py pipeline case_spec.json
```

See `examples/` (or `tools/dmca_cli.py --help`) for JSON spec formats.

## Architecture
Harness flow: requirements -> evidence -> core analysis -> knowledge ->
synthesis -> quality gate. See `PROJECT-detail.md` for the full architecture
diagram. The core analysis step drives the Python toolchain in `tools/`.

## Quality Gates
Universal gates U1-U6 plus domain gates G1-G4 defined in `skills/main.md`.

## Data Sources
- WIPO - wipo.int (copyright treaties, Berne Convention, WCT, WPPT)
- US Copyright Office - copyright.gov (DMCA 17 USC 512)
- EU DSM Directive (Art. 17) / E-Commerce Directive
- Lumen Database - lumendatabase.org (takedown archive)
- DMCA.com / takedown templates
- Google DMCA / Transparency Report
- National copyright offices (per-country procedures)

## Testing
```bash
python tools/test_knowledge_updater.py     # crawl pipeline unit tests
python tools/test_dmca_suite.py             # enforcement toolchain unit tests
python tools/validate_project.py            # 8-File Contract + toolchain
python tools/run_test_scenarios.py --all    # full structural + unit-suite
```

## Knowledge Base
`SECOND-KNOWLEDGE-BRAIN.md` is auto-updated weekly via
`tools/knowledge_updater.py`. Never hand-edit Section 7; preview with
`python tools/knowledge_updater.py --dry-run`.

## Roadmap
- [x] Phase 0: Architecture
- [x] Phase 1: Core sub-skills
- [x] Phase 2: Main harness + gates
- [x] Phase 3: Knowledge pipeline
- [x] Phase 4: Testing
- [x] Phase 5: Integration and polish
- [x] Production toolchain (detector, classifier, notice generator, evidence collector, CLI)

## License
MIT - see [LICENSE](LICENSE).

## Contributing
See [CONTRIBUTING.md](CONTRIBUTING.md). Run all validators before opening a PR.

## Citation
```bibtex
@software{dmca-takedown-assistant,
  title = {dmca-takedown-assistant: Copyright Infringement Detection and DMCA Takedown Assistant Across Border Platforms},
  year = {2026},
  version = {1.1.0}
}
```

## Why This Skill

Digital Copyright Enforcement and DMCA Takedown Automation practitioners face
fragmented data, inconsistent methodology, and tools that do not self-improve.
This skill unifies authoritative real-time data, recognized domain methods,
a court-defensible evidence chain, and a continuously-updated academic
knowledge base into one evidence-backed, risk-disclosed workflow.
