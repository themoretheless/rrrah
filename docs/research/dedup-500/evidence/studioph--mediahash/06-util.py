from .video import Video
from .types import MediaFingerprint
from collections.abc import Collection, Iterable, Mapping
from collections import defaultdict
import itertools
from .image import Image
import networkx as nx
from typing import TypeVar

T = TypeVar("T", Image, Video)


def similar(
    fp1: MediaFingerprint, fp2: MediaFingerprint, threshold: int | float
) -> bool:
    return (fp1 - fp2) <= threshold


def dedup(items: Iterable[T], threshold: int | float) -> Iterable[Collection[T]]:
    g = nx.Graph()

    for item1, item2 in itertools.combinations(items, r=2):
        similarity = item1.fingerprint - item2.fingerprint
        if similarity <= threshold:
            g.add_edge(item1, item2, weight=similarity)

    return nx.connected_components(g)


def group(items: Iterable[T]) -> Mapping[bytes, Collection[T]]:
    groups = defaultdict(list)

    for item in items:
        groups[item.checksum].append(item)

    return groups
