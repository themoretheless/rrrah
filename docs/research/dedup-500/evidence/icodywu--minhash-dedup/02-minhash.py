import contextlib
import io
import heapq
import os
import re
import random
import shutil
import nltk
import string
import struct
import functools
import tempfile
import traceback
from collections import defaultdict
from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import Generator, Dict, Iterable, List, Optional, Set, Tuple

import numpy as np
from fsspec.spec import AbstractBufferedFile
from tqdm import tqdm

from minhash_dedup.io import DataFolderLike, get_datafolder
from minhash_dedup.jsonl import JsonlWriter
from minhash_dedup.runtime import DocumentStream, Stage, StatLabel, logger
from minhash_dedup.signatures import HashConfig, TextNormConfig, create_hash_func, read_tuples_from_file, seek_to_start

try:
    import numba as _numba
    _numba_import_error: Optional[Exception] = None
    _numba_import_traceback = ""
except Exception as e:
    _numba = None
    _numba_import_error = e
    _numba_import_traceback = traceback.format_exc()

# http://en.wikipedia.org/wiki/Mersenne_prime
_mersenne_prime = np.uint64((1 << 61) - 1)

"""
n_grams -> roughly nr of words (this should be small enough to catch fuzzy matches but big enough to not have each shingle be too common)
threshold is (1/14)^(1/9)~0.746,   (1/7)^(1/18) = 0.898,  (1/12)^(1/11) = 0.798
threshold is real minhash similarity cutoff for high probability inclusion by LSH minhash
probability of inclusion for s=0.8: 1-(1-0.8^9)^14=0.867
"""

SENTINEL = (1 << 32) - 1


@dataclass
class MinhashConfig:
    """Configuration for Min-Hash deduplication

    Args:
        n_grams: n-grams size to use
        num_buckets: number of buckets to use
        hashes_per_bucket: number of hashes per bucket
        remove_rate:    remove rate of clustered nodes (except root)
        seed: random seed used to generate the hash function parameters. Should be the same on all workers to ensure they all have the same parameters
    """

    n_grams: int = 20
    num_buckets: int = 14
    hashes_per_bucket: int = 9
    seed: int = 1

    @property
    def threshold(self) -> float:
        return (1 / self.num_buckets) ** (1 / self.hashes_per_bucket)

    norm_config: TextNormConfig = field(default_factory=TextNormConfig)
    hash_config: HashConfig = field(default_factory=HashConfig)

    def __str__(self):
        return f"{self.n_grams}ng_{self.num_buckets}bs_{self.hashes_per_bucket}hs_{self.hash_config}"


@dataclass(slots=True)
class HashSig:
    """Hash signature for a given document in a given bucket

    Args:
        sig: tuple of hashes
        secondary_hash: 4-byte hash derived from global signature extrema (min or max across full signature)
        read_id: reader id. Used to know from where the next signature should be requested
        doc_id: document id       
    """

    sig: tuple[int, ...]
    secondary_hash: int
    read_id: int
    doc_id: int
    
    
    def __lt__(self, other):
        # Compare ONLY on the keys you want the heap to use
        return (self.sig, self.secondary_hash, self.read_id, self.doc_id) < (other.sig, other.secondary_hash, other.read_id, other.doc_id)

    def is_from_index(self):
        return self.doc_id == -1


def read_sigs_with_pool(
    file: AbstractBufferedFile,
    read_id: int,
    config: MinhashConfig,
    sig_pool: HashSig,
    index_file: bool = False,
    min_hash: int = 0,
    max_hash: int = _mersenne_prime,
    ensure_order: bool = True,
    lines_to_buffer: int = 50_000,
) -> Generator:
    """Read signatures from a file with object reuse from pool.
    
    Args:
        file: file to read from
        read_id: id of the reader
        config: minhash configuration
        sig_pool: pool of HashSig objects to reuse
        index_file: whether this is an index file
        min_hash: minimum hash value to consider
        max_hash: maximum hash value to consider
        ensure_order: whether to ensure signatures are in order
        lines_to_buffer: number of lines to buffer when reading
    """
    line_format = (
        f"{config.hashes_per_bucket + 1}{config.hash_config.struct_format}I"
    )  # trailing secondary_hash (hash width), doc_idx (u32)
    
    with file as f:
        if f.size == 0:
            return
        seek_to_start(f, min_hash, line_format, config.hash_config.struct_format)
        last = None
        file_stem = Path(file.path).name.removesuffix(".minhash.sig")
        file_rank = int(file_stem)
        for data in read_tuples_from_file(f, line_format, lines_to_buffer=lines_to_buffer):
            sigdata = data if index_file else data[:-2]  # Remove secondary_hash, doc_id
            assert sigdata[0] >= min_hash and (
                ensure_order is False or last is None or sigdata >= last
            ), f"Hash order error. {f.tell()=}, {min_hash=}, {sigdata=}, {last=}"
            if sigdata[0] >= max_hash:
                break
            last = sigdata
            
            # Reuse the single object from pool
            sig_obj = sig_pool
            
            # Update fields directly (reuse existing object)
            sig_obj.sig = sigdata
            sig_obj.secondary_hash = 0 if index_file else data[-2]
            sig_obj.read_id = read_id
            sig_obj.doc_id = -1 if index_file else data[-1]
                        
            yield sig_obj

class SignatureStage(Stage):
    """Stage 1: character-shingle MinHash signatures.

    Generate character-shingle signatures and sort each band's file independently.

    Args:
        output_folder: output folder where signatures will be saved
        config: minhash configuration (a MinhashConfig object)
        language: compatibility field; character shingling does not use a language tokenizer
    """

    name = "stage 1: signatures"

    def __init__(
        self,
        output_folder: DataFolderLike,
        config: MinhashConfig = None,
        language: str = "eng",
    ):
        super().__init__()
        self.output_folder = get_datafolder(output_folder)
        self.config = config or MinhashConfig()
        self.language = language
        self.num_hashes = self.config.num_buckets * self.config.hashes_per_bucket
        self._hash_func = create_hash_func(self.config.hash_config)
        self._parameters = None
        logger.debug(f"Initialized SignatureStage with language: {self.language}")

    @property
    def parameters(self):
        """Minhash parameters

        Create parameters for a random bijective permutation function
        that maps a 32/64-bit hash value to another 32/64-bit hash value.
        http://en.wikipedia.org/wiki/Universal_hashing

        Note: For 64-bit hashes the upper-bound for codomain is not [0,2**64) but [0,2**61 - 1)
        """
        if self._parameters is None:
            gen = np.random.RandomState(self.config.seed)
            self._parameters = (
                gen.randint(1, _mersenne_prime, dtype=np.uint64, size=(1, self.num_hashes)),
                gen.randint(0, _mersenne_prime, dtype=np.uint64, size=(1, self.num_hashes)),
            )
        return self._parameters
    
    def get_shingles(self, text: str) -> np.ndarray:
        """Get shingles (hashed n-grams) from a string of text

        Hash character n-grams after removing ASCII punctuation and normalizing
        whitespace. Case is preserved; the legacy norm_config is not consulted.

        Args:
            text: input text

        Returns:
            numpy array of shingles: dtype = uint64, shape = (number of n_grams in string, 1)
        """
        # lower cased
        #text = text.lower()
        # remove punctuation
        text = text.translate(str.maketrans("", "", string.punctuation))
        # remove consecutive spaces, newlines, tabs in the middle and in the beginning / end
        text = re.sub(r"\s+", " ", text.strip())

        # Create a generator that yields the hash of each joined character n-gram
        hashes_gen = (
            self._hash_func("".join(char_ngram))  # Join the tuple of chars and hash
            for char_ngram in nltk.ngrams(text, self.config.n_grams) # Iterate through character n-grams
        )

        # Create numpy array from the generator of hashes
        shingles = np.fromiter(hashes_gen, dtype=np.uint64)

        # Reshape. np.fromiter correctly handles empty input -> empty array
        return shingles.reshape((-1, 1))


    def get_signature(self, shingles: np.ndarray) -> list[list[int]]:
        """Get the signature for a set of shingles (n-grams)

        Args:
            shingles: shingles (n-grams) numpy uint64 array of size (N, 1)

        Returns:
            list (num buckets) of lists of integers (hashes)
        """
        a, b = self.parameters
        phv = (shingles * a + b) % _mersenne_prime
        if self.config.hash_config.precision == 32:
            phv = np.bitwise_and(phv, self.config.hash_config.max)
        return [
            x.tolist()
            for x in np.split(np.min(phv, axis=0).astype(self.config.hash_config.np_dtype), self.config.num_buckets)
        ]

    def _global_minhash(self, signature: list[list[int]]) -> int:
        # Secondary key to break large buckets: min across full signature.
        return min(min(bucket) for bucket in signature) if signature else 0

    def _global_maxhash(self, signature: list[list[int]]) -> int:
        # Secondary key to break large buckets: max across full signature.
        return max(max(bucket) for bucket in signature) if signature else 0

    def run(self, data: DocumentStream, rank: int = 0, world_size: int = 1):
        logger.info(f"Starting signature generation for rank {rank}, world_size {world_size}")
        
        # Process documents and generate signatures
        doc_count = 0
        bucket_counts = [0] * self.config.num_buckets
        
        # Open bucket files
        buckets = [
            self.output_folder.open(f"bucket_{bi:03d}/{rank:05d}.minhash.sig", mode="wb")
            for bi in range(self.config.num_buckets)
        ]
        
        for doc_idx, doc in enumerate(data):
            doc_count += 1
            
            # Get shingles and signature
            shingles = self.get_shingles(doc.text)
            if shingles.size != 0:
                signature = self.get_signature(shingles)
                global_min_hash = self._global_minhash(signature)
                global_max_hash = self._global_maxhash(signature)
                split_bucket_idx = self.config.num_buckets // 2

                # Write signature to each bucket
                for bi, (bucket, bucket_sig) in enumerate(zip(buckets, signature)):
                    secondary_hash = global_min_hash if bi < split_bucket_idx else global_max_hash
                    bucket.write(
                        struct.pack(
                            f"<{self.config.hashes_per_bucket + 1}{self.config.hash_config.struct_format}I",
                            *bucket_sig,
                            secondary_hash,
                            doc_idx,                           
                        )
                    )
                    bucket_counts[bi] += 1
        
        # Close bucket files
        for bucket in buckets:
            bucket.close()
            
        # Sort each bucket file using numpy
        logger.info("Sorting buckets...")
        # Build NumPy dtype once; field1..fieldH = hashes, plus 3 uint32 meta fields
        dtype = np.dtype(
            [(f"hash{i}", f"<{self.config.hash_config.struct_format}")
            for i in range(1, self.config.hashes_per_bucket + 1)] +
            [( "secondary_hash", f"<{self.config.hash_config.struct_format}"), ("doc_idx", "<u4")]
        )
        record_size = dtype.itemsize

        for bi in range(self.config.num_buckets):
            path = f"bucket_{bi:03d}/{rank:05d}.minhash.sig"

            # Read entire file as a NumPy structured array
            with self.output_folder.open(path, mode="rb") as f:
                arr = np.fromfile(f, dtype=dtype)

            # Defensive check for partial/truncated records
            if arr.nbytes % record_size != 0:
                trim = arr.nbytes // record_size * record_size
                logger.warning(
                    "Bucket %d has %d stray byte(s); truncating to %d bytes",
                    bi, arr.nbytes - trim, trim
                )
                arr = arr.view(np.uint8)[:trim].view(dtype)

            # In-place lexicographic sort across all fields
            arr.sort(order=dtype.names)

            # Rewrite file atomically: open → tofile → close
            with self.output_folder.open(path, mode="wb") as f:
                arr.tofile(f)
        
        # Log final statistics
        logger.info(f"Finished processing {doc_count} documents")
        for bucket, count in enumerate(bucket_counts):
            logger.info(f"Bucket {bucket}: {count} signatures written")
        
        logger.info(f"Signature generation completed for rank {rank}")



class BucketStage(Stage):
    """Stage 2: complete duplicate buckets.

    Merge sorted band signatures into complete duplicate buckets.

    Each output record stores a bucket's member count followed by (reader, document)
    pairs. Matching uses the band signature; the secondary hash affects ordering
    but does not split buckets. External-index matching is not implemented here.

    Args:
        input_folder: input folder containing the signature from step 1
        output_folder: output folder for length-prefixed duplicate buckets (.dups)
        config: minhash configuration (a MinhashConfig object)
        lines_to_buffer: number of signature records to buffer per reader
    """

    name = "stage 2: buckets"

    def __init__(
        self,
        input_folder: DataFolderLike,
        output_folder: DataFolderLike,
        config: MinhashConfig = None,
        lines_to_buffer: int = 50_000,
    ):
        super().__init__()
        self.input_folder = get_datafolder(input_folder)
        self.output_folder = get_datafolder(output_folder)
        self.config = config or MinhashConfig()
        self.lines_to_buffer = lines_to_buffer

    def get_worker_hash_range(self, sig_files, rank, world_size):
        workers_per_bucket = world_size // self.config.num_buckets
        bucket, bucket_worker = divmod(rank, workers_per_bucket)
        hash_min, hash_max = (
            0,
            _mersenne_prime if self.config.hash_config.precision == 64 else self.config.hash_config.max,
        )
        if workers_per_bucket > 1 and len(sig_files):
            # take the first file and find bucket_worker boundaries. all workers in a bucket process the same set of
            # files, so this should be consistent across workers (and span the entire range of hashes)
            with self.input_folder.open(sig_files[0], mode="rb") as f:
                line_size = struct.calcsize(
                    f"{self.config.hashes_per_bucket + 1}{self.config.hash_config.struct_format}I"
                )
                L, rem = divmod(f.size, line_size)
                assert rem == 0, "file size not divisible by line size"
                assert L >= workers_per_bucket, f"tried to use {workers_per_bucket=} but there are only {L} lines"
                if bucket_worker > 0:
                    # not first
                    f.seek(line_size * (L // workers_per_bucket) * bucket_worker, os.SEEK_SET)
                    hash_min = struct.unpack(
                        self.config.hash_config.struct_format,
                        f.read(struct.calcsize(self.config.hash_config.struct_format)),
                    )[0]
                if bucket_worker + 1 < workers_per_bucket:
                    # not last
                    f.seek(line_size * (L // workers_per_bucket) * (bucket_worker + 1), os.SEEK_SET)
                    hash_max = struct.unpack(
                        self.config.hash_config.struct_format,
                        f.read(struct.calcsize(self.config.hash_config.struct_format)),
                    )[0]
        return hash_min, hash_max

    def run(self, data: DocumentStream = None, rank: int = 0, world_size: int = 1):
        assert data is None, "You should not use an input block before BucketStage"
        
        # Get bucket and bucket_worker from rank and world_size
        workers_per_bucket = world_size // self.config.num_buckets
        bucket, bucket_worker = divmod(rank, workers_per_bucket)
        logger.info(f"Starting bucket comparison for bucket {bucket}, worker {bucket_worker} (rank={rank}, world_size={world_size})")

        with self.track_time():
            # Read signatures from all files
            sig_files = self.input_folder.list_files(subdirectory=f"bucket_{bucket:03d}")
            logger.info(f"Found {len(sig_files)} signature files in bucket {bucket}")
            
            # Get hash range for this worker
            hash_min, hash_max = self.get_worker_hash_range(sig_files, rank, world_size)
            logger.info(
                f"Running worker {bucket_worker + 1}/{workers_per_bucket} on bucket {bucket:03d}. "
                f"Hash range: {[hash_min, hash_max]}"
            )
            
            # Initialize object pool for signature reuse (one per signature file)
            sig_pool = [HashSig(sig=(), secondary_hash=0, read_id=0, doc_id=-1) for _ in range(len(sig_files))]
            
            # Initialize signature readers with hash range filtering and object reuse
            sig_readers = [
                read_sigs_with_pool(
                    file,
                    read_id,
                    self.config,
                    sig_pool[read_id],  # Each reader gets its own pool object
                    min_hash=hash_min,
                    max_hash=hash_max,
                    lines_to_buffer=self.lines_to_buffer,
                )
                for read_id, file in enumerate(self.input_folder.open_files(sig_files, mode="rb"))
            ]
            
            
            pq = [x for x in [next(sig_reader, None) for sig_reader in sig_readers] if x is not None]
            heapq.heapify(pq)
            logger.info("Finished initializing signatures priority queue.")
            
            # Process signatures and find duplicates
            out_name = f"{bucket:03d}_{bucket_worker:02d}.dups"
            out_path = self.output_folder.resolve_paths(out_name)  # create a real local path
            os.makedirs(os.path.dirname(out_path), exist_ok=True)

            dup_count = 0
            last_sig = None
            last_secondary_hash = 0
            last_read_id = None
            last_doc_id = None
            dup_group = None
            count_struct = struct.Struct("<I")
            pair_struct = struct.Struct("<2I")
            with open(out_path, "wb") as out_f:
                def flush_dup_group():
                    nonlocal dup_group
                    if dup_group is None:
                        return
                    out_f.write(count_struct.pack(len(dup_group)))
                    for read_id, doc_id in dup_group:
                        out_f.write(pair_struct.pack(read_id, doc_id))
                    dup_group = None

                while pq:
                    v: HashSig = heapq.heappop(pq)
                #assert last_sig is None or (v.sig, v.secondary_hash) >= (last_sig, last_secondary_hash), f"HashSig queue sort error."
                
                    if not v.is_from_index() and last_sig is not None:
                        same_sig = last_sig == v.sig
                        same_secondary = last_secondary_hash == v.secondary_hash
                        #if same_sig and same_secondary:
                        if same_sig:
                            if dup_group is None:
                                dup_group = [(last_read_id, last_doc_id)]
                            dup_group.append((v.read_id, v.doc_id))
                            dup_count += 1
                            self.stat_update("total_matches")
                        else:
                            flush_dup_group()

                    last_sig, last_secondary_hash = v.sig, v.secondary_hash
                    last_read_id, last_doc_id = v.read_id, v.doc_id
                    next_sig = next(sig_readers[v.read_id], None)
                    if next_sig:
                        #assert next_sig >= v, f"Next sig sort error. {next_sig=} < {v=}"
                        heapq.heappush(pq, next_sig)

                flush_dup_group()
            
            logger.info(f"Finished processing bucket {bucket}, worker {bucket_worker}, producing {dup_count} dups")


class PruneStage(Stage):
    """Stage 2.5: cross-band bucket pruning.

    Remove cross-band subset/equality buckets from stage-2 `.dups` files.
    """

    name = "stage 2.5: prune"

    _count_struct = struct.Struct("<I")
    _pair_struct = struct.Struct("<2I")
    _read_id_mask = (1 << 32) - 1
    _dup_name_re = re.compile(r"^(\d+)_\d+\.dups$")

    def __init__(
        self,
        input_folder: DataFolderLike,
        output_folder: DataFolderLike,
        buffer_size_bytes: int = 16 * 1024 * 1024,
    ):
        super().__init__()
        self.input_folder = get_datafolder(input_folder)
        self.output_folder = get_datafolder(output_folder)
        self.buffer_size_bytes = buffer_size_bytes

    @classmethod
    def _pack_key(cls, read_id: int, doc_id: int) -> int:
        return (read_id << 32) | doc_id

    @classmethod
    def _unpack_key(cls, key: int) -> Tuple[int, int]:
        return key >> 32, key & cls._read_id_mask

    @staticmethod
    def _bucket_uid(file_id: int, bucket_idx: int) -> int:
        return (file_id << 32) | bucket_idx

    def _iter_bucket_rows(self, rel_path: str):
        with self.input_folder.open(rel_path, "rb") as raw:
            f = io.BufferedReader(raw, buffer_size=self.buffer_size_bytes)
            bucket_idx = 0
            while True:
                raw_count = f.read(self._count_struct.size)
                if not raw_count:
                    break
                if len(raw_count) != self._count_struct.size:
                    raise ValueError(f"Corrupt .dups file {rel_path}: truncated bucket count")
                dup_group_size = self._count_struct.unpack(raw_count)[0]
                row = []
                for _ in range(dup_group_size):
                    raw_pair = f.read(self._pair_struct.size)
                    if len(raw_pair) != self._pair_struct.size:
                        raise ValueError(f"Corrupt .dups file {rel_path}: truncated pair payload")
                    read_id, doc_id = self._pair_struct.unpack(raw_pair)
                    row.append(self._pack_key(read_id, doc_id))
                yield bucket_idx, row
                bucket_idx += 1

    def _collect_band_files(self, dup_files: List[str]) -> Tuple[Dict[int, List[str]], Dict[str, int]]:
        files_by_band: Dict[int, List[str]] = defaultdict(list)
        for rel_path in dup_files:
            name = os.path.basename(rel_path)
            m = self._dup_name_re.match(name)
            if m is None:
                raise ValueError(
                    f"Unexpected dup filename format: {name}. Expected '<band>_<index>.dups'"
                )
            band_id = int(m.group(1))
            files_by_band[band_id].append(rel_path)

        for band_id in files_by_band:
            files_by_band[band_id].sort()

        file_id_map = {path: idx for idx, path in enumerate(dup_files)}
        return files_by_band, file_id_map

    def _rewrite_pruned_band(
        self,
        target_files: List[str],
        remove_by_file: Dict[str, Set[int]],
    ) -> Tuple[int, int]:
        kept = 0
        dropped = 0
        for rel_path in target_files:
            remove_idxs = remove_by_file.get(rel_path)
            with self.output_folder.open(rel_path, "wb") as out_f:
                for bucket_idx, row in self._iter_bucket_rows(rel_path):
                    if remove_idxs and bucket_idx in remove_idxs:
                        dropped += 1
                        continue
                    out_f.write(self._count_struct.pack(len(row)))
                    for key in row:
                        read_id, doc_id = self._unpack_key(key)
                        out_f.write(self._pair_struct.pack(read_id, doc_id))
                    kept += 1
        return kept, dropped

    def _process_target_band(
        self,
        target_band: int,
        all_bands: List[int],
        files_by_band: Dict[int, List[str]],
        file_id_map: Dict[str, int],
        file_by_id: Dict[int, str],
        target_files_override: Optional[List[str]] = None,
    ) -> Tuple[int, int]:
        target_files = target_files_override if target_files_override is not None else files_by_band.get(target_band, [])
        if not target_files:
            logger.warning("Band {} has no files.", target_band)
            return 0, 0

        key_to_aidx: Dict[int, int] = {}
        a_lens: List[int] = []
        a_uids: List[int] = []
        a_file_ids: List[int] = []
        a_bucket_idxs: List[int] = []

        logger.info("Band {}: loading target buckets", target_band)
        for rel_path in tqdm(target_files, desc=f"Band {target_band}: load target", leave=False):
            file_id = file_id_map[rel_path]
            for bucket_idx, row in self._iter_bucket_rows(rel_path):
                if not row:
                    continue
                aidx = len(a_uids)
                a_uid = self._bucket_uid(file_id, bucket_idx)
                a_uids.append(a_uid)
                a_lens.append(len(row))
                a_file_ids.append(file_id)
                a_bucket_idxs.append(bucket_idx)
                for key in row:
                    key_to_aidx[key] = aidx

        if not a_uids:
            logger.warning("Band {}: no non-empty buckets found.", target_band)
            return 0, 0

        removed = bytearray(len(a_uids))
        removed_count = 0
        total_scanned = 0
        strict_subset_drops = 0
        equality_tiebreak_drops = 0

        for other_band in all_bands:
            if other_band == target_band:
                continue
            other_files = files_by_band.get(other_band, [])
            if not other_files:
                continue

            desc = f"Band {target_band}: scan band {other_band}"
            for rel_path in tqdm(other_files, desc=desc, leave=False):
                file_id_b = file_id_map[rel_path]
                for bucket_idx_b, row_b in self._iter_bucket_rows(rel_path):
                    total_scanned += 1

                    hits: Dict[int, int] = {}
                    for key in row_b:
                        aidx = key_to_aidx.get(key)
                        if aidx is None or removed[aidx]:
                            continue
                        hits[aidx] = hits.get(aidx, 0) + 1

                    if not hits:
                        continue

                    len_b = len(row_b)
                    b_uid = self._bucket_uid(file_id_b, bucket_idx_b)
                    for aidx, hit_count in hits.items():
                        if removed[aidx]:
                            continue
                        len_a = a_lens[aidx]
                        if hit_count != len_a:
                            continue
                        a_uid = a_uids[aidx]
                        if len_a < len_b:
                            removed[aidx] = 1
                            removed_count += 1
                            strict_subset_drops += 1
                        elif len_a == len_b and a_uid > b_uid:
                            removed[aidx] = 1
                            removed_count += 1
                            equality_tiebreak_drops += 1

                    if removed_count == len(a_uids):
                        break
                if removed_count == len(a_uids):
                    break
            if removed_count == len(a_uids):
                break

        remove_by_file: Dict[str, Set[int]] = defaultdict(set)
        for aidx, is_removed in enumerate(removed):
            if not is_removed:
                continue
            file_id = a_file_ids[aidx]
            bucket_idx = a_bucket_idxs[aidx]
            rel_path = file_by_id[file_id]
            remove_by_file[rel_path].add(bucket_idx)

        kept, dropped = self._rewrite_pruned_band(
            target_files=target_files,
            remove_by_file=remove_by_file,
        )
        logger.info(
            "Band {}: scanned {} buckets in other bands, kept {}, dropped {} (strict_subset={}, equality_tiebreak={})",
            target_band,
            total_scanned,
            kept,
            dropped,
            strict_subset_drops,
            equality_tiebreak_drops,
        )
        return kept, dropped

    def run(self, data: DocumentStream = None, rank: int = 0, world_size: int = 1):
        assert rank < world_size, f"rank ({rank}) is out of bounds for world_size ({world_size})"

        with self.track_time():
            dup_files = self.input_folder.list_files(glob_pattern="*.dups")
            if not dup_files:
                logger.warning("No .dups files found under {}", self.input_folder.path)
                return

            files_by_band, file_id_map = self._collect_band_files(dup_files)
            file_by_id = {idx: path for path, idx in file_id_map.items()}
            bands = sorted(files_by_band)
            n_bands = len(bands)
            if world_size < n_bands or world_size % n_bands != 0:
                raise ValueError(
                    f"PruneStage expects world_size to be a multiple of number of bands "
                    f"(world_size={world_size}, bands={n_bands})."
                )

            workers_per_band = world_size // n_bands
            band_pos = rank % n_bands
            band_worker_idx = rank // n_bands
            assigned_band = bands[band_pos]
            host_files = files_by_band.get(assigned_band, [])
            target_files = host_files[band_worker_idx::workers_per_band]

            if not target_files:
                logger.info(
                    "Rank {}: host band {} shard {}/{} has no assigned files.",
                    rank,
                    assigned_band,
                    band_worker_idx,
                    workers_per_band,
                )
                return

            logger.info(
                "Stage 2.5 rank {}/{} processing host band {} shard {}/{} ({} files)",
                rank,
                world_size,
                assigned_band,
                band_worker_idx,
                workers_per_band,
                len(target_files),
            )

            total_kept = 0
            total_dropped = 0
            kept, dropped = self._process_target_band(
                target_band=assigned_band,
                all_bands=bands,
                files_by_band=files_by_band,
                file_id_map=file_id_map,
                file_by_id=file_by_id,
                target_files_override=target_files,
            )
            total_kept += kept
            total_dropped += dropped

            logger.info(
                "Stage 2.5 rank {} done. kept={}, dropped={}",
                rank,
                total_kept,
                total_dropped,
            )

def _pack_dup_key(read_id: int, doc_id: int) -> int:
    return (read_id << 32) | doc_id

def _unpack_dup_key(key: int) -> Tuple[int, int]:
    return key >> 32, key & SENTINEL


def _load_dup_buckets(
    input_folder,
    dup_files,
    lines_to_buffer: int,
    selected_bucket_ids: Optional[Set[int]] = None,
    desc: str = "Reading dup files",
) -> Tuple[List[List[int]], Dict[int, int], Dict[int, int]]:
    key_weights: Dict[int, int] = {}
    bucket_keys: List[List[int]] = []
    band_dup_freq: Dict[int, int] = {}
    count_struct = struct.Struct("<I")
    pair_struct = struct.Struct("<2I")
    buffer_size = (1000 + lines_to_buffer) * pair_struct.size
    bucket_idx = 0

    for dup_file in tqdm(dup_files, desc=desc):
        with input_folder.open(dup_file, "rb") as raw_dupf:
            dupf = io.BufferedReader(raw_dupf, buffer_size=buffer_size)
            while True:
                raw = dupf.read(count_struct.size)
                if not raw:
                    break
                dup_group_size = count_struct.unpack(raw)[0]
                row = []
                for _ in range(dup_group_size):
                    read_id, doc_id = pair_struct.unpack(dupf.read(pair_struct.size))
                    row.append(_pack_dup_key(read_id, doc_id))
                if selected_bucket_ids is None or bucket_idx in selected_bucket_ids:
                    band_dup_freq[dup_group_size] = band_dup_freq.get(dup_group_size, 0) + 1
                    bucket_keys.append(row)
                    for key in row:
                        key_weights[key] = key_weights.get(key, 0) + 1
                bucket_idx += 1

    return bucket_keys, key_weights, band_dup_freq

class SelectionStage(Stage):
    """Stage 3: bucket-feasible representative selection.

    Select representatives from complete duplicate buckets using weight-layered
    greedy selection, weight-1 preprocessing, and equal-support compression.
    Documents are assigned through buckets containing a selected representative;
    filtering keeps that representative and removes its assigned duplicates.

    This is not union-find over transitive connected components: matches A-B and
    B-C do not by themselves require all three documents to share one cluster.
    compute_cov_bound() and compute_punct_bound() compute separate diagnostics;
    they do not write the removal decisions produced by run().

    Args:
        signature_folder: signature files used for optional post-selection analysis
        input_folder: folder containing the .dups files
        output_folder: folder to write the cluster files to
        config: minhash configuration
        save_cluster_size: whether to save cluster sizes
        lines_to_buffer: number of lines to buffer when reading files
        analyze_clustering: analyze retained-document signature similarities after clustering
    """

    name = "stage 3: selection"

    def __init__(
        self,
        signature_folder: DataFolderLike,
        input_folder: DataFolderLike,
        output_folder: DataFolderLike,
        config: MinhashConfig = None,
        save_cluster_size: bool = True,
        lines_to_buffer: int = 50_000,
        analyze_clustering: bool = False,
    ):
        super().__init__()
        self.signature_folder = get_datafolder(signature_folder)
        self.input_folder = get_datafolder(input_folder)
        self.output_folder = get_datafolder(output_folder)
        self.config = config or MinhashConfig()
        self.save_cluster_size = save_cluster_size
        self.lines_to_buffer = lines_to_buffer
        self.analyze_clustering = analyze_clustering
        logger.info(f"SelectionStage config: {asdict(self.config)}")

    @staticmethod
    def _compress_equal_support_keys(
        bucket_keys: List[Optional[List[int]]],
        compressed_keys_by_rep: Optional[Dict[int, List[int]]] = None,
    ) -> Tuple[Dict[int, int], int]:
        """Compress active keys with identical live bucket support exactly.

        This builds the reverse support map, sorts by ``(support, key)`` so the
        smallest key is first in every identical-support group, then rebuilds
        bucket rows from representatives only. The returned weights are the
        representative support lengths. If provided, ``compressed_keys_by_rep``
        records removed keys under their representative. Rows are released
        during the reverse pass to avoid keeping both old and rebuilt bucket
        contents live.
        """
        key_to_bucket_ids: Dict[int, List[int]] = defaultdict(list)
        for bucket_id, dup_group in enumerate(bucket_keys):
            if dup_group is None:
                continue
            for key in dup_group:
                key_to_bucket_ids[key].append(bucket_id)
            bucket_keys[bucket_id] = None

        if not key_to_bucket_ids:
            return {}, 0

        items = sorted(key_to_bucket_ids.items(), key=lambda item: (item[1], item[0]))
        key_to_bucket_ids.clear()
        key_weights: Dict[int, int] = {}
        alias_count = 0
        item_idx = 0
        while item_idx < len(items):
            rep_key, rep_bucket_ids = items[item_idx]
            key_weights[rep_key] = len(rep_bucket_ids)
            for bucket_id in rep_bucket_ids:
                if bucket_keys[bucket_id] is None:
                    bucket_keys[bucket_id] = []
                bucket_keys[bucket_id].append(rep_key)

            next_idx = item_idx + 1
            while next_idx < len(items) and items[next_idx][1] == rep_bucket_ids:
                alias = items[next_idx][0]
                alias_count += 1
                if compressed_keys_by_rep is not None:
                    assert alias not in compressed_keys_by_rep
                    compressed_keys_by_rep.setdefault(rep_key, []).append(alias)
                next_idx += 1
            item_idx = next_idx

        return key_weights, alias_count

    def _reduce_support_dominated_keys(
        self,
        bucket_keys: List[Optional[List[int]]],
        key_weights: Dict[int, int],
    ) -> Optional[Tuple[int, List[int], Dict[int, List[int]], float, int, int]]:
        """Remove keys whose live bucket support strictly contains another key's support.

        Bucket layers are scanned from low to high weight while key supports are
        accumulated globally. When a key's full support is revealed, intersecting
        those support buckets finds active higher-degree keys whose support contains it.
        Surviving key weights are not decremented; dominated keys are deleted from
        the active key set. Bucket weights are checked lazily because suppressing
        keys can raise a bucket's effective layer.
        """
        if not key_weights:
            return None

        weight_buckets: Dict[int, List[int]] = defaultdict(list)
        max_bucket_wt = 0
        for bucket_id, dup_group in enumerate(bucket_keys):
            if dup_group is None:
                continue
            active_keys = [key for key in dup_group if key in key_weights]
            if not active_keys:
                bucket_keys[bucket_id] = None
                continue
            if len(active_keys) != len(dup_group):
                bucket_keys[bucket_id] = active_keys
            min_wt = min(key_weights[key] for key in active_keys)
            weight_buckets[min_wt].append(bucket_id)
            if min_wt > max_bucket_wt:
                max_bucket_wt = min_wt

        seen_bucket_ids: Dict[int, List[int]] = defaultdict(list)
        suppressed_count = len(key_weights)
        bucket_wt = 1
        while bucket_wt <= max_bucket_wt:
            layer_bucket_ids = weight_buckets.pop(bucket_wt, [])
            if not layer_bucket_ids:
                bucket_wt += 1
                continue

                       
            for bucket_id in layer_bucket_ids:
                dup_group = bucket_keys[bucket_id]
                if dup_group is None:
                    continue
                active_keys = [key for key in dup_group if key in key_weights]
                if not active_keys:
                    bucket_keys[bucket_id] = None
                    continue
                if len(active_keys) != len(dup_group):
                    bucket_keys[bucket_id] = active_keys
                current_wt = min(key_weights[key] for key in active_keys)
                if current_wt > bucket_wt:
                    weight_buckets[current_wt].append(bucket_id)
                    if current_wt > max_bucket_wt:
                        max_bucket_wt = current_wt
                    continue
                assert current_wt == bucket_wt, (
                    f"Bucket layer moved backwards: {bucket_id=}, {bucket_wt=}, {current_wt=}"
                )
                for key in active_keys:
                    seen_bucket_ids[key].append(bucket_id)

            full_support_rep: Dict[Tuple[int, ...], int] = {}
            full_keys = []
            for key in seen_bucket_ids:        
                if len(seen_bucket_ids[key]) == bucket_wt and key_weights[key]==bucket_wt:
                    full_keys.append(key)
                    support = tuple(seen_bucket_ids[key])
                    rep = full_support_rep.get(support)
                    if rep is None or key < rep:
                        full_support_rep[support] = key

            removed_keys = []            
            for key in seen_bucket_ids:        
                if len(seen_bucket_ids[key]) == bucket_wt and key_weights[key]>=bucket_wt:
                    support = tuple(seen_bucket_ids[key])  
                    rep = full_support_rep.get(support)
                    if rep is None or rep == key:
                        continue  
                    key_weights.pop(key, None)   
                    removed_keys.append(key)  

            for key in full_keys:
                seen_bucket_ids.pop(key, None) 
            for key in removed_keys:
                seen_bucket_ids.pop(key, None)                          

            bucket_wt += 1

        suppressed_count -= len(key_weights)
        if suppressed_count:
            for bucket_id, dup_group in enumerate(bucket_keys):
                if dup_group is None:
                    continue
                active_keys = [key for key in dup_group if key in key_weights]
                bucket_keys[bucket_id] = active_keys if active_keys else None

            bucket_wts, weight_buckets, residual_bound, min_key_wt, max_key_wt = (
                self._rebuild_layers_from_key_weights(bucket_keys, key_weights)
            )
            return suppressed_count, bucket_wts, weight_buckets, residual_bound, min_key_wt, max_key_wt

        return None

    @staticmethod
    def _rebuild_layers_from_key_weights(
        bucket_keys: List[Optional[List[int]]],
        key_weights: Dict[int, int],
    ) -> Tuple[List[int], Dict[int, List[int]], float, int, int]:
        """Recompute bucket layers from live bucket rows without mutating them."""
        bucket_wts = [0] * len(bucket_keys)
        weight_buckets: Dict[int, List[int]] = defaultdict(list)
        residual_bound = 0.0
        if not key_weights:
            return bucket_wts, weight_buckets, residual_bound, 0, 0

        for idx, dup_group in enumerate(bucket_keys):
            if dup_group is None:
                continue
            assert dup_group, f"Empty live bucket row at {idx=}"
            assert all(key in key_weights for key in dup_group), f"Stale key in live bucket row at {idx=}"
            min_wt = min(key_weights[key] for key in dup_group)
            bucket_wts[idx] = min_wt
            weight_buckets[min_wt].append(idx)
            residual_bound += 1 / min_wt

        min_key_wt = min(key_weights.values())
        max_key_wt = max(key_weights.values())
        return bucket_wts, weight_buckets, residual_bound, min_key_wt, max_key_wt

    def _preclude_w1_and_rebuild_layers(
        self,
        bucket_keys: List[Optional[List[int]]],
        key_weights: Dict[int, int],
        *,
        compressed_keys_by_rep: Optional[Dict[int, List[int]]] = None,
        cluster_sets: Optional[Dict[int, int]] = None,
        cluster_size: Optional[Dict[int, int]] = None,
        cover_sets: Optional[Dict[int, int]] = None,
        cover_bucket_ids: Optional[List[int]] = None,
        cover_bucket_rows: Optional[List[List[int]]] = None,
        log_prefix: str = "Cluster",
    ) -> Tuple[Dict[int, int], List[int], Dict[int, List[int]], float, int, int, int, int]:
        """Preclude all weight-1 buckets, compress residual equal-support keys, and rebuild layers."""
        emit_clusters = cluster_sets is not None
        assert emit_clusters == (cluster_size is not None)
        emit_cover = cover_sets is not None
        if emit_cover:
            assert cover_bucket_ids is not None
        else:
            assert cover_bucket_ids is None and cover_bucket_rows is None
        w1_bucket_count = 0
        total_aliases = 0

        if not key_weights:
            return {}, [0] * len(bucket_keys), defaultdict(list), 0.0, 0, 0, w1_bucket_count, total_aliases

        absorbed_keys: Set[int] = set()
        absorbed_buckets = 0
        for idx, dup_group in enumerate(bucket_keys):
            if dup_group is None:
                continue
            w1_keys = [key for key in dup_group if key_weights[key] == 1]
            if not w1_keys:
                continue
            root_key = min(w1_keys)
            active_keys = [key for key in dup_group if key not in absorbed_keys]
            if not active_keys:
                bucket_keys[idx] = None
                continue
            assert root_key in active_keys
            if emit_clusters:
                cluster_size.setdefault(root_key, 0)
                for key in active_keys:
                    assert key not in cluster_sets
                    cluster_sets[key] = root_key
                    cluster_size[root_key] += 1
            if emit_cover:
                for key in active_keys:
                    assert key not in cover_sets
                    cover_sets[key] = idx
                cover_bucket_ids.append(idx)
                if cover_bucket_rows is not None:
                    cover_bucket_rows.append(list(active_keys))
            absorbed_keys.update(active_keys)
            bucket_keys[idx] = None
            absorbed_buckets += 1
            w1_bucket_count += 1

        if absorbed_keys:
            for idx, dup_group in enumerate(bucket_keys):
                if dup_group is None:
                    continue
                active_keys = [key for key in dup_group if key not in absorbed_keys]
                bucket_keys[idx] = active_keys if active_keys else None
            logger.info(
                "{} precluded {} weight-1 buckets; weight-1 bucket count now {}",
                log_prefix,
                absorbed_buckets,
                w1_bucket_count,
            )

        key_weights.clear()
        key_weights = None
        key_weights, alias_count = self._compress_equal_support_keys(
            bucket_keys,
            compressed_keys_by_rep,
        )
        if alias_count:
            total_aliases += alias_count
            logger.info("{} compressed {} residual equal-support docs", log_prefix, alias_count)

        bucket_wts, weight_buckets, residual_bound, min_key_wt, max_key_wt = (
            self._rebuild_layers_from_key_weights(bucket_keys, key_weights)
        )
        if not key_weights:
            return {}, bucket_wts, weight_buckets, residual_bound, 0, 0, w1_bucket_count, total_aliases
        return key_weights, bucket_wts, weight_buckets, residual_bound, min_key_wt, max_key_wt, w1_bucket_count, total_aliases

    def compute_punct_bound(self, data: DocumentStream = None, _: int = 0, world_size: int = 1):
        dup_files = self.input_folder.list_files(glob_pattern="*.dups")
        assert world_size == 1, "World size must be 1 for clustering"

        with self.track_time():
            bucket_keys, key_weights, band_dup_freq = _load_dup_buckets(
                self.input_folder,
                dup_files,
                self.lines_to_buffer,
            )
            if not key_weights or not band_dup_freq:
                logger.info("No duplicate; skipping clustering")
                return 
 
            upper_bound_clusters = 0
            for dup_group in bucket_keys:
                upper_bound_clusters += 1 / min(key_weights[key] for key in dup_group)
            w1_bound = 0.0

            upper_bound_clusters_punct = w1_bound
            round_id = 0
            while True:
                round_id += 1
                (
                    key_weights,
                    bucket_wts,
                    weight_buckets,
                    residual_bound,
                    _,
                    max_key_wt,
                    w1_delta,
                    _,
                ) = self._preclude_w1_and_rebuild_layers(bucket_keys, key_weights, log_prefix="Puncture rebuild")
                w1_bound += w1_delta
                preclude_w1_bound = w1_bound + residual_bound

                dominated_rebuild = self._reduce_support_dominated_keys(bucket_keys, key_weights)
                suppressed_this_rebuild = 0
                if dominated_rebuild is not None:
                    (
                        suppressed_this_rebuild,
                        bucket_wts,
                        weight_buckets,
                        residual_bound,
                        _,
                        max_key_wt,
                    ) = dominated_rebuild

                round_bound = w1_bound + residual_bound
                round_start_bound = round_bound
                upper_bound_clusters_punct = round_bound
                removed_this_round = 0

                def log_round_summary():
                    logger.info(
                        "Round {} bound summary: original-form={}, post Wt-1 preclusion={}, post support-dominated reduction={}, ending bound={}, candidate keys={}, suppressed docs={}, removed buckets={}, improvement={}",
                        round_id,
                        int(upper_bound_clusters),
                        int(preclude_w1_bound),
                        int(round_start_bound),
                        int(round_bound),
                        len(key_weights),
                        int(suppressed_this_rebuild),
                        removed_this_round,
                        round_start_bound - round_bound,
                    )

                if not key_weights:
                    log_round_summary()
                    break

                key_to_bucket_ids: Dict[int, Set[int]] = defaultdict(set)
                for bucket_wt in range(max_key_wt, 1, -1):
                    if not weight_buckets[bucket_wt]:
                        continue
                    logger.info("Puncture round {}, clustering on bucket weight {}", round_id, bucket_wt)
                    
                    bucket_scores: Dict[int, int] = {}
                    score_buckets: Dict[int, Set[int]] = defaultdict(set)
                    current_min_score = len(key_weights)
                    for bucket_id in weight_buckets[bucket_wt]:
                        if bucket_keys[bucket_id] is None or bucket_wts[bucket_id] != bucket_wt:
                            continue
                        
                        min_wt_cnt = 0
                        this_bkt_keys = bucket_keys[bucket_id]
                        assert this_bkt_keys
                        for key in this_bkt_keys:
                            if key in key_weights:
                                key_to_bucket_ids[key].add(bucket_id)
                                if key_weights[key] == bucket_wt:
                                    min_wt_cnt += 1

                        bucket_scores[bucket_id] = min_wt_cnt
                        score_buckets[min_wt_cnt].add(bucket_id)
                        if min_wt_cnt < current_min_score:
                            current_min_score = min_wt_cnt

                    MAX_SCORE = bucket_wt + 1
                    while bucket_scores:
                        while current_min_score < MAX_SCORE and not score_buckets[current_min_score]:
                            current_min_score += 1
                        if current_min_score >= MAX_SCORE:
                            break
                        bucket_id = score_buckets[current_min_score].pop()
                        assert bucket_id in bucket_scores
                        old_score = bucket_scores.pop(bucket_id)
                        assert old_score == current_min_score
                        score_buckets[old_score].discard(bucket_id)
                        
                        this_bkt_keys = bucket_keys[bucket_id]
                        assert this_bkt_keys is not None
                        assert bucket_wts[bucket_id] == bucket_wt
                        punct_ids = set()
                        for key in this_bkt_keys:
                            assert key in key_weights
                            assert key in key_to_bucket_ids
                            key_wt = key_weights[key]
                            key_ids = key_to_bucket_ids[key]
                            assert key_wt >= bucket_wt
                            if key_wt == bucket_wt:
                                assert len(key_ids) <= bucket_wt
                                for bkt_id in key_ids:
                                    if bkt_id != bucket_id:
                                        punct_ids.add(bkt_id)
                            else:
                                for bkt_id in key_ids:
                                    if bkt_id != bucket_id and bucket_wts[bkt_id] == key_wt:
                                        punct_ids.add(bkt_id)

                        score = 1 / bucket_wt
                        for bkt_id in punct_ids:
                            bkt_wt = bucket_wts[bkt_id]
                            if bkt_wt == 1:
                                score = -1
                                break
                            assert bkt_wt > 1
                            score -= 1 / (bkt_wt - 1) - 1 / bkt_wt
                        score_eps = 1e-12
                        if abs(score) < score_eps:
                            score = 0.0
                        accept_score = score >= -score_eps
                        if accept_score:
                            round_bound -= score
                            upper_bound_clusters_punct = round_bound
                            removed_this_round += 1
                            for key in this_bkt_keys:
                                assert key in key_weights
                                assert key in key_to_bucket_ids
                                key_to_bucket_ids[key].discard(bucket_id)
                                key_weights[key] -= 1
                                if key_weights[key] == bucket_wt:
                                    for bkt_id in key_to_bucket_ids[key]:
                                        if bucket_keys[bkt_id] and bucket_wts[bkt_id] == bucket_wt and bkt_id in bucket_scores:
                                            min_wt_cnt = bucket_scores[bkt_id]
                                            score_buckets[min_wt_cnt].discard(bkt_id)
                                            bucket_scores[bkt_id] += 1
                                            score_buckets[min_wt_cnt + 1].add(bkt_id)

                            for bkt_id in punct_ids:
                                old_wt = bucket_wts[bkt_id]
                                assert bucket_keys[bkt_id]
                                active_key_wts = [key_weights[key] for key in bucket_keys[bkt_id] if key in key_weights]
                                assert active_key_wts
                                bucket_wts[bkt_id] = min(active_key_wts)
                                if old_wt>= bucket_wt and bucket_wts[bkt_id] < bucket_wt:
                                    weight_buckets[bucket_wts[bkt_id]].append(bkt_id)
                                    min_wt_cnt = bucket_scores.pop(bkt_id, None)
                                    if min_wt_cnt is not None:
                                        score_buckets[min_wt_cnt].discard(bkt_id)

                            bucket_keys[bucket_id] = None

                    # Remove keys only when no live associated bucket can be reprocessed in this or a lower layer.
                    keys_to_remove = []
                    for key in key_to_bucket_ids:
                        if (
                            len(key_to_bucket_ids[key]) == key_weights[key]
                            and key_weights[key] > bucket_wt
                            and all(
                                bucket_keys[bkt_id] is None or bucket_wts[bkt_id] > bucket_wt
                                for bkt_id in key_to_bucket_ids[key]
                            )
                        ):
                            keys_to_remove.append(key)
                    for key in keys_to_remove:
                        for bkt_id in key_to_bucket_ids[key]:
                            if bucket_keys[bkt_id] is not None:
                                active_keys = [bkt_key for bkt_key in bucket_keys[bkt_id] if bkt_key != key]
                                if active_keys:
                                    bucket_keys[bkt_id] = active_keys
                                else:
                                    bucket_keys[bkt_id] = None
                                    bucket_wts[bkt_id] = 0
                        key_weights.pop(key, None)
                        key_to_bucket_ids.pop(key)

                log_round_summary()
                upper_bound_clusters_punct = round_bound
                if removed_this_round == 0:
                    break

    def compute_cov_bound(self, data: DocumentStream = None, _: int = 0, world_size: int = 1):
        dup_files = self.input_folder.list_files(glob_pattern="*.dups")
        assert world_size == 1, "World size must be 1 for clustering"
        cover_sets = {}  # map a covered key to the first covering bucket id

        with self.track_time():
            bucket_keys, key_weights, band_dup_freq = _load_dup_buckets(
                self.input_folder,
                dup_files,
                self.lines_to_buffer,
            )

            if not key_weights or not band_dup_freq:
                logger.info("No duplicate; skipping covering bound")
                return 0, 0.0
            initial_key_count = len(key_weights)

            doc_weight_freq: Dict[int, int] = defaultdict(int)
            for wt in key_weights.values():
                doc_weight_freq[wt] += 1

            logger.info(
                "Completed loading, Number of unique buckets: {}, unique docs: {}, Bucket Duplicate freq stats:",
                len(bucket_keys),
                len(key_weights),
            )
            for dup_count, freq in sorted(band_dup_freq.items()):
                logger.info("{}: {}", dup_count, freq)
            logger.info("Doc weight distribution (weight: number_of_docs):")
            for wt, doc_cnt in sorted(doc_weight_freq.items()):
                logger.info("{}: {}", wt, doc_cnt)

            cover_bucket_ids: List[int] = []
            cover_aliases_by_rep: Dict[int, List[int]] = {}

            (
                key_weights,
                _,
                weight_buckets,
                _,
                min_key_wt,
                max_key_wt,
                w1_cover_count,
                alias_count,
            ) = self._preclude_w1_and_rebuild_layers(
                bucket_keys,
                key_weights,
                compressed_keys_by_rep=cover_aliases_by_rep,
                cover_sets=cover_sets,
                cover_bucket_ids=cover_bucket_ids,
                log_prefix="Covering rebuild",
            )

            if not key_weights:
                for rep, aliases in cover_aliases_by_rep.items():
                    assert rep in cover_sets
                    for alias in aliases:
                        assert alias not in cover_sets
                        cover_sets[alias] = cover_sets[rep]
                assert initial_key_count == len(cover_sets), (
                    "Covering sanity check failed in compute_cov_bound: "
                    f"initial key_weights={initial_key_count}, final cover_sets={len(cover_sets)}"
                )
                logger.info(
                    "Full coverage achieved by {} weight-1 covering buckets ({} equal-support docs compressed); this is unbeatable and no further covering iteration is needed",
                    w1_cover_count,
                    alias_count,
                )
                logger.info(
                    "Covering bucket count bound: {}, Restricted covering bound: {}, covered docs: {}, cover_buckets.dups export skipped",
                    len(cover_bucket_ids),
                    len(cover_bucket_ids),
                    len(cover_sets),
                )
                return len(cover_bucket_ids), float(len(cover_bucket_ids))

            for bucket_wt in range(min_key_wt, max_key_wt + 1):
                if not weight_buckets[bucket_wt]:
                    continue
                logger.info("Covering on bucket weight {}", bucket_wt)

                key_to_bucket_ids: Dict[int, List[int]] = defaultdict(list)
                bucket_scores: Dict[int, int] = {}
                score_buckets: Dict[int, Set[int]] = defaultdict(set)
                current_max_score = 0
                for bucket_id in weight_buckets[bucket_wt]:
                    if bucket_keys[bucket_id] is None:
                        continue
                    update_keys = []
                    min_wt_cnt = 0
                    next_wt = max_key_wt
                    # Residual bucket rows have been rebuilt with representative keys only.
                    # Equal-support aliases are absent here and are expanded after cover selection.
                    for key in bucket_keys[bucket_id]:
                        if key not in cover_sets:
                            update_keys.append(key)
                            wt = key_weights[key]
                            next_wt = min(next_wt, wt)
                            if wt == bucket_wt:
                                key_to_bucket_ids[key].append(bucket_id)
                                min_wt_cnt += 1
                    bucket_keys[bucket_id] = update_keys if update_keys else None
                    if not update_keys:
                        continue
                    if min_wt_cnt == 0:
                        weight_buckets[next_wt].append(bucket_id)
                        continue
                    bucket_scores[bucket_id] = min_wt_cnt
                    score_buckets[min_wt_cnt].add(bucket_id)
                    if min_wt_cnt > current_max_score:
                        current_max_score = min_wt_cnt
                weight_buckets[bucket_wt] = []

                while bucket_scores:
                    while current_max_score > 0 and not score_buckets[current_max_score]:
                        current_max_score -= 1
                    if current_max_score == 0:
                        break
                    bucket_id = score_buckets[current_max_score].pop()

                    active_keys = []
                    for key in bucket_keys[bucket_id]:
                        if key not in cover_sets:
                            active_keys.append(key)

                    if not active_keys:
                        bucket_keys[bucket_id] = None
                        bucket_scores.pop(bucket_id, None)
                        continue

                    bucket_keys[bucket_id] = None

                    old_cnt = bucket_scores.pop(bucket_id)
                    score_buckets[old_cnt].discard(bucket_id)
                    cover_bucket_ids.append(bucket_id)

                    for ass_key in active_keys:
                        cover_sets[ass_key] = bucket_id
                        if key_weights[ass_key] == bucket_wt:
                            for ass_bucket_id in key_to_bucket_ids[ass_key]:
                                old_cnt = bucket_scores.get(ass_bucket_id)
                                if old_cnt is None:
                                    continue
                                score_buckets[old_cnt].discard(ass_bucket_id)
                                next_cnt = old_cnt - 1
                                if next_cnt == 0:
                                    bucket_scores.pop(ass_bucket_id, None)
                                    weight_buckets[bucket_wt + 1].append(ass_bucket_id)
                                else:
                                    bucket_scores[ass_bucket_id] = next_cnt
                                    score_buckets[next_cnt].add(ass_bucket_id)

            cover_bucket_id_set = set(cover_bucket_ids)
            cover_bucket_keys, cover_key_weights, _ = _load_dup_buckets(
                self.input_folder,
                dup_files,
                self.lines_to_buffer,
                selected_bucket_ids=cover_bucket_id_set,
                desc="Reading covering buckets",
            )

            count_struct = struct.Struct("<I")
            pair_struct = struct.Struct("<2I")
            with self.output_folder.open("cover_buckets.dups", mode="wb") as out_f:
                for dup_group in cover_bucket_keys:
                    out_f.write(count_struct.pack(len(dup_group)))
                    for key in dup_group:
                        read_id, doc_id = _unpack_dup_key(key)
                        out_f.write(pair_struct.pack(read_id, doc_id))

            restricted_cov_bound = 0.0
            for dup_group in cover_bucket_keys:
                min_wt = min(cover_key_weights[key] for key in dup_group)
                restricted_cov_bound += 1 / min_wt

            for rep, aliases in cover_aliases_by_rep.items():
                assert rep in cover_sets
                for alias in aliases:
                    assert alias not in cover_sets
                    cover_sets[alias] = cover_sets[rep]
            assert initial_key_count == len(cover_sets), (
                "Covering sanity check failed in compute_cov_bound: "
                f"initial key_weights={initial_key_count}, final cover_sets={len(cover_sets)}"
            )
            logger.info(
                "Covering bucket count bound: {}, Restricted covering bound: {}, covered docs: {}, equal-support docs compressed: {}",
                len(cover_bucket_ids),
                int(restricted_cov_bound),
                len(cover_sets),
                alias_count,
            )
            return len(cover_bucket_ids), int(restricted_cov_bound)

    def run(self, data: DocumentStream = None, _: int = 0, world_size: int = 1):
        dup_files = self.input_folder.list_files(glob_pattern="*.dups")
        assert world_size == 1, "World size must be 1 for clustering"
        cluster_sets = {}      # map a clustered key to its root
        cluster_size = {}   # map a root to its clustered size

        with self.track_time():
            logger.info("Loading dup files...")
            bucket_keys, key_weights, band_dup_freq = _load_dup_buckets(
                self.input_folder,
                dup_files,
                self.lines_to_buffer,
            )
                        
            if not key_weights or not band_dup_freq:
                logger.info("No duplicate; skipping clustering")
                return
            initial_key_count = len(key_weights)

            doc_weight_freq: Dict[int, int] = defaultdict(int)
            for wt in key_weights.values():
                doc_weight_freq[wt] += 1
            
            logger.info(
                "Completed loading, Number of unique buckets: {}, unique docs: {}, Bucket Duplicate freq stats:",
                len(bucket_keys),
                len(key_weights),
            )
            for dup_count, freq in sorted(band_dup_freq.items()):
                logger.info("{}: {}", dup_count, freq)    
            logger.info("Doc weight distribution (weight: number_of_docs):")
            for wt, doc_cnt in sorted(doc_weight_freq.items()):
                logger.info("{}: {}", wt, doc_cnt)

            compressed_keys_by_rep: Dict[int, List[int]] = {}

            (
                key_weights,
                _,
                weight_buckets,
                _,
                min_key_wt,
                max_key_wt,
                w1_cluster_count,
                alias_count,
            ) = self._preclude_w1_and_rebuild_layers(
                bucket_keys,
                key_weights,
                compressed_keys_by_rep=compressed_keys_by_rep,
                cluster_sets=cluster_sets,
                cluster_size=cluster_size,
                log_prefix="Cluster rebuild",
            )

            logger.info(
                "equal-support docs compressed: {}", alias_count,
            )
            if not weight_buckets:
                logger.info("No residual buckets remain after weight-1 preprocessing")
            else:
                # remove minimum weight to reduce memory
                keys_to_drop = [k for k, v in key_weights.items() if v == min_key_wt]
                for key in keys_to_drop:
                    key_weights.pop(key, None)

                for bucket_wt in range(min_key_wt, max_key_wt + 1):
                    if not weight_buckets[bucket_wt]:
                        continue
                    logger.info("Clustering on bucket weight {}", bucket_wt)
                    # Process higher tie-count first within the same weight.

                    key_to_bucket_ids: Dict[int, List[int]] = defaultdict(list)
                    bucket_score: Dict[int, int] = {}
                    score_buckets: Dict[int, Set[int]] = defaultdict(set)
                    current_max_score = 0
                    for bucket_id in weight_buckets[bucket_wt]:
                        if bucket_keys[bucket_id] is None:
                            continue
                        update_keys = []
                        min_wt_cnt = 0
                        for key in bucket_keys[bucket_id]:
                            if key not in cluster_sets:
                                update_keys.append(key)
                                if key_weights.get(key, min_key_wt) == bucket_wt:
                                    key_to_bucket_ids[key].append(bucket_id)
                                    min_wt_cnt += 1
                        bucket_keys[bucket_id] = update_keys if update_keys else None
                        if not update_keys:
                            continue
                        if min_wt_cnt == 0:
                            bkt_wt = min(key_weights[key] for key in update_keys)
                            weight_buckets[bkt_wt].append(bucket_id)
                            continue
                        bucket_score[bucket_id] = min_wt_cnt
                        score_buckets[min_wt_cnt].add(bucket_id)
                        if min_wt_cnt > current_max_score:
                            current_max_score = min_wt_cnt
                    weight_buckets[bucket_wt] = None

                    while bucket_score:
                        while current_max_score > 0 and not score_buckets[current_max_score]:
                            current_max_score -= 1
                        if current_max_score == 0:
                            break
                        bucket_id = score_buckets[current_max_score].pop()

                        active_keys = []
                        root_ass_set: List[int] = []
                        for key in bucket_keys[bucket_id]:
                            if key not in cluster_sets:
                                active_keys.append(key)
                                wt = key_weights.get(key, min_key_wt)
                                if wt == bucket_wt:
                                    remove_key = -1
                                    for i, _key in enumerate(root_ass_set):
                                        if key_to_bucket_ids[key] == key_to_bucket_ids[_key]:
                                            remove_key = _key if key < _key else key
                                            break  # keep the smallest key under identical bucket set
                                    if remove_key == -1:
                                        root_ass_set.append(key)
                                    elif remove_key != key:
                                        root_ass_set.remove(remove_key)
                                        root_ass_set.append(key)

                        assert active_keys
                        assert root_ass_set
                        bucket_keys[bucket_id] = active_keys

                        if len(root_ass_set) > 1:
                            key_ass_size = []
                            for root_cand in root_ass_set:
                                key_ass_set = set(root_ass_set)
                                for bkt_id in key_to_bucket_ids[root_cand]:
                                    if bucket_id == bkt_id or bucket_keys[bkt_id] is None:
                                        continue
                                    for ass_key in bucket_keys[bkt_id]:
                                        if ass_key not in cluster_sets and bucket_wt == key_weights.get(ass_key, min_key_wt):
                                            key_ass_set.add(ass_key)
                                key_ass_size.append((root_cand, len(key_ass_set)))
                            root_key = min(key_ass_size, key=lambda t: (t[1], t[0]))[0]
                        else:
                            root_key = root_ass_set[0]

                        cluster_size.setdefault(root_key, 0)
                        for bkt_id in key_to_bucket_ids[root_key]:
                            for ass_key in bucket_keys[bkt_id]:
                                if ass_key not in cluster_sets:
                                    cluster_sets[ass_key] = root_key
                                    cluster_size[root_key] += 1
                                    if key_weights.get(ass_key, min_key_wt) == bucket_wt:
                                        for ass_bucket_id in key_to_bucket_ids[ass_key]:
                                            old_cnt = bucket_score.get(ass_bucket_id)
                                            if old_cnt is None:
                                                continue
                                            score_buckets[old_cnt].discard(ass_bucket_id)
                                            next_cnt = old_cnt - 1
                                            if next_cnt == 0:
                                                bucket_score.pop(ass_bucket_id, None)
                                                weight_buckets[bucket_wt + 1].append(ass_bucket_id)
                                            else:
                                                bucket_score[ass_bucket_id] = next_cnt
                                                score_buckets[next_cnt].add(ass_bucket_id)
                                    key_weights.pop(ass_key, None)
                            bucket_keys[bkt_id] = None  # release the memory
                            old_cnt = bucket_score.pop(bkt_id, None)
                            if old_cnt is not None:
                                score_buckets[old_cnt].discard(bkt_id)                    

            for rep, aliases in compressed_keys_by_rep.items():
                assert rep in cluster_sets
                root_key = cluster_sets[rep]
                for alias in aliases:
                    assert alias not in cluster_sets
                    cluster_sets[alias] = root_key
                    cluster_size[root_key] = cluster_size.get(root_key, 0) + 1

            if cluster_size:
                cluster_freq = {}
                for c_size in cluster_size.values():
                    cluster_freq[c_size] = cluster_freq.get(c_size, 0) + 1
                logger.info("Number of effective clusters: {}, clustered docs: {},  Compressed docs: {}, Cluster freq stats:", len(cluster_size), len(cluster_sets), alias_count)
                for c_size, freq in sorted(cluster_freq.items()):
                    logger.info("{}: {}", c_size, freq)        

            assert initial_key_count == len(cluster_sets), (
                "Cluster coverage sanity check failed in run: "
                f"initial key_weights={initial_key_count}, final cluster_sets={len(cluster_sets)}"
            )

            # Start at 1 so packed (cluster_id << 32) | size is always > 0xFFFFFFFF.
            # This avoids ambiguity between "size-only" and "packed" values.
            ci = 1
            remove_keys = []
            with self.output_folder.get_output_file_manager(mode="wb") as output_mg:
                for node_key in sorted(cluster_sets):
                    self.stat_update("duplicates")
                    read_id, doc_id = _unpack_dup_key(node_key)
                    root_key = cluster_sets[node_key]
                                      
                    if node_key != root_key :    
                        output_mg.write(f"{read_id:06d}.remove", struct.pack("<I", doc_id))
                        self.stat_update("to_remove")   
                        if self.analyze_clustering:  
                            remove_keys.append((read_id, doc_id))

                    # additional metadata
                    if self.save_cluster_size:
                        packed = cluster_size[root_key]
                        # If not packed yet, pack (cluster_id << 32) | size into cluster_size.
                        if packed <= 0xFFFFFFFF:
                            cluster_id = ci
                            ci += 1
                            self.stat_update("clusters")
                            size = packed
                            packed = (cluster_id << 32) | size
                            cluster_size[root_key] = packed
                        else:
                            cluster_id = packed >> 32
                            size = packed & 0xFFFFFFFF
                        output_mg.write(
                            f"{read_id:06d}.clusters",
                            struct.pack("<3I", doc_id, cluster_id, size),
                        )

            if self.analyze_clustering:               
                analyze_cluster_signature(
                    remove_keys=remove_keys,
                    signatures_folder=self.signature_folder,
                    config=self.config,
                    block_size=32,
                )
                        


class FilterStage(Stage):
    """Stage 4: filter documents using Stage 3 removal decisions.

    Keep each selected representative and documents outside duplicate buckets;
    remove documents assigned to a representative.
    Includes option to store cluster size (dedup count) in kept documents metadata.
    """

    name = "stage 4: filter"

    def __init__(
        self,
        input_folder: DataFolderLike,
        exclusion_writer: JsonlWriter = None,
        load_cluster_sizes: bool = True,  # Set True to get dedup count
        lines_to_buffer: int = 25_000,
    ):
        super().__init__()
        assert load_cluster_sizes, "load_cluster_sizes must be True for FilterStage"
        self.data_folder = get_datafolder(input_folder)
        self.exclusion_writer = exclusion_writer
        self.load_cluster_sizes = load_cluster_sizes
        self.lines_to_buffer = lines_to_buffer

    def run(self, data: DocumentStream, rank: int = 0, world_size: int = 1):
        if not self.data_folder.isfile(f"{rank:06d}.remove"):
            logger.warning(f"No .remove file for {rank=}.")
            # If loading sizes/ids is requested, we should still try to load them even if no .remove file exists
            # (all documents are kept)
            has_cluster_file = self.load_cluster_sizes and self.data_folder.exists(f"{rank:06d}.clusters")

            if not has_cluster_file:
                # No removal info and no metadata files requested/found - pass through directly
                for doc in data:
                    self.stat_update(StatLabel.total, StatLabel.forwarded)
                    yield doc
                return

        # Function to load metadata files
        def metadata_loader(file):
            with self.data_folder.open(file, "rb") as metadata_f:
                for data in read_tuples_from_file(metadata_f, "3I", lines_to_buffer=self.lines_to_buffer):
                    yield data

        # Set up iterators for metadata if requested and files exist
        cluster_loader = iter([])
        if self.data_folder.exists(f"{rank:06d}.clusters"):
            cluster_loader = metadata_loader(f"{rank:06d}.clusters")
        else:
            logger.warning(f"No .clusters file for {rank=}. Cannot load cluster IDs.")

        next_cluster = next(cluster_loader, None)

        # Set up removal logic
        removal_file_exists = self.data_folder.isfile(f"{rank:06d}.remove")
        f_remove = self.data_folder.open(f"{rank:06d}.remove", "rb") if removal_file_exists else None

        def get_next_removal(file_handle):
            if not file_handle:
                return None
            data = file_handle.read(struct.calcsize("I"))
            if data:
                return struct.unpack("<I", data)[0]
            return None

        next_removal = get_next_removal(f_remove)
        with self.exclusion_writer if self.exclusion_writer else contextlib.nullcontext() as exc_writer:
            for idx, doc in enumerate(data):
                with self.track_time():
                    # Check both doc_id and data_rank for removal
                    is_removed = (next_removal == idx)

                    # Load and save metadata even for removed docs if writer exists
                    cluster_id_to_save = -1
                    if next_cluster and next_cluster[0] == idx:
                        cluster_id_to_save = next_cluster[1]
                        dup_count_to_save = next_cluster[2] - 1
                        next_cluster = next(cluster_loader, None)
                    else:
                        dup_count_to_save = None
                    doc.metadata["minhash_cluster_id"] = cluster_id_to_save

                    # Ensure "dup_signals" key exists and is a dict
                    signals_dict = doc.metadata.setdefault("dup_signals", {})

                    # Only update dup_doc_count if we have new information from the size file
                    if dup_count_to_save is not None:
                        signals_dict["dup_doc_count"] = dup_count_to_save
                    # If no size entry for this document, preserve existing dup_doc_count or default to 0
                    elif "dup_doc_count" not in signals_dict:
                        signals_dict["dup_doc_count"] = 0

                    self.stat_update(StatLabel.total)
                    if is_removed:
                        # Document marked for removal
                        self.stat_update(StatLabel.dropped)
                        if self.exclusion_writer:
                            # Add metadata before writing to exclusion
                            exc_writer.write(doc, rank)
                        next_removal = get_next_removal(f_remove)
                        continue
                    else:
                        # Document is kept
                        self.stat_update(StatLabel.forwarded)
                        # Metadata is already added to the doc object above
                        yield doc

        if f_remove:
            f_remove.close()


_SIG_MEMMAP = None
_SIG_NUM_SIGS = None
_SIG_DIFF_THRESHOLD = None


def _init_sig_memmap(path: str, shape: Tuple[int, int], dtype: np.dtype, num_sigs: int, diff_threshold: int):
    global _SIG_MEMMAP, _SIG_NUM_SIGS, _SIG_DIFF_THRESHOLD
    _SIG_MEMMAP = np.memmap(path, mode="r", dtype=dtype, shape=shape)
    _SIG_NUM_SIGS = num_sigs
    _SIG_DIFF_THRESHOLD = diff_threshold


if _numba is not None:
    @_numba.njit(cache=False, fastmath=True, parallel=True)
    def _hist_all_blocks_numba(sig, n_docs, num_sigs, diff_threshold, block_size, range_starts, range_ends):
        max_bin = diff_threshold + 1
        n_blocks = (n_docs + block_size - 1) // block_size
        n_ranges = len(range_starts)
        range_hists = np.zeros((n_ranges, diff_threshold + 2), dtype=np.int64)
        for ri in _numba.prange(n_ranges):
            bi_start = range_starts[ri]
            bi_end = range_ends[ri]
            hist = np.zeros(diff_threshold + 2, dtype=np.int64)
            for bi in range(bi_start, bi_end):
                i0 = bi * block_size
                i1 = min(n_docs, i0 + block_size)
                for bj in range(bi, n_blocks):
                    j0 = bj * block_size
                    j1 = min(n_docs, j0 + block_size)
                    diag = bi == bj
                    for i in range(i0, i1):
                        j_start = i + 1 if diag else j0
                        for j in range(j_start, j1):
                            diff = 0
                            for s in range(num_sigs):
                                if sig[i, s] != sig[j, s]:
                                    diff += 1
                                    if diff > diff_threshold:
                                        diff = max_bin
                                        break
                            hist[diff] += 1
            range_hists[ri, :] = hist
        hist_total = np.zeros(diff_threshold + 2, dtype=np.int64)
        for bi in range(n_ranges):
            for k in range(diff_threshold + 2):
                hist_total[k] += range_hists[bi, k]
        return hist_total


def _build_balanced_block_ranges(n_blocks: int, n_ranges: int) -> Tuple[np.ndarray, np.ndarray]:
    if n_blocks <= 0:
        return np.empty(0, dtype=np.int64), np.empty(0, dtype=np.int64)
    n_ranges = max(1, min(n_ranges, n_blocks))
    starts: List[int] = []
    ends: List[int] = []
    remaining_pairs = n_blocks * (n_blocks + 1) // 2
    start = 0
    for r in range(n_ranges - 1):
        ranges_left = n_ranges - r
        target = max(1, remaining_pairs // ranges_left)
        acc = 0
        bi = start
        max_bi = n_blocks - (ranges_left - 1)
        while bi < max_bi:
            w = n_blocks - bi
            if acc + w > target and bi > start:
                break
            acc += w
            bi += 1
        starts.append(start)
        ends.append(bi)
        remaining_pairs -= acc
        start = bi
    starts.append(start)
    ends.append(n_blocks)
    return np.asarray(starts, dtype=np.int64), np.asarray(ends, dtype=np.int64)


def analyze_cluster_signature(
    remove_keys: List[Tuple[int, int]],
    signatures_folder: DataFolderLike,
    config: MinhashConfig,
    num_workers: int = 128,
    block_size: int = 32,
    lines_to_buffer: int = 50_000,
    strict: bool = True,
    numba_ranges_per_worker: int = 1,
    temp_dir: Optional[str] = None,
    max_docs_exact: int = 10_000_000,
    sample_seed: int = 42,
):
    """
    Analyze signature overlap among unclustered docs.

    remove_keys contains (read_id, doc_id) clustered pairs to be removed from the full list 
    Among unclustered (read_id, doc_id) pairs, collects full minhash
    signatures across all buckets for each document. Then computes a histogram
    of the number of different signatures (per-hash, not per-bucket) for each
    document pair, capping counts above diff_threshold.

    Pairs file format: little-endian uint32, uint32 per record (read_id, doc_id).

    Args:
        remove_keys: (read_id, doc_id) pairs to exclude from analysis.
        signatures_folder: folder containing bucket_###/<read_id>.minhash.sig files.
        config: MinhashConfig used to generate signatures.
        num_workers: number of worker processes for pairwise comparison.
        block_size: block size for pairwise comparison (smaller -> less memory, more overhead).
        lines_to_buffer: number of lines to buffer while reading signature files.
        strict: additionally require requested removals to be present in each loaded file.
            Missing, duplicate, or unexpected retained document IDs always raise.
        diff_threshold: cap differences above this value for early exit.
        numba_ranges_per_worker: number of balanced block ranges assigned per numba thread.
        temp_dir: directory for memmap storage (defaults to a new temp dir).
        max_docs_exact: run exact all-pairs only up to this many docs.
            If n_docs is larger, sample docs and compute an approximate histogram.
        sample_seed: RNG seed for reproducible sampling when max_docs_exact is exceeded.
    """
    global _SIG_MEMMAP, _SIG_NUM_SIGS, _SIG_DIFF_THRESHOLD
    sig_folder = get_datafolder(signatures_folder)

    num_buckets = config.num_buckets
    hashes_per_bucket = config.hashes_per_bucket
    num_sigs = num_buckets * hashes_per_bucket
    diff_threshold = int(num_sigs*0.25)
 
    created_temp_dir = temp_dir is None
    if temp_dir is None:
        temp_dir = tempfile.mkdtemp(prefix="minhash_cluster_")
    os.makedirs(temp_dir, exist_ok=True)
    memmap_path = os.path.join(temp_dir, "cluster_sig_memmap.dat")
    sig_memmap = None
    try:
        if not sig_folder.exists("bucket_000"):
            return

        bucket_000_files = sig_folder.list_files(subdirectory="bucket_000", recursive=False)
        line_format = f"{hashes_per_bucket + 1}{config.hash_config.struct_format}I"
        remove_set = set(remove_keys or [])
        remove_by_read: Dict[int, Set[int]] = {}
        for read_id, doc_id in remove_set:
            remove_by_read.setdefault(read_id, set()).add(doc_id)

        # Preserve band-0 document order in compact arrays; other bands may have different orders.
        kept_doc_ids_by_read: Dict[int, np.ndarray] = {}
        for path in bucket_000_files:
            read_id = int(os.path.basename(path).split(".", 1)[0])
            if read_id in kept_doc_ids_by_read:
                raise ValueError(f"Duplicate signature file for read_id={read_id} in bucket_000")
            kept_doc_ids = {}
            removed_doc_ids = remove_by_read.get(read_id, set())
            removed_hits = set()
            with sig_folder.open(path, "rb") as f:
                for data in read_tuples_from_file(f, line_format, lines_to_buffer=lines_to_buffer):
                    doc_id = data[-1]
                    if doc_id in kept_doc_ids or doc_id in removed_hits:
                        raise ValueError(f"Duplicate doc_id={doc_id} for read_id={read_id} in bucket_000")
                    if doc_id in removed_doc_ids:
                        removed_hits.add(doc_id)
                    else:
                        kept_doc_ids[doc_id] = None
            kept_doc_ids_by_read[read_id] = np.fromiter(kept_doc_ids, dtype=np.uint32, count=len(kept_doc_ids))
            del kept_doc_ids
            if strict and len(removed_hits) != len(removed_doc_ids):
                raise ValueError(f"Unmatched removals for read_id={read_id} in bucket_000")

        if strict:
            missing_read_ids = remove_by_read.keys() - kept_doc_ids_by_read.keys()
            if missing_read_ids:
                raise ValueError(f"Unmatched removals for read_id={min(missing_read_ids)} in bucket_000")

        n_docs = sum(len(doc_ids) for doc_ids in kept_doc_ids_by_read.values())
        if n_docs == 0:
            logger.info("No retained documents; skipping signature analysis")
            return
        total_pairs_full = n_docs * (n_docs - 1) // 2

        read_offsets: Dict[int, int] = {}
        running_offset = 0
        for read_id in sorted(kept_doc_ids_by_read):
            kept_count = len(kept_doc_ids_by_read[read_id])
            if kept_count <= 0:
                continue
            read_offsets[read_id] = running_offset
            running_offset += kept_count

        sig_memmap = np.memmap(memmap_path, mode="w+", dtype=np.uint64, shape=(n_docs, num_sigs))
        expected_read_ids = set(read_offsets)

        # Load signatures bucket by bucket
        for bi in tqdm(range(num_buckets), desc="Loading bucket signatures"):
            base = bi * hashes_per_bucket
            bucket_dir = f"bucket_{bi:03d}"
            bucket_files = sig_folder.list_files(subdirectory=bucket_dir, recursive=False)
            read_paths: Dict[int, str] = {}
            for path in bucket_files:
                read_id = int(os.path.basename(path).split(".", 1)[0])
                if read_id not in kept_doc_ids_by_read:
                    raise ValueError(f"Unexpected signature file for read_id={read_id} in {bucket_dir}")
                if read_id in read_offsets:
                    if read_id in read_paths:
                        raise ValueError(f"Duplicate signature file for read_id={read_id} in {bucket_dir}")
                    read_paths[read_id] = path
            if len(read_paths) != len(expected_read_ids):
                first_missing = None
                for expected_read_id in expected_read_ids:
                    if expected_read_id not in read_paths:
                        first_missing = expected_read_id
                        break
                if first_missing is not None:
                    raise FileNotFoundError(
                        f"Missing signature file: {bucket_dir}/{first_missing:05d}.minhash.sig"
                    )
            for read_id, path in read_paths.items():
                read_offset = read_offsets[read_id]
                removed_doc_ids = remove_by_read.get(read_id, set())
                # Build only one reader's lookup at a time, not a corpus-wide Python dictionary.
                doc_rows = {int(doc_id): row for row, doc_id in enumerate(kept_doc_ids_by_read[read_id])}
                seen_rows = bytearray(len(doc_rows))
                loaded_count = 0
                removed_hits = set()
                with sig_folder.open(path, "rb") as f:
                    for data in read_tuples_from_file(f, line_format, lines_to_buffer=lines_to_buffer):
                        doc_id = data[-1]
                        if doc_id in removed_doc_ids:
                            if doc_id in removed_hits:
                                raise ValueError(f"Duplicate doc_id={doc_id} for read_id={read_id} in {bucket_dir}")
                            removed_hits.add(doc_id)
                            continue
                        row = doc_rows.get(doc_id)
                        if row is None:
                            raise ValueError(f"Unexpected doc_id={doc_id} for read_id={read_id} in {bucket_dir}")
                        if seen_rows[row]:
                            raise ValueError(f"Duplicate doc_id={doc_id} for read_id={read_id} in {bucket_dir}")
                        seen_rows[row] = 1
                        sig_memmap[read_offset + row, base : base + hashes_per_bucket] = data[:-2]
                        loaded_count += 1
                if strict and len(removed_hits) != len(removed_doc_ids):
                    raise ValueError(f"Unmatched removals for read_id={read_id} in {bucket_dir}")
                if loaded_count != len(doc_rows):
                    missing_doc_id = next(doc_id for doc_id, row in doc_rows.items() if not seen_rows[row])
                    raise ValueError(f"Missing doc_id={missing_doc_id} for read_id={read_id} in {bucket_dir}")
                del doc_rows, seen_rows

        del kept_doc_ids_by_read
        sig_memmap.flush()

        sampled = False
        sampled_scale = 1.0
        compare_sig = sig_memmap
        compare_docs = n_docs
        compare_in_memory = False
        if max_docs_exact > 0 and n_docs > max_docs_exact:
            rng = np.random.default_rng(sample_seed)
            sample_idx = np.sort(rng.choice(n_docs, size=max_docs_exact, replace=False))
            compare_sig = np.ascontiguousarray(sig_memmap[sample_idx, :])
            compare_docs = compare_sig.shape[0]
            compare_in_memory = True
            sampled = True
            sampled_pairs = compare_docs * (compare_docs - 1) // 2
            if sampled_pairs > 0:
                sampled_scale = total_pairs_full / sampled_pairs
            logger.warning(
                "analyze_clustering: n_docs={} exceeds max_docs_exact={}. "
                "Using sampled analysis on {} docs (seed={}), scale factor {:.3f}",
                n_docs,
                max_docs_exact,
                compare_docs,
                sample_seed,
                sampled_scale,
            )
        else:
            # Numba runs substantially faster on a plain contiguous ndarray than on a memmap.
            compare_sig = np.ascontiguousarray(sig_memmap)
            compare_in_memory = True

        # Pairwise histogram
        max_workers = min(num_workers, os.cpu_count() or num_workers)
        _SIG_MEMMAP = compare_sig
        _SIG_NUM_SIGS = num_sigs
        _SIG_DIFF_THRESHOLD = diff_threshold
        if _numba is None:
            err_lines = [
                "analyze_clustering requires numba, but numba is unavailable.",
                "Install numba in the runtime environment and rerun.",
            ]
            if _numba_import_error is not None:
                err_lines.append(f"Captured numba import error: {_numba_import_error!r}")
                if _numba_import_traceback:
                    err_lines.append("Captured numba import traceback:")
                    err_lines.append(_numba_import_traceback.rstrip())
            raise RuntimeError("\n".join(err_lines))
        logger.info("analyze_clustering compare backend: numba")
        max_numba_threads = getattr(_numba.config, "NUMBA_NUM_THREADS", max_workers)
        worker_threads = max(1, min(max_workers, max_numba_threads))
        _numba.set_num_threads(worker_threads)
        n_blocks = (compare_docs + block_size - 1) // block_size
        n_ranges = min(n_blocks, max(1, worker_threads * max(1, numba_ranges_per_worker)))
        range_starts, range_ends = _build_balanced_block_ranges(n_blocks, n_ranges)
        hist_total = _hist_all_blocks_numba(
            _SIG_MEMMAP, compare_docs, num_sigs, diff_threshold, block_size, range_starts, range_ends
        )
        try:
            threading_layer = _numba.threading_layer()
        except Exception:
            threading_layer = "unknown"
        logger.info(
            "analyze_clustering compare setup: docs={}, sigs={}, dtype={}, block_size={}, "
            "threads={}, threading_layer={}, in_memory={}",
            compare_docs,
            num_sigs,
            str(compare_sig.dtype),
            block_size,
            worker_threads,
            threading_layer,
            compare_in_memory,
        )

        hist_report = hist_total
        if sampled and sampled_scale > 1.0:
            hist_report = np.rint(hist_total.astype(np.float64) * sampled_scale).astype(np.int64)
            logger.info(
                "Reported counts are scaled estimates from sampled analysis: "
                "sample_docs={}, total_docs={}",
                compare_docs,
                n_docs,
            )

        logger.info("Minhash diff distribution among {} docs (>{} capped)", n_docs, diff_threshold)
        for i, count in enumerate(hist_report):
            if i >= diff_threshold + 1:
                logger.info(" >{}: {}", i, count)
            else:
                logger.info(" {}: {}", i, count)
    finally:
        _SIG_MEMMAP = None
        _SIG_NUM_SIGS = None
        _SIG_DIFF_THRESHOLD = None
        if sig_memmap is not None:
            del sig_memmap
        if created_temp_dir:
            shutil.rmtree(temp_dir, ignore_errors=True)
