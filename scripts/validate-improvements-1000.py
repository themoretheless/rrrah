#!/usr/bin/env python3
"""Validate the rrrah improvement backlog without running its proposed work."""

import argparse
import collections
import hashlib
import json
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
REQUIRED = {
    "id", "title", "subsystem", "type", "evidence_status", "evidence",
    "proposed_change", "acceptance", "priority", "size", "dependencies",
    "verification_conditions", "public_api",
}


def validate(data, markdown, root=ROOT, strict_snapshot=False):
    errors, warnings = [], []
    tasks = data.get("tasks", [])
    expected = [f"RR-{i:04d}" for i in range(1, 1001)]
    if len(tasks) != 1000:
        errors.append(f"expected 1000 tasks, got {len(tasks)}")
    ids = [t.get("id") for t in tasks]
    if ids != expected:
        errors.append("IDs must be unique and ordered RR-0001 through RR-1000")
    if data.get("schema_version") != 1:
        errors.append("unsupported schema_version")
    if data.get("first_30") != expected[:30]:
        errors.append("first_30 must contain the first thirty task IDs")
    if data.get("execution_schema_version") is not None:
        if data["execution_schema_version"] != 1:
            errors.append("unsupported execution_schema_version")
        if any(not isinstance(t.get("execution"), dict) for t in tasks[:30]):
            errors.append("first 30 tasks require execution metadata")
        counts = collections.Counter(
            t.get("execution", {}).get("status") for t in tasks[:30]
            if isinstance(t.get("execution"), dict)
        )
        if dict(counts) != data.get("execution_summary"):
            errors.append("execution_summary inconsistent")
    titles, outcomes = set(), set()
    graph = {}
    source_cache = {}
    blocks = {
        match.group(1).upper(): match.group(2)
        for match in re.finditer(
            r'<a id="(rr-\d{4})"></a>(.*?)(?=<a id="rr-\d{4}"></a>|\Z)',
            markdown,
            re.DOTALL,
        )
    }
    for task in tasks:
        ident = task.get("id", "<missing>")
        execution = task.get("execution")
        if execution is not None:
            if not isinstance(execution, dict) or execution.get("status") not in {
                "pending", "verified", "partial", "blocked", "reviewed", "rejected", "measured_no_change"
            }:
                errors.append(f"{ident}: invalid execution status")
            else:
                for key in ("updated_at", "result", "remaining", "report"):
                    if not isinstance(execution.get(key), str) or not execution[key].strip():
                        errors.append(f"{ident}: missing execution {key}")
                refs = execution.get("evidence_files")
                if not isinstance(refs, list):
                    errors.append(f"{ident}: invalid execution evidence_files")
                    refs = []
                for ref in refs + [execution.get("report", "")]:
                    path = (root / ref).resolve()
                    if not path.is_relative_to(root.resolve()) or not path.is_file():
                        errors.append(f"{ident}: missing/unsafe execution evidence {ref}")
        missing = REQUIRED - task.keys()
        if missing:
            errors.append(f"{ident}: missing fields {sorted(missing)}")
            continue
        for key in REQUIRED - {"evidence", "dependencies", "verification_conditions"}:
            if not isinstance(task[key], str) or not task[key].strip():
                errors.append(f"{ident}: empty/non-string {key}")
        for key, allowed in {
            "type": {"исправление", "оптимизация", "улучшение"},
            "evidence_status": {"подтверждено", "требует воспроизведения"},
            "priority": {"P0", "P1", "P2", "P3"},
            "size": {"S", "M", "L"},
        }.items():
            if task[key] not in allowed:
                errors.append(f"{ident}: invalid {key}")
        title = " ".join(task["title"].casefold().split())
        if title in titles:
            errors.append(f"{ident}: duplicate normalized title")
        titles.add(title)
        dependencies = task["dependencies"]
        if not isinstance(dependencies, list) or any(
            dep not in expected or dep == ident for dep in dependencies
        ) or len(set(dependencies)) != len(dependencies):
            errors.append(f"{ident}: invalid dependencies")
            dependencies = []
        graph[ident] = dependencies
        conditions = task["verification_conditions"]
        if not isinstance(conditions, list) or not conditions or any(
            not isinstance(c, str) or not c.strip() for c in conditions
        ):
            errors.append(f"{ident}: invalid verification_conditions")
        evidence = task["evidence"]
        if not isinstance(evidence, dict) or any(
            not evidence.get(key) for key in ("path", "symbol", "line", "excerpt", "observation")
        ):
            errors.append(f"{ident}: incomplete evidence")
            continue
        path = (root / evidence["path"]).resolve()
        if not path.is_relative_to(root.resolve()) or not path.is_file():
            errors.append(f"{ident}: missing/unsafe source path {evidence['path']}")
            continue
        if path not in source_cache:
            source_cache[path] = path.read_text(encoding="utf-8").splitlines()
        lines = source_cache[path]
        line = evidence["line"]
        if not isinstance(line, int) or not 1 <= line <= len(lines):
            errors.append(f"{ident}: source line out of range")
        elif lines[line - 1].strip() != evidence["excerpt"]:
            errors.append(f"{ident}: source excerpt drift at {evidence['path']}:{line}")
        symbol = evidence["symbol"]
        if re.fullmatch(r"[A-Za-z_]\w*", symbol):
            language = path.suffix
            pattern = rf"\b{'def' if language == '.py' else 'fn'}\s+{re.escape(symbol)}\b"
            if not re.search(pattern, "\n".join(lines)):
                errors.append(f"{ident}: symbol not found: {symbol}")
        if evidence.get("signal"):
            outcome = (evidence["path"], symbol)
            if outcome in outcomes:
                errors.append(f"{ident}: repeated function-level outcome {outcome}")
            outcomes.add(outcome)
        if evidence["path"] not in data.get("source_sha256", {}):
            errors.append(f"{ident}: no source fingerprint")
        block = blocks.get(ident, "")
        if execution is not None and isinstance(execution, dict):
            if f"Выполнение: {execution.get('status')}" not in block or execution.get("result", "") not in block:
                errors.append(f"{ident}: Markdown execution inconsistent")
            report = execution.get("report")
            if isinstance(report, str) and Path(report).is_relative_to("docs"):
                report_link = f"[Отчёт]({Path(report).relative_to('docs').as_posix()})"
                if report_link not in block:
                    errors.append(f"{ident}: Markdown report link inconsistent")
        if f"### {ident} — {task['title']}" not in block:
            errors.append(f"{ident}: Markdown title missing or inconsistent")
        for key in ("proposed_change", "acceptance", "public_api"):
            if task[key] not in block:
                errors.append(f"{ident}: Markdown {key} inconsistent")
        expected_link = f"../{evidence['path']}#L{line}"
        if expected_link not in block:
            errors.append(f"{ident}: Markdown source link inconsistent")
    visiting, done = set(), set()

    def visit(ident):
        if ident in visiting:
            errors.append(f"dependency cycle at {ident}")
            return
        if ident in done:
            return
        visiting.add(ident)
        for dep in graph.get(ident, []):
            visit(dep)
        visiting.remove(ident)
        done.add(ident)

    for ident in graph:
        visit(ident)
    anchors = re.findall(r'<a id="(rr-\d{4})"></a>', markdown)
    if collections.Counter(anchors) != collections.Counter(i.lower() for i in expected):
        errors.append("Markdown must contain each task anchor exactly once")
    for relative, digest in data.get("source_sha256", {}).items():
        path = (root / relative).resolve()
        if not path.is_relative_to(root.resolve()) or not path.is_file():
            errors.append(f"missing/unsafe fingerprint path: {relative}")
            continue
        if hashlib.sha256(path.read_bytes()).hexdigest() != digest:
            message = f"source snapshot changed: {relative}"
            (errors if strict_snapshot else warnings).append(message)
    return errors, warnings


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--strict-snapshot", action="store_true")
    args = parser.parse_args()
    try:
        data = json.loads((ROOT / "docs/improvements-1000.json").read_text(encoding="utf-8"))
        markdown = (ROOT / "docs/IMPROVEMENTS_1000.md").read_text(encoding="utf-8")
        errors, warnings = validate(data, markdown, strict_snapshot=args.strict_snapshot)
    except (OSError, ValueError, TypeError, KeyError, RecursionError) as exc:
        print(f"ERROR: invalid registry: {exc}", file=sys.stderr)
        return 1
    for message in warnings:
        print(f"WARNING: {message}", file=sys.stderr)
    for message in errors:
        print(f"ERROR: {message}", file=sys.stderr)
    if errors:
        return 1
    print(f"OK: 1000 tasks, 30 initial tasks, source links, fields, dependencies, Markdown parity; {len(warnings)} snapshot warnings")
    print(json.dumps(dict(collections.Counter(t["subsystem"] for t in data["tasks"])), ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
