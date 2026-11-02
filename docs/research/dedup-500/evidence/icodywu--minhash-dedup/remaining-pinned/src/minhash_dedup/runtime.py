"""Document records and per-stage measurements for the MinHash workflow."""

from collections import defaultdict
from collections.abc import Iterable
from contextlib import contextmanager
from dataclasses import dataclass, field
from time import perf_counter
from typing import Any

from loguru import logger as logger


@dataclass
class Document:
    text: str
    id: str
    media: list[Any] = field(default_factory=list)
    metadata: dict[str, Any] = field(default_factory=dict)


DocumentStream = Iterable[Document] | None


class StatLabel:
    total = "total"
    dropped = "dropped"
    forwarded = "forwarded"


@dataclass
class Metric:
    total: float = 0
    n: int = 0
    min: float = float("inf")
    max: float = float("-inf")
    unit: str = "doc"

    def update(self, value, unit=None):
        self.total += value
        self.n += 1
        self.min = min(self.min, value)
        self.max = max(self.max, value)
        if unit is not None:
            self.unit = unit

    @property
    def mean(self):
        return self.total / self.n if self.n else 0


class StageStats:
    def __init__(self, name):
        self.name = name
        self.stats = defaultdict(Metric)
        self.time_stats = Metric(unit="stage")

    def __getitem__(self, label):
        return self.stats[label]

    def to_dict(self):
        def describe(metric):
            return {
                "total": metric.total,
                "n": metric.n,
                "mean": metric.mean,
                "min": metric.min if metric.n else None,
                "max": metric.max if metric.n else None,
                "unit": metric.unit,
            }

        return {
            "name": self.name,
            "time_stats": describe(self.time_stats),
            "stats": {name: describe(metric) for name, metric in self.stats.items()},
        }


class Stage:
    """Small execution contract shared by the five stages and JSONL I/O."""

    name = "stage"

    def __init__(self):
        self.stats = StageStats(str(self))

    def stat_update(self, *labels, value=1, unit=None):
        for label in labels:
            self.stats[label].update(value, unit)

    def update_doc_stats(self, document):
        self.stat_update("doc_len", value=len(document.text), unit="doc")
        if count := document.metadata.get("token_count"):
            self.stat_update("doc_len_tokens", value=count, unit="doc")

    @contextmanager
    def track_time(self, unit=None):
        start = perf_counter()
        try:
            yield
        finally:
            self.stats.time_stats.update(perf_counter() - start, unit)

    def __repr__(self):
        return self.name

    def __call__(self, data=None, rank=0, world_size=1):
        return self.run(data, rank, world_size)

    def run(self, data=None, rank=0, world_size=1):
        raise NotImplementedError
