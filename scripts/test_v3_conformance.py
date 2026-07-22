from __future__ import annotations

import copy
import importlib.util
import unittest
from unittest import mock

import v3_conformance


class V3ConformanceTests(unittest.TestCase):
    def test_independent_encoder_and_parser_match_checked_in_object(self) -> None:
        manifest = v3_conformance.load_manifest()
        artifact = v3_conformance.load_object(manifest)
        parsed = v3_conformance.parse_v3_object(manifest, artifact)
        self.assertEqual(parsed["descriptor_bytes"], 106)
        self.assertEqual(parsed["payload_bytes"], 223)
        self.assertEqual(parsed["pixels"], (0, 512, 8192, 16383))
        self.assertEqual(v3_conformance.build_object(manifest), artifact)

        result = v3_conformance.verify_fixture()
        self.assertEqual(
            result["sha256"],
            "437fd88e2be1310a7132ceb48147b4d2dd96a0067f8675dabec8c3f5ab7a9335",
        )
        self.assertIn(result["blake3"], {"skipped-not-installed", "verified"})
        self.assertEqual(result["release_verified"], result["blake3"] == "verified")

    def test_manifest_rejects_unknown_missing_wrong_type_and_version(self) -> None:
        cases = []

        unknown = copy.deepcopy(v3_conformance.load_manifest())
        unknown["surprise"] = True
        cases.append((unknown, "unknown fields"))

        missing = copy.deepcopy(v3_conformance.load_manifest())
        del missing["protocol"]["byte_order"]
        cases.append((missing, "missing fields"))

        wrong_type = copy.deepcopy(v3_conformance.load_manifest())
        wrong_type["artifact"]["decoded_length_dec"] = 465
        cases.append((wrong_type, "must be a string"))

        noncanonical_decimal = copy.deepcopy(v3_conformance.load_manifest())
        noncanonical_decimal["artifact"]["decoded_length_dec"] = "0465"
        cases.append((noncanonical_decimal, "canonical decimal"))

        future = copy.deepcopy(v3_conformance.load_manifest())
        future["manifest_schema"] = "rrrah.cache.conformance.v2"
        cases.append((future, "must equal"))

        protocol_drift = copy.deepcopy(v3_conformance.load_manifest())
        protocol_drift["protocol"]["payload_schema_version_dec"] = "2"
        cases.append((protocol_drift, "unsupported version/value"))

        for manifest, message in cases:
            with self.subTest(message=message), self.assertRaisesRegex(ValueError, message):
                v3_conformance.validate_manifest(manifest)

    def test_json_duplicate_fields_and_nonstandard_constants_are_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "duplicate JSON field"):
            v3_conformance._decode_json('{"field": 1, "field": 2}')
        with self.assertRaisesRegex(ValueError, "non-standard JSON constant"):
            v3_conformance._decode_json('{"field": NaN}')

    def test_sections_require_unique_bounded_nonoverlapping_full_coverage(self) -> None:
        base = v3_conformance.load_manifest()
        cases = []

        duplicate = copy.deepcopy(base)
        duplicate["sections"][1]["name"] = "envelope"
        cases.append((duplicate, "duplicate section name"))

        overlap = copy.deepcopy(base)
        overlap["sections"][1]["offset_dec"] = "135"
        cases.append((overlap, "overlaps"))

        gap = copy.deepcopy(base)
        gap["sections"][1]["offset_dec"] = "137"
        cases.append((gap, "uncovered byte range"))

        out_of_bounds = copy.deepcopy(base)
        out_of_bounds["sections"][2]["length_dec"] = "224"
        cases.append((out_of_bounds, "exceeds artifact length"))

        uncovered_tail = copy.deepcopy(base)
        uncovered_tail["sections"][2]["length_dec"] = "222"
        cases.append((uncovered_tail, "uncovered byte range"))

        empty = copy.deepcopy(base)
        empty["sections"][2]["length_dec"] = "0"
        cases.append((empty, "must not be empty"))

        for manifest, message in cases:
            with self.subTest(message=message), self.assertRaisesRegex(ValueError, message):
                v3_conformance.validate_manifest(manifest)

        reordered = copy.deepcopy(base)
        reordered["sections"].reverse()
        self.assertIs(v3_conformance.validate_manifest(reordered), reordered)

    def test_independent_parser_rejects_wire_mutations_before_hash_checks(self) -> None:
        manifest = v3_conformance.load_manifest()
        artifact = v3_conformance.load_object(manifest)
        section_offsets = {
            section["name"]: int(section["offset_dec"])
            for section in manifest["sections"]
        }
        cases = (
            (22, "reserved bytes"),
            (section_offsets["semantic_descriptor"], "artifact kind"),
            (section_offsets["mosaic_payload"] + 24, "reserved bytes"),
            (section_offsets["mosaic_payload"] + 76, "sample format"),
        )
        for offset, message in cases:
            changed = bytearray(artifact)
            changed[offset] ^= 1
            with self.subTest(offset=offset), self.assertRaisesRegex(ValueError, message):
                v3_conformance.parse_v3_object(manifest, bytes(changed))

    def test_single_byte_changes_are_visible_to_section_hashes(self) -> None:
        manifest = v3_conformance.load_manifest()
        artifact = bytearray(v3_conformance.load_object(manifest))
        payload = next(section for section in manifest["sections"] if section["name"] == "mosaic_payload")
        offset = int(payload["offset_dec"])
        length = int(payload["length_dec"])
        artifact[offset + 8] ^= 1
        self.assertNotEqual(
            v3_conformance.hashlib.sha256(artifact[offset : offset + length]).hexdigest(),
            payload["sha256"],
        )

    def test_missing_blake3_is_explicitly_non_release_and_fail_closed_when_required(self) -> None:
        with mock.patch.object(v3_conformance, "_import_blake3", side_effect=ImportError):
            result = v3_conformance.verify_fixture()
            self.assertEqual(result["blake3"], "skipped-not-installed")
            self.assertFalse(result["release_verified"])
            with self.assertRaisesRegex(RuntimeError, "release conformance requires"):
                v3_conformance.verify_fixture(require_blake3=True)

    @unittest.skipUnless(
        importlib.util.find_spec("blake3") is not None,
        "non-release local environment: Python blake3 is not installed",
    )
    def test_release_mode_verifies_all_v3_protocol_digests(self) -> None:
        result = v3_conformance.verify_fixture(require_blake3=True)
        self.assertEqual(result["blake3"], "verified")
        self.assertTrue(result["release_verified"])


class LegacyConformanceTests(unittest.TestCase):
    def test_legacy_fixture_has_independent_structure_and_section_hashes(self) -> None:
        manifest = v3_conformance.load_legacy_manifest()
        artifact = v3_conformance.load_object(manifest)
        parsed = v3_conformance.parse_legacy_object(manifest, artifact)
        self.assertEqual(parsed["pixels"], (512, 1024, 2048, 4096))
        self.assertEqual(parsed["header_bytes"], 743)
        result = v3_conformance.verify_legacy_fixture()
        self.assertEqual(
            result["sha256"],
            "3eea098a30755cfa03bd91ee64431f07078ce2b640a41584342230652351e530",
        )

    def test_legacy_manifest_uses_its_own_strict_schema(self) -> None:
        manifest = copy.deepcopy(v3_conformance.load_legacy_manifest())
        manifest["protocol"] = {}
        with self.assertRaisesRegex(ValueError, "unknown fields"):
            v3_conformance.validate_manifest(manifest, "legacy_v2")

    def test_legacy_parser_rejects_prefix_and_dimension_mutations(self) -> None:
        manifest = v3_conformance.load_legacy_manifest()
        artifact = bytearray(v3_conformance.load_object(manifest))
        artifact[0] ^= 1
        with self.assertRaisesRegex(ValueError, "legacy prefix differs"):
            v3_conformance.parse_legacy_object(manifest, bytes(artifact))

    @unittest.skipUnless(
        importlib.util.find_spec("blake3") is not None,
        "non-release local environment: Python blake3 is not installed",
    )
    def test_release_mode_verifies_legacy_payload_digest(self) -> None:
        result = v3_conformance.verify_legacy_fixture(require_blake3=True)
        self.assertEqual(result["blake3"], "verified")
        self.assertTrue(result["release_verified"])


if __name__ == "__main__":
    unittest.main()
