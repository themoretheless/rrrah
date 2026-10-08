#!/usr/bin/env python3
"""Create an external diagnostic PDF with an explicit DefaultCMYK profile.

Requires pypdf. This changes color semantics and must never replace the source
or be used as proof that the original AI file passes qualification.
"""
import argparse
from pathlib import Path
from pypdf import PdfReader, PdfWriter
from pypdf.generic import ArrayObject, DecodedStreamObject, DictionaryObject, NameObject, NumberObject


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--profile", type=Path, default=Path(__file__).resolve().parents[1] / "vendor/hayro-interpret/assets/CGATS001Compat-v2-micro.icc")
    args = parser.parse_args()
    if args.output.exists() or args.output.resolve() == args.source.resolve():
        raise ValueError("diagnostic output must be a new file")
    writer = PdfWriter()
    writer.clone_document_from_reader(PdfReader(args.source))
    profile = DecodedStreamObject()
    profile.set_data(args.profile.read_bytes())
    profile[NameObject("/N")] = NumberObject(4)
    color_space = ArrayObject([NameObject("/ICCBased"), writer._add_object(profile)])
    seen = set()
    count = 0

    def visit(obj):
        nonlocal count
        obj = obj.get_object() if hasattr(obj, "get_object") else obj
        if id(obj) in seen:
            return
        seen.add(id(obj))
        if isinstance(obj, dict):
            resources = obj.get("/Resources")
            if resources is not None:
                resources = resources.get_object()
                spaces = resources.get("/ColorSpace")
                if spaces is None:
                    spaces = DictionaryObject()
                    resources[NameObject("/ColorSpace")] = spaces
                spaces.get_object()[NameObject("/DefaultCMYK")] = color_space
                count += 1
            for value in list(obj.values()):
                visit(value)
        elif isinstance(obj, (list, tuple)):
            for value in obj:
                visit(value)

    for page in writer.pages:
        visit(page)
    with args.output.open("xb") as output:
        writer.write(output)
    print(f"DefaultCMYK added to {count} resource dictionaries")


if __name__ == "__main__":
    main()
