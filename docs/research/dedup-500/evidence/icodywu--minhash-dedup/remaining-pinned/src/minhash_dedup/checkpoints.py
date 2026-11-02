"""Whole-stage completion records for the local/shared-filesystem Slurm pipeline."""

import hashlib
import json
import shutil
import uuid
from pathlib import Path

from .io import get_datafolder
from .partition import _atomic_json


STAGE_OUTPUTS = {
    "2": ("buckets",),
    "2.5": ("buckets_pruned",),
    "3": ("remove_ids", "buckets_covered", "buckets_covered2"),
    "4": ("deduped", "removed"),
}


class StageCheckpoints:
    def __init__(self, output_folder, configuration, stage_options=None):
        folder = get_datafolder(output_folder)
        if not folder.is_local():
            raise ValueError("Stage checkpoints require a local/shared-filesystem output folder")
        self.root = Path(folder.resolve_paths("")).resolve()
        self.configuration = hashlib.sha256(json.dumps(configuration, sort_keys=True).encode()).hexdigest()
        self.stage_options = stage_options or {}

    def _path(self, stage, suffix="complete"):
        if stage not in STAGE_OUTPUTS:
            raise ValueError(f"Unknown checkpoint stage: {stage}")
        return self.root / ".stage_checkpoints" / f"{stage}.{suffix}.json"

    def _read(self, stage, suffix="complete"):
        with self._path(stage, suffix).open(encoding="utf-8") as stream:
            return json.load(stream)

    def _context(self, stage):
        stages = list(STAGE_OUTPUTS)
        index = stages.index(stage)
        parent = self.require_complete(stages[index - 1]) if index else None
        return {"configuration": self.configuration, "parent": parent, "options": self.stage_options.get(stage, {})}

    def _inventory(self, stage):
        files = {}
        for directory in STAGE_OUTPUTS[stage]:
            for path in (self.root / directory).rglob("*"):
                if path.is_file():
                    stat = path.stat()
                    files[path.relative_to(self.root).as_posix()] = [stat.st_size, stat.st_mtime_ns]
        return files

    @staticmethod
    def _clear_directory(path):
        if path.is_symlink():
            raise ValueError(f"Refusing to clear symlinked output directory: {path}")
        if path.exists():
            shutil.rmtree(path)

    def invalidate(self, stage):
        stages = list(STAGE_OUTPUTS)
        index = 0 if stage == "1" else stages.index(stage)
        for downstream in stages[index:]:
            self._path(downstream).unlink(missing_ok=True)
            self._path(downstream, "started").unlink(missing_ok=True)
        if stage == "1":
            self._clear_directory(self.root / "signatures" / "stage1_complete")

    def begin(self, stage, expected_files=()):
        self.invalidate(stage)
        context = self._context(stage)
        # Whole-stage retries must not mix old shards with a new worker layout.
        for directory in STAGE_OUTPUTS[stage]:
            self._clear_directory(self.root / directory)
        _atomic_json(
            self._path(stage, "started"),
            {
                "version": 1,
                "generation": uuid.uuid4().hex,
                "context": context,
                "expected_files": list(expected_files),
            },
        )

    def complete(self, stage):
        record = self._read(stage, "started")
        if record["context"] != self._context(stage):
            raise RuntimeError(f"Stage {stage} inputs or configuration changed while running")
        for relative_path in record["expected_files"]:
            if not (self.root / relative_path).is_file():
                raise RuntimeError(f"Stage {stage} did not produce {relative_path}")
        record["files"] = self._inventory(stage)
        _atomic_json(self._path(stage), record)
        self._path(stage, "started").unlink()

    def require_complete(self, stage):
        record = self._read(stage)
        if record.get("version") != 1 or record.get("context") != self._context(stage):
            raise RuntimeError(f"Stage {stage} completion does not match this run")
        if record.get("files") != self._inventory(stage):
            raise RuntimeError(f"Stage {stage} output files changed since completion")
        return record["generation"]
