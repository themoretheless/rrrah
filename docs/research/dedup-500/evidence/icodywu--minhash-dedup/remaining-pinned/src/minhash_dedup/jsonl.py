"""Streaming JSONL input and routed output for stages 1 and 4.

Reader order, adapter calls, metadata precedence, and output names retain the
existing stage contracts. No general reader/writer plugin framework is needed.
"""

import dataclasses
import json
import os.path
import random
from collections import Counter
from string import Template
from types import MethodType

from tqdm import tqdm

from .io import get_datafolder, get_shard_from_paths_file
from .runtime import Document, Stage, StatLabel, logger


class JsonlReader(Stage):
    name = "jsonl reader"

    def __init__(
        self,
        data_folder,
        paths_file=None,
        compression="infer",
        limit=-1,
        skip=0,
        file_progress=False,
        doc_progress=False,
        adapter=None,
        text_key="text",
        id_key="id",
        default_metadata=None,
        recursive=True,
        glob_pattern=None,
        shuffle_files=False,
        *,
        files=None,
    ):
        super().__init__()
        if files is not None and paths_file is not None:
            raise ValueError("Specify files or paths_file, not both")
        self.data_folder = get_datafolder(data_folder)
        self.files = tuple(files) if files is not None else None
        self.paths_file = paths_file
        self.compression = compression
        self.limit, self.skip = limit, skip
        self.file_progress, self.doc_progress = file_progress, doc_progress
        self.text_key, self.id_key = text_key, id_key
        self.default_metadata = default_metadata or {}
        self.recursive, self.glob_pattern = recursive, glob_pattern
        self.shuffle_files = shuffle_files
        self.adapter = MethodType(adapter, self) if adapter else self._default_adapter
        self._empty_warning = False

    def _default_adapter(self, data, path, id_in_file):
        metadata = data.pop("metadata", {})
        if isinstance(metadata, str):
            try:
                metadata = json.loads(metadata)
            except json.JSONDecodeError:
                pass
        if not isinstance(metadata, dict):
            metadata = {"metadata": metadata}
        text = data.pop(self.text_key, "")
        identifier = data.pop(self.id_key, f"{path}/{id_in_file}")
        media = data.pop("media", [])
        return {"text": text, "id": identifier, "media": media, "metadata": metadata | data}

    def get_document_from_dict(self, data, source_file, id_in_file):
        fields = self.adapter(data, source_file, id_in_file)
        if not fields.get("text"):
            if not self._empty_warning:
                logger.warning("Skipping a document without text in {}", source_file)
                self._empty_warning = True
            return None
        document = Document(**fields)
        document.metadata = self.default_metadata | document.metadata
        document.metadata.setdefault("file_path", self.data_folder.resolve_paths(source_file))
        return document

    def read_file(self, filepath):
        import orjson

        with self.data_folder.open(filepath, "r", compression=self.compression) as stream:
            try:
                for line_number, line in enumerate(stream):
                    with self.track_time():
                        try:
                            document = self.get_document_from_dict(orjson.loads(line), filepath, line_number)
                        except (EOFError, orjson.JSONDecodeError) as error:
                            logger.warning("Error when reading `{}`: {}", filepath, error)
                            continue
                    if document is not None:
                        yield document
            except UnicodeDecodeError as error:
                logger.warning("File `{}` may be corrupted: {}", filepath, error)

    def run(self, data=None, rank=0, world_size=1):
        if data is not None:
            yield from data
        if self.files is not None:
            if not 0 <= rank < world_size:
                raise ValueError("rank must be in [0, world_size)")
            files = list(self.files[rank::world_size])
        else:
            files = (
                list(get_shard_from_paths_file(self.paths_file, rank, world_size))
                if self.paths_file
                else self.data_folder.get_shard(
                    rank, world_size, recursive=self.recursive, glob_pattern=self.glob_pattern
                )
            )
        if files is None:
            raise RuntimeError(f"No files found on {self.data_folder.path}!")
        if not files:
            logger.warning("No files found on {} for rank={}", self.data_folder.path, rank)
        if self.shuffle_files:
            random.shuffle(files)
        skipped = emitted = 0
        with tqdm(total=len(files), desc="File progress", unit="file", disable=not self.file_progress) as file_bar:
            with tqdm(
                total=self.limit if self.limit != -1 else None,
                desc="Document progress",
                unit="doc",
                disable=not self.doc_progress,
            ) as doc_bar:
                for position, filename in enumerate(files, 1):
                    self.stat_update("input_files")
                    logger.info("Reading input file {}, {}/{}", filename, position, len(files))
                    count = 0
                    for document in self.read_file(filename):
                        if skipped < self.skip:
                            skipped += 1
                            continue
                        if self.limit != -1 and emitted >= self.limit:
                            break
                        self.update_doc_stats(document)
                        yield document
                        emitted += 1
                        count += 1
                        doc_bar.update()
                    file_bar.update()
                    self.stat_update("documents", value=count, unit="input_file")
                    if self.limit != -1 and emitted >= self.limit:
                        break


class JsonlWriter(Stage):
    name = "jsonl writer"

    def __init__(
        self,
        output_folder,
        output_filename=None,
        compression="gzip",
        adapter=None,
        expand_metadata=False,
        max_file_size=-1,
    ):
        super().__init__()
        self.output_folder = get_datafolder(output_folder)
        self.compression = compression
        filename = output_filename or "${rank}.jsonl"
        suffix = {"gzip": ".gz", "zstd": ".zst"}.get(compression)
        if suffix and not filename.endswith(suffix):
            filename += suffix
        self.output_filename = Template(filename)
        self.expand_metadata = expand_metadata
        self.max_file_size = max_file_size
        self.file_id_counter = Counter()
        self.output_mg = self.output_folder.get_output_file_manager(mode="wb", compression=compression)
        self.adapter = MethodType(adapter, self) if adapter else self._default_adapter

    def _default_adapter(self, document):
        fields = {key: value for key, value in dataclasses.asdict(document).items() if value}
        if self.expand_metadata and "metadata" in fields:
            fields |= fields.pop("metadata")
        return fields

    def _get_output_filename(self, document, rank, **kwargs):
        if "${data_label}" in self.output_filename.template and "data_label" not in document.metadata:
            raise ValueError("${data_label} requires data_label in document.metadata")
        return self.output_filename.substitute(
            {"rank": str(rank).zfill(5), "id": document.id, **document.metadata, **kwargs}
        )

    def _get_filename_with_file_id(self, filename):
        directory, basename = os.path.split(filename)
        for suffix in (".jsonl.gz", ".jsonl.zst", ".jsonl.bz2", ".jsonl.xz"):
            if basename.endswith(suffix):
                stem, extension = basename[: -len(suffix)], suffix
                break
        else:
            stem, extension = os.path.splitext(basename)
        name = f"{stem}_{self.file_id_counter[filename]:04d}{extension}"
        return os.path.join(directory, name) if directory else name

    def write(self, document, rank=0, **kwargs):
        import orjson

        original = filename = self._get_output_filename(document, rank, **kwargs)
        if self.max_file_size > 0:
            filename = self._get_filename_with_file_id(original)
            if self.output_mg.get_file(filename).tell() >= self.max_file_size:
                self.output_mg.pop(filename).close()
                self.file_id_counter[original] += 1
                filename = self._get_filename_with_file_id(original)
        self.output_mg.write(filename, orjson.dumps(self.adapter(document), option=orjson.OPT_APPEND_NEWLINE))
        self.stat_update(self._get_output_filename(document, "XXXXX", **kwargs), StatLabel.total)
        self.update_doc_stats(document)

    def close(self):
        self.output_mg.close()

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()

    def run(self, data, rank=0, world_size=1):
        with self:
            for document in data:
                with self.track_time():
                    self.write(document, rank)
                yield document
