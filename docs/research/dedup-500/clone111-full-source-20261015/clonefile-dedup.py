#!/usr/bin/env python3
"""
APFS Clonefile Deduplicator

Finds duplicate files and replaces them with APFS clones (copy-on-write),
saving disk space while maintaining separate file identities.

Requirements: pip install xxhash xattr tqdm
"""

import os
import sys
import ctypes
import ctypes.util
import argparse
from pathlib import Path
from collections import defaultdict
from dataclasses import dataclass, field
from typing import Optional
from concurrent.futures import ProcessPoolExecutor, as_completed
from tqdm import tqdm

# Use xxhash for speed — it's ~10x faster than SHA256 and collision-resistant
# enough for deduplication (not crypto, but we don't need crypto here)
try:
	import xxhash
	def make_hasher():
		return xxhash.xxh3_128()
except ImportError:
	import hashlib
	def make_hasher():
		return hashlib.sha256()
	print("Warning: xxhash not installed, falling back to SHA256 (slower)", file=sys.stderr)

try:
	import xattr
	HAS_XATTR = True
except ImportError:
	HAS_XATTR = False
	print("Warning: xattr not installed, extended attributes won't be preserved", file=sys.stderr)


BLOCK_SIZE = 65536
MIN_FILE_SIZE = 1024


# ============================================================================
# clonefile syscall binding
# ============================================================================

class ClonefileError(OSError):
	"""Raised when clonefile syscall fails."""
	pass


def _setup_clonefile():
	"""Set up the clonefile syscall binding once at module load."""
	libc_path = ctypes.util.find_library('c')
	if not libc_path:
		return None
	
	libc = ctypes.CDLL(libc_path, use_errno=True)
	
	try:
		_cf = libc.clonefile
		_cf.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_uint32]
		_cf.restype = ctypes.c_int
		return _cf
	except AttributeError:
		return None


_clonefile_func = _setup_clonefile()


def clonefile(src: Path, dst: Path) -> None:
	"""
	Use macOS clonefile syscall for copy-on-write file duplication.
	
	The destination must not exist. Creates a new file that shares storage
	with the source until either is modified.
	"""
	if _clonefile_func is None:
		raise ClonefileError("clonefile syscall not available (macOS/APFS only)")
	
	result = _clonefile_func(str(src).encode(), str(dst).encode(), 0)
	if result != 0:
		errno = ctypes.get_errno()
		raise ClonefileError(errno, os.strerror(errno), str(src))


# ============================================================================
# File hashing
# ============================================================================

def hash_file(path: Path, partial: bool = False) -> Optional[str]:
	"""
	Hash a file's contents.
	
	If partial=True, only hashes the first BLOCK_SIZE bytes (fast prefilter).
	Returns None on error.
	"""
	try:
		hasher = make_hasher()
		with open(path, 'rb') as f:
			while chunk := f.read(BLOCK_SIZE):
				hasher.update(chunk)
				if partial:
					break
		return hasher.hexdigest()
	except (OSError, PermissionError):
		return None


def _hash_file_partial(path_str: str) -> tuple[str, Optional[str]]:
	"""Worker function for parallel partial hashing."""
	return (path_str, hash_file(Path(path_str), partial=True))


def _hash_file_full(path_str: str) -> tuple[str, Optional[str]]:
	"""Worker function for parallel full hashing."""
	return (path_str, hash_file(Path(path_str), partial=False))


# ============================================================================
# Core deduplicator
# ============================================================================

@dataclass
class FileInfo:
	"""Tracks a file through the deduplication pipeline."""
	path: Path
	size: int
	partial_hash: Optional[str] = None
	full_hash: Optional[str] = None


@dataclass
class DeduplicationResult:
	"""Results from a deduplication run."""
	files_scanned: int = 0
	duplicates_found: int = 0
	files_replaced: int = 0
	bytes_saved: int = 0
	errors: list[str] = field(default_factory=list)


class Deduplicator:
	"""
	Finds and deduplicates files using APFS clonefile.
	
	Strategy:
	1. Scan directory tree for regular files
	2. Group by size (free, instant)
	3. For size collisions, compute partial hash (first 64k)
	4. For partial hash collisions, compute full hash
	5. Replace duplicates with clones of the original
	"""
	
	def __init__(
		self,
		root: Path,
		min_size: int = MIN_FILE_SIZE,
		workers: int = None,
		verbose: bool = True
	):
		self.root = Path(root).resolve()
		self.min_size = min_size
		self.workers = workers or os.cpu_count() or 4
		self.verbose = verbose
		self.files: list[FileInfo] = []
	
	def log(self, msg: str) -> None:
		if self.verbose:
			print(msg)
	
	def scan(self) -> int:
		"""
		Scan directory tree for regular files.
		Returns number of files found.
		"""
		self.files = []
		self.log(f"📂 Scanning {self.root}...")
		
		for entry in tqdm(self.root.rglob("*"), desc="Scanning", disable=not self.verbose):
			try:
				if entry.is_file() and not entry.is_symlink():
					size = entry.stat().st_size
					if size >= self.min_size:
						self.files.append(FileInfo(path=entry, size=size))
			except (PermissionError, OSError):
				continue
		
		self.log(f"   Found {len(self.files):,} files (>= {self.min_size} bytes)")
		return len(self.files)
	
	def find_duplicates(self) -> dict[str, list[FileInfo]]:
		"""
		Find duplicate files using progressive hashing.
		Returns dict mapping full_hash -> list of FileInfo with that hash.
		"""
		# Phase 1: Group by size
		by_size: dict[int, list[FileInfo]] = defaultdict(list)
		for f in self.files:
			by_size[f.size].append(f)
		
		candidates = [f for files in by_size.values() if len(files) > 1 for f in files]
		self.log(f"🔍 {len(candidates):,} files have size duplicates")
		
		if not candidates:
			return {}
		
		# Phase 2: Partial hash (first 64k)
		self.log(f"⚡ Computing partial hashes ({self.workers} workers)...")
		path_to_file = {str(f.path): f for f in candidates}
		
		with ProcessPoolExecutor(max_workers=self.workers) as pool:
			futures = [pool.submit(_hash_file_partial, str(f.path)) for f in candidates]
			for future in tqdm(as_completed(futures), total=len(futures), desc="Partial hash", disable=not self.verbose):
				path_str, hash_val = future.result()
				if hash_val and path_str in path_to_file:
					path_to_file[path_str].partial_hash = hash_val
		
		# Group by partial hash
		by_partial: dict[str, list[FileInfo]] = defaultdict(list)
		for f in candidates:
			if f.partial_hash:
				by_partial[f.partial_hash].append(f)
		
		needs_full = [f for files in by_partial.values() if len(files) > 1 for f in files]
		self.log(f"🔬 {len(needs_full):,} files need full hash verification")
		
		if not needs_full:
			return {}
		
		# Phase 3: Full hash
		self.log(f"🔐 Computing full hashes...")
		path_to_file = {str(f.path): f for f in needs_full}
		
		with ProcessPoolExecutor(max_workers=self.workers) as pool:
			futures = [pool.submit(_hash_file_full, str(f.path)) for f in needs_full]
			for future in tqdm(as_completed(futures), total=len(futures), desc="Full hash", disable=not self.verbose):
				path_str, hash_val = future.result()
				if hash_val and path_str in path_to_file:
					path_to_file[path_str].full_hash = hash_val
		
		# Final grouping
		by_full: dict[str, list[FileInfo]] = defaultdict(list)
		for f in needs_full:
			if f.full_hash:
				by_full[f.full_hash].append(f)
		
		return {h: files for h, files in by_full.items() if len(files) > 1}
	
	def deduplicate(
		self,
		duplicates: dict[str, list[FileInfo]],
		dry_run: bool = True,
		verify: bool = True,
		cautious: bool = False
	) -> DeduplicationResult:
		"""
		Replace duplicate files with clones.
		
		Args:
			duplicates: Output from find_duplicates()
			dry_run: If True, only report what would be done
			verify: If True, verify file hash after replacement
		
		Returns:
			DeduplicationResult with statistics
		"""
		result = DeduplicationResult(files_scanned=len(self.files))
		
		skip_all_prompts = False
		
		for hash_val, files in duplicates.items():
			# Sort by path to get deterministic "original" selection
			# Prefer shorter paths (likely closer to root, less likely to be a copy)
			files.sort(key=lambda f: (len(f.path.parts), str(f.path)))
			original = files[0]
			result.duplicates_found += len(files) - 1
			
			self.log(f"\n📄 Original: {original.path}")
			self.log(f"   Size: {original.size:,} bytes | Hash: {hash_val[:16]}...")
			
			for dup in files[1:]:
				if dry_run:
					self.log(f"   Would replace: {dup.path}")
					result.bytes_saved += dup.size
				else:
					# Cautious mode: prompt before each operation
					if cautious and not skip_all_prompts:
						print(f"\n{'─' * 60}")
						print(f"📋 Proposed clone operation:")
						print(f"   Source (original):  {original.path}")
						print(f"   Target (duplicate): {dup.path}")
						print(f"   Size: {dup.size:,} bytes")
						print(f"{'─' * 60}")
						
						response = self._prompt_user(
							"Proceed? [y]es / [n]o / [a]ll remaining / [q]uit: ",
							valid={"y", "n", "a", "q", "yes", "no", "all", "quit"}
						)
						
						if response in ("q", "quit"):
							self.log("🛑 Aborted by user")
							return result
						elif response in ("n", "no"):
							self.log(f"   ⏭️  Skipped: {dup.path}")
							continue
						elif response in ("a", "all"):
							skip_all_prompts = True
					
					try:
						self._replace_with_clone(original.path, dup.path)
						
						if verify:
							new_hash = hash_file(dup.path, partial=False)
							if new_hash != hash_val:
								raise ValueError(f"Verification failed: hash mismatch after clone")
						
						self.log(f"   ✓ Replaced: {dup.path}")
						result.files_replaced += 1
						result.bytes_saved += dup.size
						
						# Cautious mode: offer to open Finder for audit
						if cautious and not skip_all_prompts:
							response = self._prompt_user(
								"Clone successful! [enter] continue / [f] reveal in Finder / [q]uit: ",
								valid={"", "f", "q", "quit"}
							)
							if response == "f":
								self._reveal_in_finder(dup.path)
								self._prompt_user("Press [enter] to continue...")
							elif response in ("q", "quit"):
								self.log("🛑 Stopped by user")
								return result
						
					except Exception as e:
						error_msg = f"Failed to replace {dup.path}: {e}"
						self.log(f"   ✗ {error_msg}")
						result.errors.append(error_msg)
						
						if cautious:
							response = self._prompt_user(
								"Error occurred. [enter] continue / [q]uit: ",
								valid={"", "q", "quit"}
							)
							if response in ("q", "quit"):
								return result
		
		return result
	
	def _prompt_user(self, prompt: str, valid: set[str] = None) -> str:
		"""Prompt user for input, optionally validating against allowed values."""
		while True:
			try:
				response = input(prompt).strip().lower()
				if valid is None or response in valid:
					return response
				print(f"   Invalid input. Please enter one of: {', '.join(sorted(valid))}")
			except (EOFError, KeyboardInterrupt):
				print()
				return "q"
	
	def _reveal_in_finder(self, path: Path) -> None:
		"""Open Finder with the specified file selected."""
		import subprocess
		subprocess.run(["open", "-R", str(path)], check=False)
	
	def _replace_with_clone(self, src: Path, dst: Path) -> None:
		"""Replace dst with a clone of src, preserving all metadata."""
		# Capture original metadata
		old_stat = dst.stat()
		old_xattrs = {}
		if HAS_XATTR:
			try:
				old_xattrs = dict(xattr.xattr(str(dst)))
			except Exception:
				pass
		
		# Also preserve parent directory timestamps
		parent = dst.parent
		parent_stat = parent.stat()
		
		# Clone to temp file
		tmp = dst.with_name(dst.name + '.clonetmp')
		try:
			if tmp.exists():
				tmp.unlink()
			
			clonefile(src, tmp)
			
			# Restore metadata on temp file
			os.chown(tmp, old_stat.st_uid, old_stat.st_gid)
			os.chmod(tmp, old_stat.st_mode)
			os.utime(tmp, (old_stat.st_atime, old_stat.st_mtime))
			
			if HAS_XATTR and old_xattrs:
				for k, v in old_xattrs.items():
					try:
						xattr.setxattr(str(tmp), k, v)
					except Exception:
						pass
			
			# Atomic replace
			tmp.replace(dst)
			
			# Restore parent directory timestamps
			os.utime(parent, (parent_stat.st_atime, parent_stat.st_mtime))
			
		except Exception:
			if tmp.exists():
				tmp.unlink()
			raise
	
	def run(self, dry_run: bool = True, verify: bool = True, cautious: bool = False) -> DeduplicationResult:
		"""
		Run the full deduplication pipeline.
		
		This is the main entry point — combines scan, find, and deduplicate.
		"""
		self.scan()
		duplicates = self.find_duplicates()
		
		total_dupes = sum(len(files) - 1 for files in duplicates.values())
		self.log(f"\n📊 Found {len(duplicates)} duplicate groups ({total_dupes:,} duplicate files)")
		
		if not duplicates:
			self.log("Nothing to deduplicate!")
			return DeduplicationResult(files_scanned=len(self.files))
		
		return self.deduplicate(duplicates, dry_run=dry_run, verify=verify, cautious=cautious)


# ============================================================================
# CLI
# ============================================================================

def main():
	parser = argparse.ArgumentParser(
		description="Deduplicate files using APFS clonefile (copy-on-write)",
		formatter_class=argparse.RawDescriptionHelpFormatter,
		epilog="""
Examples:
  %(prog)s ~/Documents                    # Dry run on Documents
  %(prog)s ~/Documents --execute          # Actually deduplicate
  %(prog)s ~/Documents --execute --cautious  # Interactive mode with auditing
  %(prog)s . --min-size 1048576           # Only files >= 1MB
  %(prog)s /Volumes/Data -w 8 --execute   # Use 8 workers
		"""
	)
	
	parser.add_argument(
		"path",
		nargs="?",
		default=".",
		type=Path,
		help="Directory to deduplicate (default: current directory)"
	)
	parser.add_argument(
		"-w", "--workers",
		type=int,
		default=os.cpu_count(),
		help=f"Number of parallel workers (default: {os.cpu_count()})"
	)
	parser.add_argument(
		"--min-size",
		type=int,
		default=MIN_FILE_SIZE,
		help=f"Minimum file size in bytes (default: {MIN_FILE_SIZE})"
	)
	parser.add_argument(
		"--execute",
		action="store_true",
		help="Actually perform deduplication (default is dry-run)"
	)
	parser.add_argument(
		"--cautious",
		action="store_true",
		help="Interactive mode: confirm each operation and optionally audit in Finder"
	)
	parser.add_argument(
		"--no-verify",
		action="store_true",
		help="Skip hash verification after cloning (faster but riskier)"
	)
	parser.add_argument(
		"-q", "--quiet",
		action="store_true",
		help="Minimal output"
	)
	
	args = parser.parse_args()
	
	if not args.path.exists():
		print(f"Error: {args.path} does not exist", file=sys.stderr)
		sys.exit(1)
	
	if not args.path.is_dir():
		print(f"Error: {args.path} is not a directory", file=sys.stderr)
		sys.exit(1)
	
	# Check we're on APFS
	if _clonefile_func is None:
		print("Error: clonefile not available. This tool requires macOS with APFS.", file=sys.stderr)
		sys.exit(1)
	
	dry_run = not args.execute
	if dry_run:
		print("🔍 DRY RUN MODE — no files will be modified")
		print("   Run with --execute to actually deduplicate\n")
	elif args.cautious:
		print("🐢 CAUTIOUS MODE — you'll confirm each operation")
		print("   Press 'a' at any prompt to approve all remaining\n")
	else:
		print("⚠️  EXECUTE MODE — files will be modified!\n")
	
	dedup = Deduplicator(
		root=args.path,
		min_size=args.min_size,
		workers=args.workers,
		verbose=not args.quiet
	)
	
	result = dedup.run(dry_run=dry_run, verify=not args.no_verify, cautious=args.cautious)
	
	# Summary
	print("\n" + "=" * 60)
	print("📈 Summary")
	print("=" * 60)
	print(f"   Files scanned:      {result.files_scanned:,}")
	print(f"   Duplicate files:    {result.duplicates_found:,}")
	
	if dry_run:
		print(f"   Potential savings:  {result.bytes_saved:,} bytes ({result.bytes_saved / 1024 / 1024:.1f} MB)")
	else:
		print(f"   Files replaced:     {result.files_replaced:,}")
		print(f"   Space saved:        {result.bytes_saved:,} bytes ({result.bytes_saved / 1024 / 1024:.1f} MB)")
	
	if result.errors:
		print(f"   Errors:             {len(result.errors)}")
	
	return 0 if not result.errors else 1


if __name__ == "__main__":
	sys.exit(main())