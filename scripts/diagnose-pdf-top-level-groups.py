#!/usr/bin/env python3
"""Compare independent top-level q/Q groups with Poppler (diagnostic only).

Requires pypdf. Rejects drawing/state operators outside balanced q/Q groups;
marked-content wrappers are omitted. Isolated layers change compositing context
and do not establish source-file qualification.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
from pypdf import PdfReader, PdfWriter
from pypdf.generic import ContentStream, NameObject


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("--dump", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    args.output_dir.mkdir(exist_ok=False)
    reader = PdfReader(args.source)
    operations = ContentStream(reader.pages[0].get_contents(), reader).operations
    depth = 0
    groups = []
    start = None
    for index, (_, operator) in enumerate(operations):
        if operator == b"q":
            if depth == 0:
                start = index
            depth += 1
        elif operator == b"Q":
            depth -= 1
            if depth < 0:
                raise ValueError("unbalanced graphics state")
            if depth == 0:
                groups.append((start, index + 1))
        elif depth == 0 and operator not in (b"BDC", b"BMC", b"EMC"):
            raise ValueError(f"state or drawing outside groups: {operator!r}")
    if depth:
        raise ValueError("unbalanced graphics state")
    rows = []
    for index, (start, end) in enumerate(groups):
        writer = PdfWriter()
        writer.clone_document_from_reader(PdfReader(args.source))
        content = ContentStream(None, writer)
        content.operations = operations[start:end]
        writer.pages[0][NameObject("/Contents")] = writer._add_object(content)
        prefix = args.output_dir / f"group-{index:02}"
        pdf = prefix.with_suffix(".pdf")
        with pdf.open("xb") as output:
            writer.write(output)
        metadata = json.loads(subprocess.check_output(
            [str(args.dump.resolve()), str(pdf.resolve()), "0", str(prefix.with_suffix(".rgba"))], text=True))
        subprocess.run(["pdftoppm", "-r", "72", "-singlefile", str(pdf), str(prefix)], check=True)
        width, height = metadata["width"], metadata["height"]
        header, oracle = prefix.with_suffix(".ppm").read_bytes().split(b"\n255\n", 1)
        native = prefix.with_suffix(".rgba").read_bytes()
        if header != f"P6\n{width} {height}".encode() or len(native) != width * height * 4 or len(oracle) != width * height * 3:
            raise ValueError("incompatible render payload")
        actual = bytes((c*a + 255*(255-a)+127)//255 for r,g,b,a in zip(native[0::4], native[1::4], native[2::4], native[3::4]) for c in (r,g,b))
        errors = [abs(a-b) for a,b in zip(actual, oracle)]
        row = {"group": index, "operators": end-start,
               "resources": [[str(a), op.decode()] for a,op in operations[start:end] if op in (b"Do", b"sh", b"gs")],
               "pdf_sha256": hashlib.sha256(pdf.read_bytes()).hexdigest(),
               "different_pixels": sum(any(errors[i:i+3]) for i in range(0,len(errors),3)),
               "max_channel_error": max(errors), "mean_absolute_channel_error": sum(errors)/len(errors),
               "pixels_above_32": sum(max(errors[i:i+3])>32 for i in range(0,len(errors),3))}
        rows.append(row)
        print(index, row["different_pixels"], row["max_channel_error"], round(row["mean_absolute_channel_error"],4), flush=True)
    report = {"scope": "First-page independent graphics-state groups on white; diagnostics change layer context and cannot qualify source.",
              "source_sha256": hashlib.sha256(args.source.read_bytes()).hexdigest(), "groups": rows}
    args.report.write_text(json.dumps(report, indent=2)+"\n")


if __name__ == "__main__":
    main()
