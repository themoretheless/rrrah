"""File listing, rank sharding, and managed outputs for the MinHash stages."""

import os
import posixpath
from contextlib import ExitStack
from glob import has_magic
from typing import TypeAlias

from fsspec import AbstractFileSystem
from fsspec.core import url_to_fs
from fsspec.implementations.dirfs import DirFileSystem
from fsspec.implementations.local import LocalFileSystem


class OutputFileManager:
    def __init__(self, folder, mode="wt", compression="infer"):
        self.folder = folder
        self.mode = mode
        self.compression = compression
        self._files = {}

    def get_file(self, filename):
        if filename not in self._files:
            self._files[filename] = self.folder.open(filename, self.mode, compression=self.compression)
        return self._files[filename]

    def write(self, filename, data):
        self.get_file(filename).write(data)

    def pop(self, filename):
        self.get_file(filename)
        return self._files.pop(filename)

    def get_open_files(self):
        return self._files

    def close(self):
        files, self._files = self._files, {}
        with ExitStack() as pending:
            for stream in files.values():
                pending.callback(stream.close)

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()


class DataFolder:
    """A small fsspec facade with stable relative filenames and stride sharding."""

    def __init__(self, path, fs=None, auto_mkdir=True, **storage_options):
        path = os.fspath(path)
        if fs is None:
            fs, path = url_to_fs(path, **storage_options)
        self._directory = DirFileSystem(path=path, fs=fs)
        self.fs = fs
        self.path = self._directory.path
        self.auto_mkdir = auto_mkdir

    def list_files(self, subdirectory="", recursive=True, glob_pattern=None, include_directories=False):
        options = {"detail": True, "maxdepth": None if recursive else 1}
        if glob_pattern:
            if not has_magic(glob_pattern):
                glob_pattern = "*" + glob_pattern
            entries = self._directory.glob(posixpath.join(subdirectory, glob_pattern), **options)
        else:
            entries = self._directory.find(subdirectory, withdirs=include_directories, **options)
        return sorted(name for name, info in entries.items() if include_directories or info["type"] != "directory")

    def get_shard(self, rank, world_size, **kwargs):
        if not 0 <= rank < world_size:
            raise ValueError("rank must be in [0, world_size)")
        files = self.list_files(**kwargs)
        return files[rank::world_size] if files else None

    def resolve_paths(self, paths):
        if isinstance(paths, (str, os.PathLike)):
            path = self._directory._join(os.fspath(paths))
            return self.fs._strip_protocol(path) if self.is_local() else self.fs.unstrip_protocol(path)
        return [self.resolve_paths(path) for path in paths]

    def open(self, path, mode="rb", **kwargs):
        path = os.fspath(path)
        if self.auto_mkdir and any(flag in mode for flag in "wax"):
            self.fs.makedirs(posixpath.dirname(self._directory._join(path)), exist_ok=True)
        return self._directory.open(path, mode=mode, **kwargs)

    def open_files(self, paths, mode="rb", **kwargs):
        with ExitStack() as pending:
            files = [pending.enter_context(self.open(path, mode, **kwargs)) for path in paths]
            pending.pop_all()
            return files

    def exists(self, path):
        return self._directory.exists(os.fspath(path))

    def isfile(self, path):
        return self._directory.isfile(os.fspath(path))

    def size(self, path):
        return self._directory.size(os.fspath(path))

    def is_local(self):
        return isinstance(self.fs, LocalFileSystem)

    def get_output_file_manager(self, **kwargs):
        return OutputFileManager(self, **kwargs)


DataFileLike: TypeAlias = str | os.PathLike | tuple[str, dict] | tuple[str, AbstractFileSystem]
DataFolderLike: TypeAlias = DataFileLike | DataFolder


def get_datafolder(location: DataFolderLike) -> DataFolder:
    if isinstance(location, DataFolder):
        return location
    if isinstance(location, (str, os.PathLike)):
        return DataFolder(location)
    if isinstance(location, tuple) and len(location) == 2:
        path, options = location
        if isinstance(options, dict):
            return DataFolder(path, **options)
        if isinstance(options, AbstractFileSystem):
            return DataFolder(path, fs=options)
    raise TypeError("Expected a path, DataFolder, (path, storage_options), or (path, filesystem)")


def get_shard_from_paths_file(paths_file: DataFileLike, rank, world_size):
    if not 0 <= rank < world_size:
        raise ValueError("rank must be in [0, world_size)")
    if isinstance(paths_file, tuple):
        path, options = paths_file
        if isinstance(options, AbstractFileSystem):
            fs = options
        else:
            fs, path = url_to_fs(os.fspath(path), **options)
    else:
        fs, path = url_to_fs(os.fspath(paths_file))
    with fs.open(path, "rt") as stream:
        for index, path in enumerate(stream):
            if index % world_size == rank:
                yield path.strip()
