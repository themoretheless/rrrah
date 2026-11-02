"""Persistent reader assignments shared by signature generation and filtering."""

import hashlib
import json
import os
import tempfile
from bisect import bisect_right
from dataclasses import asdict, dataclass
from functools import cached_property
from itertools import accumulate
from math import floor
from pathlib import Path

from .io import get_datafolder


def allocate_reader_tasks(file_counts, directory_sizes, world_size):
    """Preserve the original capacity-capped, largest-remainder allocation."""
    if world_size < 1:
        raise ValueError("Logical world_size must be positive")
    if len(file_counts) != len(directory_sizes):
        raise ValueError("File counts and directory sizes must have the same length")
    if any(count < 0 for count in file_counts) or any(size < 0 for size in directory_sizes):
        raise ValueError("File counts and directory sizes must be nonnegative")
    if world_size >= sum(file_counts):
        return list(file_counts)

    workers = [int(count > 0) for count in file_counts]
    remaining = world_size - sum(workers)
    if remaining < 0:
        raise ValueError("Logical world_size must cover every nonempty input directory")
    capacity = [count - assigned for count, assigned in zip(file_counts, workers)]
    if remaining:
        total_size = sum(directory_sizes)
        if total_size <= 0:
            raise ValueError("Cannot allocate additional readers proportionally when all input sizes are zero")
        shares = [size / total_size * remaining for size in directory_sizes]
        for index, share in enumerate(shares):
            give = min(floor(share), capacity[index])
            workers[index] += give
            capacity[index] -= give
            remaining -= give
        remainders = sorted([(share - floor(share), index) for index, share in enumerate(shares)], reverse=True)
        index = 0
        while remaining:
            _, directory = remainders[index % len(remainders)]
            if capacity[directory] > 0:
                workers[directory] += 1
                capacity[directory] -= 1
                remaining -= 1
            index += 1
    return workers


def _atomic_json(path, value, exclusive=False):
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=path.parent, delete=False) as stream:
        temporary = Path(stream.name)
        try:
            json.dump(value, stream, sort_keys=True, allow_nan=False)
            stream.flush()
            os.fsync(stream.fileno())
            if exclusive:
                os.link(temporary, path)
            else:
                os.replace(temporary, path)
        finally:
            temporary.unlink(missing_ok=True)


@dataclass
class InputPartition:
    path: str
    files: list[str]
    sizes: list[int]
    workers: int


@dataclass
class ReaderPlan:
    world_size: int
    inputs: list[InputPartition]
    signature_config: dict
    version: int = 1

    @property
    def active_ranks(self):
        return sum(source.workers for source in self.inputs)

    @cached_property
    def offsets(self):
        return list(accumulate([0] + [source.workers for source in self.inputs]))

    @cached_property
    def fingerprint(self):
        return hashlib.sha256(json.dumps(asdict(self), sort_keys=True).encode("utf-8")).hexdigest()

    @staticmethod
    def path(signature_folder):
        folder = get_datafolder(signature_folder)
        if not folder.is_local():
            raise ValueError("Reader plans require a local/shared-filesystem signature folder")
        return Path(folder.resolve_paths("reader_plan.json"))

    @classmethod
    def load(cls, signature_folder):
        path = cls.path(signature_folder)
        if not path.is_file():
            raise FileNotFoundError(
                f"Missing {path}. Stage 1 must create a reader plan; do not infer world_size from signature files."
            )
        with path.open(encoding="utf-8") as stream:
            value = json.load(stream)
        if value.get("version") != 1:
            raise ValueError(f"Unsupported reader plan version in {path}")
        plan = cls(
            world_size=value["world_size"],
            inputs=[InputPartition(**source) for source in value["inputs"]],
            signature_config=value["signature_config"],
        )
        if plan.world_size < 1 or plan.active_ranks > plan.world_size:
            raise ValueError(f"Invalid rank counts in {path}")
        for source in plan.inputs:
            if (
                len(source.files) != len(source.sizes)
                or not 0 <= source.workers <= len(source.files)
                or bool(source.files) != bool(source.workers)
            ):
                raise ValueError(f"Invalid input partition in {path}")
        return plan

    @classmethod
    def prepare(cls, signature_folder, input_paths, world_size, launch_world_size, signature_config):
        """Publish once; subsequent workers validate and reuse the same plan."""
        path = cls.path(signature_folder)
        if not path.exists():
            # Old or partial signature files do not establish the original reader layout.
            if any(path.parent.glob("bucket_*/*.minhash.sig")) and not path.exists():
                raise ValueError("Existing signatures have no reader plan; use a new signature output directory")
            inputs = []
            sizes = []
            for input_path in input_paths:
                folder = get_datafolder(input_path)
                files = folder.list_files()
                file_sizes = [folder.size(name) for name in files]
                for name, size in zip(files, file_sizes):
                    if size > 10 * 1024**3:
                        raise ValueError(f"Input file exceeds 10 GiB; split it first: {folder.resolve_paths(name)}")
                inputs.append(InputPartition(folder.resolve_paths(""), files, file_sizes, 0))
                sizes.append(sum(file_sizes) * (2 if any(name.endswith(".gz") for name in files) else 1))
            logical_size = world_size if world_size is not None else launch_world_size
            workers = allocate_reader_tasks([len(source.files) for source in inputs], sizes, logical_size)
            for source, count in zip(inputs, workers):
                source.workers = count
            candidate = cls(logical_size, inputs, signature_config)
            try:
                _atomic_json(path, asdict(candidate), exclusive=True)
            except FileExistsError:
                if asdict(cls.load(signature_folder)) != asdict(candidate):
                    raise ValueError("Concurrent Stage 1 workers produced different reader plans") from None
        plan = cls.load(signature_folder)
        plan.validate(input_paths, world_size, signature_config)
        return plan

    def validate(self, input_paths, world_size=None, signature_config=None):
        if world_size is not None and world_size != self.world_size:
            raise ValueError(
                f"Logical world_size mismatch: requested {world_size}, Stage 1 plan uses {self.world_size}"
            )
        paths = [get_datafolder(path).resolve_paths("") for path in input_paths]
        if paths != [source.path for source in self.inputs]:
            raise ValueError("Input directories or their order differ from the Stage 1 reader plan")
        if signature_config is not None and signature_config != self.signature_config:
            raise ValueError("MinHash configuration differs from the Stage 1 reader plan")

    def ranks_for_worker(self, launch_rank, launch_world_size):
        if launch_world_size < 1 or not 0 <= launch_rank < launch_world_size:
            raise ValueError("Launched worker rank must be in [0, launch_world_size)")
        return range(launch_rank, self.active_ranks, launch_world_size)

    def shard(self, rank):
        if not 0 <= rank < self.active_ranks:
            raise ValueError(f"Logical rank {rank} has no input assignment")
        index = bisect_right(self.offsets, rank) - 1
        source = self.inputs[index]
        local_rank = rank - self.offsets[index]
        files = source.files[local_rank :: source.workers]
        sizes = source.sizes[local_rank :: source.workers]
        folder = get_datafolder(source.path)
        for name, expected_size in zip(files, sizes):
            if folder.size(name) != expected_size:
                raise ValueError(f"Input size changed since partitioning: {folder.resolve_paths(name)}")
        return index, files

    def _completion_path(self, signature_folder, rank):
        if not 0 <= rank < self.active_ranks:
            raise ValueError(f"Logical rank {rank} has no input assignment")
        return self.path(signature_folder).parent / "stage1_complete" / f"{rank:05d}.json"

    def begin_rank(self, signature_folder, rank):
        self._completion_path(signature_folder, rank).unlink(missing_ok=True)

    def complete_rank(self, signature_folder, rank):
        _atomic_json(self._completion_path(signature_folder, rank), {"plan": self.fingerprint, "rank": rank})

    def require_complete(self, signature_folder, rank):
        path = self._completion_path(signature_folder, rank)
        if not path.is_file():
            raise RuntimeError(f"Stage 1 logical rank {rank} has not completed; missing {path}")
        with path.open(encoding="utf-8") as stream:
            completion = json.load(stream)
        if completion != {"plan": self.fingerprint, "rank": rank}:
            raise ValueError(f"Stage 1 completion for rank {rank} does not match the reader plan")
