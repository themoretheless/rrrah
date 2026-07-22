"""Independent verifier for the checked-in V2 and V3 cache fixtures.

The encoder and parser intentionally do not import rrrah or invoke Cargo.  The
standard-library checks always validate the manifest schema, complete section
layout, wire structure, and SHA-256 digests.  Protocol BLAKE3 verification is a
required release gate when ``--require-blake3`` is used; without the optional
Python package the local result is explicitly marked non-release.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import struct
import sys
from pathlib import Path
from typing import Any, Callable


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "crates" / "rrrah-cache" / "tests" / "fixtures"
MANIFEST_PATH = FIXTURES / "v3_mosaic_v1_manifest.json"
LEGACY_MANIFEST_PATH = FIXTURES / "legacy_v2_mosaic_manifest.json"

MANIFEST_SCHEMA = "rrrah.cache.conformance.v1"
V3_SECTIONS = frozenset(("envelope", "semantic_descriptor", "mosaic_payload"))
LEGACY_SECTIONS = frozenset(("prefix", "json_header", "pixels"))
_CASE_ID = re.compile(r"[a-z0-9][a-z0-9-]*\Z")


def _fail(path: str, message: str) -> None:
    raise ValueError(f"{path}: {message}")


def _object(value: Any, path: str, keys: set[str] | frozenset[str]) -> dict[str, Any]:
    if type(value) is not dict:
        _fail(path, "must be an object")
    actual = set(value)
    missing = sorted(keys - actual)
    unknown = sorted(actual - keys)
    if missing or unknown:
        details = []
        if missing:
            details.append(f"missing fields {missing}")
        if unknown:
            details.append(f"unknown fields {unknown}")
        _fail(path, "; ".join(details))
    return value


def _list(value: Any, path: str) -> list[Any]:
    if type(value) is not list:
        _fail(path, "must be an array")
    return value


def _string(value: Any, path: str) -> str:
    if type(value) is not str:
        _fail(path, "must be a string")
    return value


def _literal(value: Any, expected: str, path: str) -> None:
    if _string(value, path) != expected:
        _fail(path, f"must equal {expected!r}")


def _decimal(value: Any, path: str, *, maximum: int = (1 << 64) - 1) -> int:
    encoded = _string(value, path)
    if not encoded or not encoded.isascii() or not encoded.isdecimal():
        _fail(path, "must be an unsigned decimal string")
    if len(encoded) > 1 and encoded[0] == "0":
        _fail(path, "must use canonical decimal encoding")
    decoded = int(encoded)
    if decoded > maximum:
        _fail(path, f"exceeds {maximum}")
    return decoded


def _hex(value: Any, size: int | None = None, path: str = "hex") -> bytes:
    encoded = _string(value, path)
    if len(encoded) % 2 != 0:
        _fail(path, "must contain complete bytes")
    if encoded != encoded.lower() or any(character not in "0123456789abcdef" for character in encoded):
        _fail(path, "must be canonical lowercase hex")
    decoded = bytes.fromhex(encoded)
    if size is not None and len(decoded) != size:
        _fail(path, f"expected {size} bytes, got {len(decoded)}")
    return decoded


def _unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON field {key!r}")
        result[key] = value
    return result


def _reject_json_constant(value: str) -> None:
    raise ValueError(f"non-standard JSON constant {value!r}")


def _decode_json(text: str) -> Any:
    return json.loads(
        text,
        object_pairs_hook=_unique_object,
        parse_constant=_reject_json_constant,
    )


def _validate_artifact(artifact: Any, path: str, *, v3: bool) -> int:
    fields = {"file", "encoding", "decoded_length_dec", "sha256"}
    if v3:
        fields.add("blake3_unkeyed")
    artifact = _object(artifact, path, fields)
    filename = _string(artifact["file"], f"{path}.file")
    if filename in {"", ".", ".."} or Path(filename).name != filename:
        _fail(f"{path}.file", "must be a plain fixture filename")
    _literal(artifact["encoding"], "lowercase-hex-final-lf", f"{path}.encoding")
    decoded_length = _decimal(artifact["decoded_length_dec"], f"{path}.decoded_length_dec")
    _hex(artifact["sha256"], 32, f"{path}.sha256")
    if v3:
        _hex(artifact["blake3_unkeyed"], 32, f"{path}.blake3_unkeyed")
    return decoded_length


def _validate_sections(sections: Any, artifact_bytes: int, expected_names: frozenset[str]) -> None:
    sections = _list(sections, "$.sections")
    if len(sections) != len(expected_names):
        _fail("$.sections", f"must contain exactly {len(expected_names)} sections")
    parsed: list[tuple[int, int, str]] = []
    names: set[str] = set()
    for index, raw_section in enumerate(sections):
        path = f"$.sections[{index}]"
        section = _object(raw_section, path, {"name", "offset_dec", "length_dec", "sha256"})
        name = _string(section["name"], f"{path}.name")
        if name in names:
            _fail(f"{path}.name", f"duplicate section name {name!r}")
        names.add(name)
        offset = _decimal(section["offset_dec"], f"{path}.offset_dec")
        length = _decimal(section["length_dec"], f"{path}.length_dec")
        if length == 0:
            _fail(f"{path}.length_dec", "section must not be empty")
        end = offset + length
        if end > artifact_bytes:
            _fail(path, f"section [{offset}, {end}) exceeds artifact length {artifact_bytes}")
        _hex(section["sha256"], 32, f"{path}.sha256")
        parsed.append((offset, end, name))
    if names != expected_names:
        _fail("$.sections", f"names must equal {sorted(expected_names)}, got {sorted(names)}")
    cursor = 0
    for offset, end, name in sorted(parsed):
        if offset < cursor:
            _fail("$.sections", f"section {name!r} overlaps the preceding section")
        if offset > cursor:
            _fail("$.sections", f"uncovered byte range [{cursor}, {offset})")
        cursor = end
    if cursor != artifact_bytes:
        _fail("$.sections", f"uncovered byte range [{cursor}, {artifact_bytes})")


def _validate_protocol(protocol: Any) -> None:
    protocol = _object(
        protocol,
        "$.protocol",
        {
            "byte_order",
            "envelope_version_dec",
            "payload_schema_id_dec",
            "payload_schema_version_dec",
            "payload_wire_major_dec",
            "payload_wire_minor_dec",
            "artifact_key_context",
            "payload_digest_context",
            "envelope_digest_context",
        },
    )
    _literal(protocol["byte_order"], "little", "$.protocol.byte_order")
    numeric = {
        "envelope_version_dec": 1,
        "payload_schema_id_dec": 1,
        "payload_schema_version_dec": 1,
        "payload_wire_major_dec": 1,
        "payload_wire_minor_dec": 0,
    }
    for field, expected in numeric.items():
        actual = _decimal(protocol[field], f"$.protocol.{field}")
        if actual != expected:
            _fail(f"$.protocol.{field}", f"unsupported version/value {actual}")
    _literal(
        protocol["artifact_key_context"],
        "rrrah.cache.artifact-key.v1",
        "$.protocol.artifact_key_context",
    )
    _literal(
        protocol["payload_digest_context"],
        "rrrah.cache.object-payload.v1",
        "$.protocol.payload_digest_context",
    )
    _literal(
        protocol["envelope_digest_context"],
        "rrrah.cache.object-envelope.v1",
        "$.protocol.envelope_digest_context",
    )


def _validate_v3_expected(expected: Any) -> None:
    expected = _object(
        expected,
        "$.expected",
        {
            "artifact_kind_dec",
            "source_id_hex",
            "image_index_dec",
            "artifact_variant_hex",
            "recipe_id_hex",
            "artifact_key_hex",
            "payload_digest_hex",
            "envelope_digest_hex",
            "metadata_prefix_bytes_dec",
            "make_utf8_hex",
            "model_utf8_hex",
            "pixels_u16_dec",
        },
    )
    if _decimal(expected["artifact_kind_dec"], "$.expected.artifact_kind_dec", maximum=0xFFFF) != 1:
        _fail("$.expected.artifact_kind_dec", "must identify the sensor mosaic artifact kind")
    _decimal(expected["image_index_dec"], "$.expected.image_index_dec")
    for field in (
        "source_id_hex",
        "artifact_variant_hex",
        "recipe_id_hex",
        "artifact_key_hex",
        "payload_digest_hex",
        "envelope_digest_hex",
    ):
        _hex(expected[field], 32, f"$.expected.{field}")
    _decimal(expected["metadata_prefix_bytes_dec"], "$.expected.metadata_prefix_bytes_dec", maximum=1 << 20)
    for field in ("make_utf8_hex", "model_utf8_hex"):
        encoded = _hex(expected[field], path=f"$.expected.{field}")
        try:
            encoded.decode("utf-8")
        except UnicodeDecodeError as error:
            _fail(f"$.expected.{field}", f"is not UTF-8: {error}")
    pixels = _list(expected["pixels_u16_dec"], "$.expected.pixels_u16_dec")
    if len(pixels) != 4:
        _fail("$.expected.pixels_u16_dec", "this conformance case requires four pixels")
    for index, value in enumerate(pixels):
        _decimal(value, f"$.expected.pixels_u16_dec[{index}]", maximum=0xFFFF)


def _validate_legacy_expected(expected: Any) -> None:
    expected = _object(
        expected,
        "$.expected",
        {"stored_schema_dec", "cache_key_hex", "payload_blake3_unkeyed", "pixels_u16_dec"},
    )
    if _decimal(expected["stored_schema_dec"], "$.expected.stored_schema_dec") != 1:
        _fail("$.expected.stored_schema_dec", "unsupported legacy stored schema")
    _hex(expected["cache_key_hex"], 32, "$.expected.cache_key_hex")
    _hex(expected["payload_blake3_unkeyed"], 32, "$.expected.payload_blake3_unkeyed")
    pixels = _list(expected["pixels_u16_dec"], "$.expected.pixels_u16_dec")
    if not pixels:
        _fail("$.expected.pixels_u16_dec", "must not be empty")
    for index, value in enumerate(pixels):
        _decimal(value, f"$.expected.pixels_u16_dec[{index}]", maximum=0xFFFF)


def validate_manifest(manifest: Any, kind: str = "v3") -> dict[str, Any]:
    """Validate all fields and reject unknown fields for a supported manifest."""

    if kind not in {"v3", "legacy_v2"}:
        raise ValueError(f"unsupported manifest kind {kind!r}")
    top_fields = {"manifest_schema", "case_id", "class", "license", "artifact", "sections", "expected"}
    if kind == "v3":
        top_fields.add("protocol")
    manifest = _object(manifest, "$", top_fields)
    _literal(manifest["manifest_schema"], MANIFEST_SCHEMA, "$.manifest_schema")
    case_id = _string(manifest["case_id"], "$.case_id")
    if _CASE_ID.fullmatch(case_id) is None:
        _fail("$.case_id", "must use canonical lowercase kebab-case")
    _literal(manifest["class"], "synthetic", "$.class")
    _literal(manifest["license"], "CC0-1.0", "$.license")
    artifact_bytes = _validate_artifact(manifest["artifact"], "$.artifact", v3=kind == "v3")
    _validate_sections(
        manifest["sections"],
        artifact_bytes,
        V3_SECTIONS if kind == "v3" else LEGACY_SECTIONS,
    )
    if kind == "v3":
        _validate_protocol(manifest["protocol"])
        _validate_v3_expected(manifest["expected"])
    else:
        _validate_legacy_expected(manifest["expected"])
    return manifest


def _load_manifest(path: Path, kind: str) -> dict[str, Any]:
    manifest = _decode_json(path.read_text(encoding="utf-8"))
    return validate_manifest(manifest, kind)


def load_manifest(path: Path = MANIFEST_PATH) -> dict[str, Any]:
    return _load_manifest(path, "v3")


def load_legacy_manifest(path: Path = LEGACY_MANIFEST_PATH) -> dict[str, Any]:
    return _load_manifest(path, "legacy_v2")


def load_object(manifest: dict[str, Any]) -> bytes:
    filename = _string(manifest["artifact"]["file"], "$.artifact.file")
    if filename in {"", ".", ".."} or Path(filename).name != filename:
        _fail("$.artifact.file", "must be a plain fixture filename")
    encoded = (FIXTURES / filename).read_text(encoding="ascii")
    if not encoded.endswith("\n") or encoded.count("\n") != 1:
        raise ValueError("fixture must be one canonical lowercase-hex line with a final LF")
    return _hex(encoded[:-1], path="fixture")


def _sections(manifest: dict[str, Any], artifact: bytes) -> dict[str, bytes]:
    result: dict[str, bytes] = {}
    for section in manifest["sections"]:
        offset = _decimal(section["offset_dec"], f"section {section['name']} offset")
        length = _decimal(section["length_dec"], f"section {section['name']} length")
        result[section["name"]] = artifact[offset : offset + length]
    return result


def _verify_sha256_manifest(manifest: dict[str, Any], artifact: bytes) -> str:
    declared_bytes = _decimal(manifest["artifact"]["decoded_length_dec"], "$.artifact.decoded_length_dec")
    if len(artifact) != declared_bytes:
        raise ValueError("object length differs from manifest")
    sha256 = hashlib.sha256(artifact).hexdigest()
    if sha256 != manifest["artifact"]["sha256"]:
        raise ValueError("object SHA-256 differs from manifest")
    for section in manifest["sections"]:
        offset = _decimal(section["offset_dec"], f"section {section['name']} offset")
        length = _decimal(section["length_dec"], f"section {section['name']} length")
        actual = hashlib.sha256(artifact[offset : offset + length]).hexdigest()
        if actual != section["sha256"]:
            raise ValueError(f"section {section['name']} SHA-256 differs")
    return sha256


def _u16(data: bytes, offset: int) -> int:
    return struct.unpack_from("<H", data, offset)[0]


def _u32(data: bytes, offset: int) -> int:
    return struct.unpack_from("<I", data, offset)[0]


def _u64(data: bytes, offset: int) -> int:
    return struct.unpack_from("<Q", data, offset)[0]


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def parse_v3_object(manifest: dict[str, Any], artifact: bytes) -> dict[str, Any]:
    """Parse and validate the object independently from ``build_object``."""

    parts = _sections(manifest, artifact)
    envelope = parts["envelope"]
    descriptor = parts["semantic_descriptor"]
    payload = parts["mosaic_payload"]
    protocol = manifest["protocol"]
    expected = manifest["expected"]

    _require(len(envelope) == 136, "V3 envelope must be 136 bytes")
    _require(envelope[:8] == b"RRRAHOBJ", "V3 envelope magic differs")
    _require(_u16(envelope, 8) == _decimal(protocol["envelope_version_dec"], "envelope version"), "V3 envelope version differs")
    _require(_u16(envelope, 10) == len(envelope), "V3 envelope header length differs")
    _require(_u32(envelope, 12) == 0, "V3 envelope flags are not canonical zero")
    _require(_u32(envelope, 16) == _decimal(protocol["payload_schema_id_dec"], "schema id"), "V3 payload schema ID differs")
    _require(_u16(envelope, 20) == _decimal(protocol["payload_schema_version_dec"], "schema version"), "V3 payload schema version differs")
    _require(envelope[22:24] == bytes(2) and envelope[28:32] == bytes(4), "V3 envelope reserved bytes are non-zero")
    _require(_u32(envelope, 24) == len(descriptor), "V3 descriptor length differs from section")
    _require(_u64(envelope, 32) == len(payload), "V3 payload length differs from section")
    _require(envelope[40:72] == _hex(expected["artifact_key_hex"], 32), "V3 artifact key differs")
    _require(envelope[72:104] == _hex(expected["payload_digest_hex"], 32), "V3 payload digest differs")
    _require(envelope[104:136] == _hex(expected["envelope_digest_hex"], 32), "V3 envelope digest differs")

    _require(len(descriptor) == 106, "V3 mosaic semantic descriptor must be 106 bytes")
    _require(_u16(descriptor, 0) == _decimal(expected["artifact_kind_dec"], "artifact kind"), "V3 artifact kind differs")
    _require(descriptor[2:34] == _hex(expected["source_id_hex"], 32), "V3 source ID differs")
    _require(_u64(descriptor, 34) == _decimal(expected["image_index_dec"], "image index"), "V3 image index differs")
    _require(descriptor[42:74] == _hex(expected["artifact_variant_hex"], 32), "V3 artifact variant differs")
    _require(descriptor[74:106] == _hex(expected["recipe_id_hex"], 32), "V3 recipe ID differs")

    _require(len(payload) >= 192, "V3 mosaic payload is shorter than its fixed header")
    _require(payload[:8] == b"RRMPAY1\0", "V3 mosaic payload magic differs")
    _require(_u16(payload, 8) == _decimal(protocol["payload_wire_major_dec"], "payload major"), "V3 payload major version differs")
    _require(_u16(payload, 10) == _decimal(protocol["payload_wire_minor_dec"], "payload minor"), "V3 payload minor version differs")
    _require(_u32(payload, 12) == 192, "V3 mosaic fixed-header length differs")
    metadata_bytes = _u32(payload, 16)
    _require(metadata_bytes == _decimal(expected["metadata_prefix_bytes_dec"], "metadata prefix"), "V3 metadata-prefix length differs")
    _require(_u32(payload, 20) == 3, "V3 mosaic presence flags differ")
    _require(payload[24:32] == bytes(8) and payload[87:96] == bytes(9), "V3 mosaic reserved bytes are non-zero")
    width, height = _u32(payload, 32), _u32(payload, 36)
    sample_count, pixel_bytes = _u64(payload, 40), _u64(payload, 48)
    make_len, model_len = _u32(payload, 56), _u32(payload, 60)
    cfa_count, black_count, white_count = _u32(payload, 64), _u32(payload, 68), _u32(payload, 72)
    _require((width, height, sample_count, pixel_bytes) == (2, 2, 4, 8), "V3 mosaic dimensions/counts differ")
    _require(_u16(payload, 76) == 1, "V3 mosaic sample format differs")
    _require(tuple(payload[78:87]) == (1, 1, 1, 14, 2, 2, 1, 1, 1), "V3 mosaic enum/grid fields differ")
    _require(struct.unpack_from("<4I", payload, 96) == (0, 0, 2, 2), "V3 active area differs")
    _require(struct.unpack_from("<4I", payload, 112) == (0, 0, 0, 0), "V3 crop-area canonical zeros differ")
    _require(struct.unpack_from("<4I", payload, 128) == (0x40000000, 0x3F800000, 0x3FC00000, 0x3F800000), "V3 white-balance bits differ")
    matrix_bits = (0x3F800000, 0, 0, 0, 0x3F800000, 0, 0, 0, 0x3F800000, 0, 0, 0)
    _require(struct.unpack_from("<12I", payload, 144) == matrix_bits, "V3 color-matrix bits differ")
    _require(width * height * payload[80] == sample_count, "V3 dimensions do not match sample count")
    _require(pixel_bytes == sample_count * 2, "V3 pixel byte count is not U16LE")
    _require(metadata_bytes == 192 + make_len + model_len + cfa_count + 4 * black_count + 4 * white_count, "V3 metadata-prefix arithmetic differs")
    _require(metadata_bytes + pixel_bytes == len(payload), "V3 payload fields do not cover its section")

    cursor = 192
    make = payload[cursor : cursor + make_len]
    cursor += make_len
    model = payload[cursor : cursor + model_len]
    cursor += model_len
    cfa = payload[cursor : cursor + cfa_count]
    cursor += cfa_count
    black = payload[cursor : cursor + 4 * black_count]
    cursor += 4 * black_count
    white = payload[cursor : cursor + 4 * white_count]
    cursor += 4 * white_count
    _require(cursor == metadata_bytes, "V3 metadata cursor differs from declared prefix")
    try:
        make.decode("utf-8")
        model.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ValueError(f"V3 make/model is not UTF-8: {error}") from error
    _require(make == _hex(expected["make_utf8_hex"]), "V3 make bytes differ")
    _require(model == _hex(expected["model_utf8_hex"]), "V3 model bytes differ")
    _require(cfa == bytes((0, 1, 1, 2)), "V3 CFA cells differ")
    _require(struct.unpack("<I", black) == (0x44000000,), "V3 black-level bits differ")
    _require(struct.unpack("<I", white) == (0x467FFC00,), "V3 white-level bits differ")
    pixels = struct.unpack(f"<{sample_count}H", payload[cursor:])
    expected_pixels = tuple(_decimal(value, "expected pixel", maximum=0xFFFF) for value in expected["pixels_u16_dec"])
    _require(pixels == expected_pixels, "V3 U16LE pixels differ")
    return {"descriptor_bytes": len(descriptor), "payload_bytes": len(payload), "pixels": pixels}


def build_descriptor(expected: dict[str, Any]) -> bytes:
    return b"".join(
        (
            struct.pack("<H", _decimal(expected["artifact_kind_dec"], "artifact kind", maximum=0xFFFF)),
            _hex(expected["source_id_hex"], 32),
            struct.pack("<Q", _decimal(expected["image_index_dec"], "image index")),
            _hex(expected["artifact_variant_hex"], 32),
            _hex(expected["recipe_id_hex"], 32),
        )
    )


def build_payload(manifest: dict[str, Any]) -> bytes:
    expected = manifest["expected"]
    protocol = manifest["protocol"]
    make = _hex(expected["make_utf8_hex"])
    model = _hex(expected["model_utf8_hex"])
    cfa = bytes((0, 1, 1, 2))
    black_bits = (0x44000000,)
    white_bits = (0x467FFC00,)
    wb_bits = (0x40000000, 0x3F800000, 0x3FC00000, 0x3F800000)
    matrix_bits = (
        0x3F800000, 0, 0,
        0, 0x3F800000, 0,
        0, 0, 0x3F800000,
        0, 0, 0,
    )
    pixels = tuple(_decimal(value, "expected pixel", maximum=0xFFFF) for value in expected["pixels_u16_dec"])
    metadata_bytes = 192 + len(make) + len(model) + len(cfa) + 4 * len(black_bits) + 4 * len(white_bits)

    header = bytearray(192)
    struct.pack_into(
        "<8sHHIII",
        header,
        0,
        b"RRMPAY1\0",
        _decimal(protocol["payload_wire_major_dec"], "payload major", maximum=0xFFFF),
        _decimal(protocol["payload_wire_minor_dec"], "payload minor", maximum=0xFFFF),
        192,
        metadata_bytes,
        3,
    )
    struct.pack_into("<IIQQ", header, 32, 2, 2, len(pixels), 2 * len(pixels))
    struct.pack_into(
        "<IIIIIH",
        header,
        56,
        len(make),
        len(model),
        len(cfa),
        len(black_bits),
        len(white_bits),
        1,
    )
    header[78:87] = bytes((1, 1, 1, 14, 2, 2, 1, 1, 1))
    struct.pack_into("<IIII", header, 96, 0, 0, 2, 2)
    struct.pack_into("<4I", header, 128, *wb_bits)
    struct.pack_into("<12I", header, 144, *matrix_bits)
    return b"".join(
        (
            bytes(header),
            make,
            model,
            cfa,
            struct.pack("<I", *black_bits),
            struct.pack("<I", *white_bits),
            struct.pack(f"<{len(pixels)}H", *pixels),
        )
    )


def build_object(manifest: dict[str, Any]) -> bytes:
    expected = manifest["expected"]
    protocol = manifest["protocol"]
    descriptor = build_descriptor(expected)
    payload = build_payload(manifest)
    header = bytearray(136)
    struct.pack_into(
        "<8sHHI",
        header,
        0,
        b"RRRAHOBJ",
        _decimal(protocol["envelope_version_dec"], "envelope version", maximum=0xFFFF),
        136,
        0,
    )
    struct.pack_into(
        "<IH",
        header,
        16,
        _decimal(protocol["payload_schema_id_dec"], "schema id", maximum=0xFFFFFFFF),
        _decimal(protocol["payload_schema_version_dec"], "schema version", maximum=0xFFFF),
    )
    struct.pack_into("<I", header, 24, len(descriptor))
    struct.pack_into("<Q", header, 32, len(payload))
    header[40:72] = _hex(expected["artifact_key_hex"], 32)
    header[72:104] = _hex(expected["payload_digest_hex"], 32)
    header[104:136] = _hex(expected["envelope_digest_hex"], 32)
    return bytes(header) + descriptor + payload


def _import_blake3() -> Callable[..., Any]:
    from blake3 import blake3

    return blake3


def _blake3(required: bool) -> Callable[..., Any] | None:
    try:
        return _import_blake3()
    except ImportError as error:
        if required:
            raise RuntimeError(
                "release conformance requires Python blake3; install the CI-pinned dependency"
            ) from error
        return None


def verify_fixture(*, require_blake3: bool = False) -> dict[str, str | bool]:
    manifest = load_manifest()
    artifact = load_object(manifest)
    parse_v3_object(manifest, artifact)
    rebuilt = build_object(manifest)
    if rebuilt != artifact:
        raise ValueError("independent encoder differs from checked-in object")
    sha256 = _verify_sha256_manifest(manifest, artifact)

    blake3 = _blake3(require_blake3)
    if blake3 is None:
        return {
            "sha256": sha256,
            "blake3": "skipped-not-installed",
            "release_verified": False,
        }
    expected = manifest["expected"]
    parts = _sections(manifest, artifact)
    artifact_key_hasher = blake3(derive_key_context=manifest["protocol"]["artifact_key_context"])
    artifact_key_hasher.update(parts["semantic_descriptor"])
    if artifact_key_hasher.hexdigest() != expected["artifact_key_hex"]:
        raise ValueError("artifact-key BLAKE3-DERIVE differs")
    payload_hasher = blake3(derive_key_context=manifest["protocol"]["payload_digest_context"])
    payload_hasher.update(parts["mosaic_payload"])
    if payload_hasher.hexdigest() != expected["payload_digest_hex"]:
        raise ValueError("payload BLAKE3-DERIVE differs")
    envelope_hasher = blake3(derive_key_context=manifest["protocol"]["envelope_digest_context"])
    envelope_hasher.update(parts["envelope"][:104] + parts["semantic_descriptor"])
    if envelope_hasher.hexdigest() != expected["envelope_digest_hex"]:
        raise ValueError("envelope BLAKE3-DERIVE differs")
    if blake3(artifact).hexdigest() != manifest["artifact"]["blake3_unkeyed"]:
        raise ValueError("whole-object unkeyed BLAKE3 differs")
    return {"sha256": sha256, "blake3": "verified", "release_verified": True}


def _byte_array(value: Any, path: str, expected_length: int) -> bytes:
    values = _list(value, path)
    if len(values) != expected_length:
        _fail(path, f"must contain {expected_length} bytes")
    decoded = bytearray()
    for index, item in enumerate(values):
        if type(item) is not int or not 0 <= item <= 255:
            _fail(f"{path}[{index}]", "must be an integer byte")
        decoded.append(item)
    return bytes(decoded)


def parse_legacy_object(manifest: dict[str, Any], artifact: bytes) -> dict[str, Any]:
    parts = _sections(manifest, artifact)
    prefix, header_bytes, pixels_bytes = parts["prefix"], parts["json_header"], parts["pixels"]
    _require(len(prefix) == 12 and prefix[:8] == b"RRRAHRC1", "legacy prefix differs")
    _require(_u32(prefix, 8) == len(header_bytes), "legacy JSON-header length differs")
    try:
        header_text = header_bytes.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ValueError(f"legacy JSON header is not UTF-8: {error}") from error
    header = _object(
        _decode_json(header_text),
        "legacy header",
        {"schema", "key", "pixel_count", "payload_blake3", "metadata"},
    )
    expected = manifest["expected"]
    schema = header["schema"]
    pixel_count = header["pixel_count"]
    _require(type(schema) is int, "legacy schema must be an integer")
    _require(type(pixel_count) is int and pixel_count >= 0, "legacy pixel_count must be an unsigned integer")
    _require(schema == _decimal(expected["stored_schema_dec"], "legacy schema"), "legacy stored schema differs")
    _require(_byte_array(header["key"], "legacy header.key", 32) == _hex(expected["cache_key_hex"], 32), "legacy key differs")
    _require(
        _byte_array(header["payload_blake3"], "legacy header.payload_blake3", 32)
        == _hex(expected["payload_blake3_unkeyed"], 32),
        "legacy payload digest differs",
    )
    metadata = _object(
        header["metadata"],
        "legacy header.metadata",
        {
            "make", "model", "width", "height", "components_per_pixel", "bits_per_sample",
            "photometric", "cfa", "black_level", "white_level", "white_balance",
            "xyz_to_camera", "active_area", "crop_area", "orientation",
        },
    )
    for field in ("width", "height", "components_per_pixel", "bits_per_sample"):
        _require(type(metadata[field]) is int and metadata[field] > 0, f"legacy metadata.{field} must be a positive integer")
    expected_count = metadata["width"] * metadata["height"] * metadata["components_per_pixel"]
    _require(pixel_count == expected_count, "legacy pixel count differs from metadata dimensions")
    _require(len(pixels_bytes) == pixel_count * 2, "legacy pixel section length differs")
    pixels = struct.unpack(f"<{pixel_count}H", pixels_bytes)
    expected_pixels = tuple(_decimal(value, "legacy expected pixel", maximum=0xFFFF) for value in expected["pixels_u16_dec"])
    _require(pixels == expected_pixels, "legacy pixels differ")
    return {"header_bytes": len(header_bytes), "pixels": pixels, "payload_digest": header["payload_blake3"]}


def verify_legacy_fixture(*, require_blake3: bool = False) -> dict[str, str | bool]:
    manifest = load_legacy_manifest()
    artifact = load_object(manifest)
    parsed = parse_legacy_object(manifest, artifact)
    sha256 = _verify_sha256_manifest(manifest, artifact)
    blake3 = _blake3(require_blake3)
    if blake3 is None:
        return {"sha256": sha256, "blake3": "skipped-not-installed", "release_verified": False}
    pixels = _sections(manifest, artifact)["pixels"]
    if blake3(pixels).digest() != bytes(parsed["payload_digest"]):
        raise ValueError("legacy payload BLAKE3 differs")
    return {"sha256": sha256, "blake3": "verified", "release_verified": True}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--require-blake3",
        action="store_true",
        help="fail closed unless all protocol BLAKE3 checks run (required in release CI)",
    )
    args = parser.parse_args(argv)
    result = {
        "v3": verify_fixture(require_blake3=args.require_blake3),
        "legacy_v2": verify_legacy_fixture(require_blake3=args.require_blake3),
    }
    if not args.require_blake3 and not result["v3"]["release_verified"]:
        print(
            "NON-RELEASE: Python blake3 is not installed; protocol digest checks were skipped",
            file=sys.stderr,
        )
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
