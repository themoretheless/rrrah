from abc import abstractmethod
from collections.abc import Collection
from typing import IO, Protocol

class MediaError(Exception):
    """Aggregate error for different media types"""


class MediaFingerprint(Protocol):
    @abstractmethod
    def __sub__(self, other: "MediaFingerprint") -> int | float: ...

    @abstractmethod
    def __bytes__(self) -> bytes: ...


class MediaImpl(Protocol):
    extensions: Collection[str]

    @abstractmethod
    def analyze(self, media: IO) -> object: ...

    @abstractmethod
    def checksum(self, media: IO) -> bytes: ...

    @abstractmethod
    def fingerprint(self, media: IO) -> MediaFingerprint: ...

    @abstractmethod
    def parse_fingerprint(self, __bytes: bytes) -> MediaFingerprint: ...
