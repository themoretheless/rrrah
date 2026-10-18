#!/usr/bin/env python3
"""Inspect catalogs of the two exact pinned, non-object-stream AI fixtures only."""
import argparse
import hashlib
import json
from pathlib import Path
import re

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('corpus', type=Path)
parser.add_argument('report', type=Path)
args = parser.parse_args()
root = Path(__file__).resolve().parents[3]
manifest = json.loads((root / 'docs/research/ai-upstream-poppler-2026-10-06.json').read_text())
rows = []
for source in manifest['sources']:
    data = (args.corpus / source['name']).read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    if digest != source['sha256'] or len(data) != source['bytes']:
        raise ValueError('Pinned source identity mismatch')
    # These exact fixtures use a plain first-object Catalog, not an ObjStm.
    # This deliberately refuses other layouts instead of acting as a PDF parser.
    if b'/ObjStm' in data:
        raise ValueError('Object streams require a real PDF parser')
    match = re.search(rb'1\s+0\s+obj\s*(<<.*?>>)\s*endobj', data, re.S)
    if match is None:
        raise ValueError('Expected pinned catalog object 1 0')
    catalog = match[1]
    if b'/Type/Catalog' not in catalog or b'#' in catalog:
        raise ValueError('Unexpected catalog or escaped name; refusing lexical inference')
    rows.append({'file': source['name'], 'source_sha256': digest,
                 'source_bytes': len(data),
                 'catalog_sha256': hashlib.sha256(catalog).hexdigest(),
                 'catalog_bytes': len(catalog),
                 'has_output_intents': b'/OutputIntents' in catalog,
                 'has_object_streams': False})
args.report.write_text(json.dumps({'scope': 'Exact two pinned plain Catalogs only; no general PDF parser or color qualification', 'sources': rows}, indent=2) + '\n')
