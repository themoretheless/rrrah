import gzip
import hashlib
import io
import json
import struct
import tempfile
import unittest
from dataclasses import asdict
from pathlib import Path
from types import MethodType
from unittest.mock import Mock, patch

import fsspec
import xxhash

from minhash_dedup.io import DataFolder, OutputFileManager, get_datafolder, get_shard_from_paths_file
from minhash_dedup.jsonl import JsonlReader, JsonlWriter
from minhash_dedup.runtime import Document, Stage
from minhash_dedup.signatures import HashConfig, create_hash_func, read_tuples_from_file, seek_to_start


class TemporaryFiles(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.folder = DataFolder(self.root)

    def write(self, name, content):
        with self.folder.open(name, "wb") as stream:
            stream.write(content)


class TestRuntime(unittest.TestCase):
    def test_document_defaults_are_isolated(self):
        first, second = Document("a", "1"), Document("b", "2")
        first.metadata["source"] = "test"
        first.media.append({"url": "image"})
        self.assertEqual(asdict(second), {"text": "b", "id": "2", "media": [], "metadata": {}})

    def test_metrics_and_timing_on_failure(self):
        step = Stage()
        step.stat_update("total", "kept", value=2)
        step.stat_update("total", value=3)
        step.update_doc_stats(Document("abcd", "1", metadata={"token_count": 2}))
        self.assertEqual(step.stats["total"].total, 5)
        self.assertEqual(step.stats["total"].mean, 2.5)
        self.assertEqual(step.stats["doc_len_tokens"].total, 2)
        with self.assertRaisesRegex(ValueError, "fixture"), step.track_time():
            raise ValueError("fixture")
        self.assertEqual(step.stats.time_stats.n, 1)
        self.assertGreaterEqual(step.stats.time_stats.total, 0)
        json.dumps(step.stats.to_dict(), allow_nan=False)


class TestFileIO(TemporaryFiles):
    def test_sorted_listing_and_stride_shards(self):
        for name in ("z.jsonl", "nested/b.jsonl", "nested/a.bin", "a.jsonl"):
            self.write(name, b"data")
        names = ["a.jsonl", "nested/a.bin", "nested/b.jsonl", "z.jsonl"]
        self.assertEqual(self.folder.list_files(), names)
        self.assertEqual(self.folder.list_files(recursive=False), ["a.jsonl", "z.jsonl"])
        self.assertEqual(self.folder.list_files(subdirectory="nested"), names[1:3])
        self.assertEqual(self.folder.list_files(glob_pattern=".jsonl"), ["a.jsonl", "z.jsonl"])
        self.assertEqual(self.folder.list_files(glob_pattern="**/*.jsonl"), [names[i] for i in (0, 2, 3)])
        self.assertEqual(self.folder.get_shard(1, 2), names[1::2])
        self.assertEqual(self.folder.get_shard(5, 6), [])
        self.assertIsNone(DataFolder(self.root / "empty").get_shard(0, 1))
        with self.assertRaises(ValueError):
            self.folder.get_shard(2, 2)

    def test_paths_and_filesystem_forms(self):
        self.write("a.bin", b"abc")
        self.assertIs(get_datafolder(self.folder), self.folder)
        for specification in (
            self.root,
            str(self.root),
            (str(self.root), {}),
            (str(self.root), fsspec.filesystem("file")),
        ):
            folder = get_datafolder(specification)
            self.assertTrue(folder.isfile("a.bin"))
            self.assertTrue(folder.exists("a.bin"))
            self.assertEqual(folder.size("a.bin"), 3)
            self.assertEqual(folder.resolve_paths([Path("a.bin")]), [str(self.root / "a.bin")])
        remote = get_datafolder((f"memory://{self.root.name}", {}))
        self.addCleanup(remote.fs.rm, remote.path, recursive=True)
        with remote.open("nested/a.bin", "wb") as stream:
            stream.write(b"abc")
        self.assertFalse(remote.is_local())
        self.assertEqual(remote.list_files(), ["nested/a.bin"])
        with fsspec.open(remote.resolve_paths("nested/a.bin"), "rb") as stream:
            self.assertEqual(stream.read(), b"abc")

    def test_paths_file_sharding(self):
        self.write("paths.txt", b"c.jsonl\na.jsonl\nb.jsonl\n")
        path = str(self.root / "paths.txt")
        for specification in (path, (path, {}), (path, fsspec.filesystem("file"))):
            self.assertEqual(list(get_shard_from_paths_file(specification, 0, 2)), ["c.jsonl", "b.jsonl"])
            self.assertEqual(list(get_shard_from_paths_file(specification, 1, 2)), ["a.jsonl"])

    def test_managed_gzip_output_and_closing(self):
        with self.folder.get_output_file_manager(mode="wb") as outputs:
            outputs.write("nested/out.gz", b"first\n")
            stream = outputs.get_file("nested/out.gz")
            outputs.write("nested/out.gz", b"second\n")
            self.assertIs(stream, outputs.get_file("nested/out.gz"))
        self.assertTrue(stream.closed)
        self.assertEqual(outputs.get_open_files(), {})
        self.assertEqual(gzip.decompress((self.root / "nested/out.gz").read_bytes()), b"first\nsecond\n")

    def test_all_outputs_close_even_when_one_close_fails(self):
        first, second = Mock(), Mock()
        first.close.side_effect = OSError("close failed")
        manager = OutputFileManager(Mock(open=Mock(side_effect=[first, second])))
        manager.get_file("a")
        manager.get_file("b")
        with self.assertRaises(OSError):
            manager.close()
        first.close.assert_called_once()
        second.close.assert_called_once()
        self.assertEqual(manager.get_open_files(), {})

    def test_open_files_closes_partial_success_on_error(self):
        stream = io.BytesIO(b"abc")
        with patch.object(self.folder, "open", side_effect=[stream, FileNotFoundError()]):
            with self.assertRaises(FileNotFoundError):
                self.folder.open_files(["a", "missing"])
        self.assertTrue(stream.closed)
        self.write("a", b"abc")
        streams = self.folder.open_files(["a"])
        with streams[0] as stream:
            self.assertFalse(stream.closed)
            self.assertEqual(stream.size, 3)
            self.assertEqual(stream.read(), b"abc")


class TestSignatureHelpers(TemporaryFiles):
    def test_hash_definitions_for_both_widths(self):
        for bits in (32, 64):
            config = HashConfig(precision=bits, hash_fc="sha1")
            value = "Mixed Case 123"
            expected = int.from_bytes(hashlib.sha1(value.encode()).digest()[: bits // 8], "little")
            self.assertEqual(create_hash_func(config)(value), expected)
            self.assertEqual(config.max, (1 << bits) - 1)
            self.assertEqual(config.min, 0)
            self.assertEqual(struct.calcsize(config.struct_format), bits // 8)
            fast_hash = xxhash.xxh32_intdigest if bits == 32 else xxhash.xxh64_intdigest
            self.assertEqual(create_hash_func(HashConfig(bits))(value), fast_hash(value.encode("utf-8")))
        with self.assertRaises(ValueError):
            create_hash_func(HashConfig(hash_fc="unknown"))

    def test_xxhash_matches_previous_string_hashes(self):
        # Captured with xxhash 3.6.0, whose string API encoded UTF-8 implicitly.
        fixtures = {
            "Mixed Case 123": (896668924, 6301136730594866680),
            "caf\u00e9 \u4f60\u597d": (3596219129, 8119377547891369737),
            "": (46947589, 17241709254077376921),
        }
        for text, values in fixtures.items():
            for bits, expected in zip((32, 64), values):
                with self.subTest(text=text, bits=bits):
                    self.assertEqual(create_hash_func(HashConfig(bits))(text), expected)

    def test_record_buffering_and_truncation(self):
        rows = [(1, 2), (3, 4), (5, 6)]
        payload = b"".join(struct.pack("<QI", *row) for row in rows)
        for buffer_size in (-1, 1, 2, 5):
            self.assertEqual(
                list(read_tuples_from_file(io.BytesIO(payload), "Q", "I", lines_to_buffer=buffer_size)), rows
            )
        with self.assertRaises(struct.error):
            list(read_tuples_from_file(io.BytesIO(payload[:-1]), "QI"))
        with self.assertRaises(ValueError):
            list(read_tuples_from_file(io.BytesIO(payload), "QI", lines_to_buffer=0))

    def test_seek_to_first_matching_hash(self):
        for fmt in ("I", "Q"):
            record = struct.Struct(f"<{fmt}I")
            self.write("sigs", b"".join(record.pack(key, index) for index, key in enumerate((1, 3, 3, 7))))
            for start, index in ((0, 0), (1, 0), (2, 1), (3, 1), (4, 3), (7, 3), (8, 4)):
                with self.folder.open("sigs") as stream:
                    seek_to_start(stream, start, f"{fmt}I", fmt)
                    self.assertEqual(stream.tell(), index * record.size)


class TestJsonl(TemporaryFiles):
    def test_reader_skips_preserve_physical_line_ids_and_metadata(self):
        self.write("a.jsonl", b'not json\n{"text":""}\n{"text":"first","metadata":"{\\"tag\\":1}","extra":2}\n')
        self.write("b.jsonl", b'{"text":"second","id":"B","metadata":{"tag":3,"file_path":"saved"}}\n')
        reader = JsonlReader(self.root, default_metadata={"tag": 0, "default": True})
        documents = list(reader())
        self.assertEqual([doc.id for doc in documents], ["a.jsonl/2", "B"])
        self.assertEqual(
            documents[0].metadata, {"tag": 1, "extra": 2, "default": True, "file_path": str(self.root / "a.jsonl")}
        )
        self.assertEqual(documents[1].metadata["file_path"], "saved")
        self.assertEqual(reader.stats["documents"].total, 2)
        self.assertEqual([doc.id for doc in JsonlReader(self.root, skip=1, limit=1)()], ["B"])
        self.assertEqual(list(JsonlReader(self.root, limit=0)()), [])
        self.assertEqual([doc.id for doc in JsonlReader(self.root)(rank=1, world_size=2)], ["B"])

    def test_reader_custom_adapter_gzip_and_path_order(self):
        def adapter(reader, row, path, line):
            return {"text": row[reader.text_key], "id": f"{path}:{line}", "metadata": {"line": line}}

        for name in ("a.jsonl.gz", "b.jsonl.gz"):
            self.write(name, gzip.compress(b'{"body":"sample"}\n'))
        self.write("paths.txt", b"b.jsonl.gz\na.jsonl.gz\n")
        reader = JsonlReader(self.root, paths_file=self.root / "paths.txt", adapter=adapter, text_key="body")
        self.assertEqual([doc.id for doc in reader()], ["b.jsonl.gz:0", "a.jsonl.gz:0"])

    def test_no_files_differs_from_unassigned_rank(self):
        with self.assertRaises(RuntimeError):
            list(JsonlReader(self.root)())
        self.write("a.jsonl", b'{"text":"a"}\n')
        self.assertEqual(list(JsonlReader(self.root)(rank=1, world_size=2)), [])

    def test_writer_default_metadata_and_roundtrip(self):
        documents = [Document("first", "a", metadata={"tag": 1}), Document("second", "b")]
        writer = JsonlWriter(self.root)
        self.assertEqual(list(writer(documents, rank=3)), documents)
        rows = [json.loads(line) for line in gzip.decompress((self.root / "00003.jsonl.gz").read_bytes()).splitlines()]
        self.assertEqual(rows, [{"text": "first", "id": "a", "metadata": {"tag": 1}}, {"text": "second", "id": "b"}])
        self.assertEqual(writer.stats["total"].total, 2)
        self.assertEqual(writer.output_mg.get_open_files(), {})
        self.assertEqual([doc.id for doc in JsonlReader(self.root)()], ["a", "b"])

    def test_writer_labels_adapters_and_split_names(self):
        def adapter(writer, document):
            return {"body": document.text, "identifier": document.id, "expanded": writer.expand_metadata}

        writer = JsonlWriter(self.root, output_filename="${data_label}/${rank}.jsonl", max_file_size=1)
        writer.adapter = MethodType(adapter, writer)
        with writer:
            for index in range(2):
                writer.write(Document("text", str(index), metadata={"data_label": "fixture"}), rank=7)
        self.assertEqual(self.folder.list_files(), ["fixture/00007_0000.jsonl.gz", "fixture/00007_0001.jsonl.gz"])
        for index, filename in enumerate(self.folder.list_files()):
            row = json.loads(gzip.decompress((self.root / filename).read_bytes()))
            self.assertEqual(row, {"body": "text", "identifier": str(index), "expanded": False})
        with self.assertRaisesRegex(ValueError, "data_label"):
            writer._get_output_filename(Document("text", "a"), 0)
        document = Document("text", "a", metadata={"data_label": "fixture", "rank": "custom"})
        self.assertEqual(writer._get_output_filename(document, 0, rank_override=1), "fixture/custom.jsonl.gz")

    def test_writer_constructor_adapter_and_metadata_expansion(self):
        def adapter(writer, document):
            return {"text": document.text.upper(), "id": document.id}

        with JsonlWriter(self.root, compression=None, adapter=adapter) as writer:
            writer.write(Document("hello", "a"))
        self.assertEqual(json.loads((self.root / "00000.jsonl").read_bytes()), {"text": "HELLO", "id": "a"})
        with JsonlWriter(self.root, compression=None, expand_metadata=True) as writer:
            writer.write(Document("hello", "b", metadata={"id": "metadata-id", "tag": 1}), rank=1)
        self.assertEqual(
            json.loads((self.root / "00001.jsonl").read_bytes()), {"text": "hello", "id": "metadata-id", "tag": 1}
        )


if __name__ == "__main__":
    unittest.main()
