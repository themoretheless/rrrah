# -*- coding: utf-8 -*-
"""Production-grade hooks system for dmca-takedown-assistant.

This module provides lifecycle hooks for:
- Pre-execution validation and enrichment
- Post-execution verification and cleanup
- Failure handling and recovery
- Degradation monitoring and adaptive behavior
- Performance tracking and optimization

All hooks implement the SkillHook interface from skill_registry.py
"""
from __future__ import annotations

import json
import logging
import time
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple
from dataclasses import dataclass, field

from skill_registry import (
    SkillDescriptor,
    SkillHook,
    SkillState,
    ExecutionContext,
    ExecutionPriority,
)


@dataclass
class HookConfig:
    """Configuration for hook behavior."""
    enable_validation: bool = True
    enable_metrics: bool = True
    enable_logging: bool = True
    enable_cache: bool = True
    cache_ttl_seconds: int = 3600
    max_retries: int = 3
    backoff_base: float = 2.0
    log_level: str = "INFO"
    log_format: str = "%(asctime)s - %(name)s - %(levelname)s - %(message)s"


class LoggingHook(SkillHook):
    """Comprehensive logging hook for all skill lifecycle events."""

    def __init__(self, config: HookConfig) -> None:
        self.config = config
        self.logger = self._setup_logger()

    def _setup_logger(self) -> logging.Logger:
        """Configure structured logger."""
        logger = logging.getLogger("dmca.skills")
        logger.setLevel(getattr(logging, self.config.log_level))

        handler = logging.StreamHandler()
        formatter = logging.Formatter(self.config.log_format)
        handler.setFormatter(formatter)
        logger.addHandler(handler)

        return logger

    def get_name(self) -> str:
        return "logging_hook"

    def pre_validate(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
    ) -> Tuple[bool, Optional[str]]:
        self.logger.info(
            f"[{descriptor.skill_id}] Pre-validation: inputs={list(inputs.keys())}"
        )
        return True, None

    def post_validate(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
    ) -> Tuple[bool, Optional[str]]:
        self.logger.debug(f"[{descriptor.skill_id}] Validation passed")
        return True, None

    def pre_execute(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
    ) -> Tuple[bool, Optional[str]]:
        self.logger.info(
            f"[{descriptor.skill_id}] Executing with priority={descriptor.priority.name}"
        )
        return True, None

    def post_execute(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
        outputs: Dict[str, Any],
        duration_ms: float,
    ) -> Tuple[bool, Optional[str]]:
        self.logger.info(
            f"[{descriptor.skill_id}] Completed in {duration_ms:.0f}ms, "
            f"outputs={list(outputs.keys())}"
        )
        return True, None

    def on_failure(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
        error: Exception,
    ) -> None:
        self.logger.error(
            f"[{descriptor.skill_id}] Failed: {type(error).__name__}: {error}"
        )

    def on_degradation(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
        degradation_level: int,
    ) -> None:
        self.logger.warning(
            f"[{descriptor.skill_id}] Degraded mode level={degradation_level}"
        )

    def on_retry(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
        attempt: int,
        max_attempts: int,
    ) -> None:
        self.logger.info(
            f"[{descriptor.skill_id}] Retry {attempt}/{max_attempts}"
        )


class EvidenceChainHook(SkillHook):
    """Maintains tamper-evident chain of custody for all evidence."""

    def __init__(self, evidence_dir: Path) -> None:
        self.evidence_dir = evidence_dir
        self.evidence_dir.mkdir(parents=True, exist_ok=True)
        self._current_case_id: Optional[str] = None

    def get_name(self) -> str:
        return "evidence_chain_hook"

    def set_case(self, case_id: str) -> None:
        """Set current case ID for evidence tracking."""
        self._current_case_id = case_id

    def pre_execute(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
    ) -> Tuple[bool, Optional[str]]:
        """Capture inputs as evidence before execution."""
        if self._current_case_id:
            self._capture_evidence(
                f"pre_execute_{descriptor.skill_id}",
                {"inputs": inputs, "skill": descriptor.skill_id},
            )
        return True, None

    def post_execute(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
        outputs: Dict[str, Any],
        duration_ms: float,
    ) -> Tuple[bool, Optional[str]]:
        """Capture outputs as evidence after execution."""
        if self._current_case_id:
            self._capture_evidence(
                f"post_execute_{descriptor.skill_id}",
                {
                    "outputs": outputs,
                    "skill": descriptor.skill_id,
                    "duration_ms": duration_ms,
                    "timestamp": datetime.now().isoformat(),
                },
            )
        return True, None

    def _capture_evidence(self, event_type: str, data: Dict[str, Any]) -> None:
        """Store evidence with HMAC-SHA256 sealing."""
        import hmac
        import hashlib

        case_dir = self.evidence_dir / self._current_case_id
        case_dir.mkdir(parents=True, exist_ok=True)

        # Serialize data
        content = json.dumps(data, sort_keys=True, default=str)
        timestamp = datetime.now().isoformat()

        # Create evidence record
        record = {
            "case_id": self._current_case_id,
            "event_type": event_type,
            "timestamp": timestamp,
            "content": content,
        }

        # Compute HMAC for tamper evidence
        # In production, use a proper key from secure storage
        key = b"dmca-takedown-assistant-evidence-key"  # Placeholder
        signature = hmac.new(key, content.encode(), hashlib.sha256).hexdigest()

        record["signature"] = signature
        record["previous_signature"] = self._get_last_signature(case_dir)

        # Write evidence file
        filename = f"{event_type}_{timestamp.replace(':', '-')}.json"
        filepath = case_dir / filename

        with open(filepath, "w", encoding="utf-8") as f:
            json.dump(record, f, indent=2)

    def _get_last_signature(self, case_dir: Path) -> Optional[str]:
        """Get signature from most recent evidence file for chain continuity."""
        files = sorted(case_dir.glob("*.json"), reverse=True)
        if files:
            try:
                with open(files[0], "r", encoding="utf-8") as f:
                    data = json.load(f)
                    return data.get("signature")
            except Exception:
                pass
        return None


class DegradationMonitorHook(SkillHook):
    """Monitors execution quality and manages graceful degradation."""

    def __init__(self, config: HookConfig) -> None:
        self.config = config
        self._skill_failures: Dict[str, List[datetime]] = {}
        self._degradation_levels: Dict[str, int] = {}

    def get_name(self) -> str:
        return "degradation_monitor_hook"

    def on_failure(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
        error: Exception,
    ) -> None:
        """Track failures and detect degradation patterns."""
        now = datetime.now()
        skill_id = descriptor.skill_id

        if skill_id not in self._skill_failures:
            self._skill_failures[skill_id] = []

        self._skill_failures[skill_id].append(now)

        # Clean old failures (older than 1 hour)
        cutoff = now.timestamp() - 3600
        self._skill_failures[skill_id] = [
            f for f in self._skill_failures[skill_id]
            if f.timestamp() > cutoff
        ]

        # Update degradation level
        recent_failures = len(self._skill_failures[skill_id])
        if recent_failures >= 5:
            self._degradation_levels[skill_id] = 4  # Critical
        elif recent_failures >= 3:
            self._degradation_levels[skill_id] = 3  # High
        elif recent_failures >= 2:
            self._degradation_levels[skill_id] = 2  # Medium
        elif recent_failures >= 1:
            self._degradation_levels[skill_id] = 1  # Low
        else:
            self._degradation_levels[skill_id] = 0  # Normal

    def get_degradation_level(self, skill_id: str) -> int:
        """Get current degradation level for a skill."""
        return self._degradation_levels.get(skill_id, 0)

    def should_allow_execution(
        self,
        descriptor: SkillDescriptor,
    ) -> bool:
        """Determine if skill should execute based on degradation level."""
        degradation = self.get_degradation_level(descriptor.skill_id)

        # Critical priority skills always allowed
        if descriptor.priority == ExecutionPriority.CRITICAL:
            return True

        # Block if degradation is severe and skill is not critical
        if degradation >= 4:
            return False

        # Allow with warning for high degradation
        if degradation >= 3 and descriptor.priority == ExecutionPriority.LOW:
            return False

        return True


class CacheHook(SkillHook):
    """Intelligent caching hook for expensive operations."""

    def __init__(self, config: HookConfig) -> None:
        self.config = config
        self._cache: Dict[str, Tuple[Any, float]] = {}

    def get_name(self) -> str:
        return "cache_hook"

    def _get_cache_key(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
    ) -> str:
        """Generate cache key from skill and inputs."""
        import hashlib
        key_data = {
            "skill": descriptor.skill_id,
            "inputs": json.dumps(inputs, sort_keys=True, default=str),
        }
        return hashlib.sha256(json.dumps(key_data).encode()).hexdigest()

    def pre_execute(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
    ) -> Tuple[bool, Optional[str]]:
        """Check cache before execution."""
        if not self.config.enable_cache:
            return True, None

        cache_key = self._get_cache_key(descriptor, inputs)

        if cache_key in self._cache:
            result, timestamp = self._cache[cache_key]
            age = time.time() - timestamp

            if age < self.config.cache_ttl_seconds:
                # Cache hit - set special flag in context
                descriptor.metadata["_cached"] = True
                descriptor.metadata["_cache_age"] = age
                return False, "cache_hit"  # Signal to skip execution

        return True, None

    def post_execute(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
        outputs: Dict[str, Any],
        duration_ms: float,
    ) -> Tuple[bool, Optional[str]]:
        """Store results in cache after successful execution."""
        if not self.config.enable_cache:
            return True, None

        # Only cache successful executions
        if descriptor.metadata.get("_cached"):
            return True, None

        cache_key = self._get_cache_key(descriptor, inputs)
        self._cache[cache_key] = (outputs, time.time())

        return True, None

    def clear_cache(self) -> None:
        """Clear all cached results."""
        self._cache.clear()

    def get_cache_stats(self) -> Dict[str, Any]:
        """Get cache statistics."""
        return {
            "size": len(self._cache),
            "ttl_seconds": self.config.cache_ttl_seconds,
        }


class CircuitBreakerHook(SkillHook):
    """Circuit breaker pattern for failing skills."""

    def __init__(self, failure_threshold: int = 5, timeout_seconds: int = 60) -> None:
        self.failure_threshold = failure_threshold
        self.timeout_seconds = timeout_seconds
        self._failures: Dict[str, List[datetime]] = {}
        self._open_until: Dict[str, datetime] = {}

    def get_name(self) -> str:
        return "circuit_breaker_hook"

    def pre_execute(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
    ) -> Tuple[bool, Optional[str]]:
        """Check if circuit is open for this skill."""
        skill_id = descriptor.skill_id

        # Check if circuit should reset
        if skill_id in self._open_until:
            if datetime.now() > self._open_until[skill_id]:
                # Reset circuit after timeout
                self._failures[skill_id] = []
                del self._open_until[skill_id]
            else:
                # Circuit is still open
                return False, f"Circuit open for {skill_id} until {self._open_until[skill_id]}"

        return True, None

    def on_failure(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
        error: Exception,
    ) -> None:
        """Track failures and open circuit if threshold reached."""
        skill_id = descriptor.skill_id

        if skill_id not in self._failures:
            self._failures[skill_id] = []

        now = datetime.now()
        self._failures[skill_id].append(now)

        # Clean old failures
        cutoff = now.timestamp() - self.timeout_seconds
        self._failures[skill_id] = [
            f for f in self._failures[skill_id]
            if f.timestamp() > cutoff
        ]

        # Open circuit if threshold reached
        if len(self._failures[skill_id]) >= self.failure_threshold:
            self._open_until[skill_id] = datetime.fromtimestamp(
                now.timestamp() + self.timeout_seconds
            )

    def is_open(self, skill_id: str) -> bool:
        """Check if circuit is open for a skill."""
        if skill_id in self._open_until:
            if datetime.now() > self._open_until[skill_id]:
                # Auto-reset
                del self._open_until[skill_id]
                return False
            return True
        return False

    def reset(self, skill_id: str) -> None:
        """Manually reset circuit for a skill."""
        self._failures.pop(skill_id, None)
        self._open_until.pop(skill_id, None)


class TimeoutHook(SkillHook):
    """Enforces timeout limits on skill execution."""

    def __init__(self, default_timeout: float = 120.0) -> None:
        self.default_timeout = default_timeout
        self._deadlines: Dict[str, float] = {}

    def get_name(self) -> str:
        return "timeout_hook"

    def pre_execute(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
    ) -> Tuple[bool, Optional[str]]:
        """Set deadline for execution."""
        timeout = descriptor.timeout_s if descriptor.timeout_s > 0 else self.default_timeout
        self._deadlines[descriptor.skill_id] = time.time() + timeout
        return True, None

    def post_execute(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
        outputs: Dict[str, Any],
        duration_ms: float,
    ) -> Tuple[bool, Optional[str]]:
        """Clean up deadline after execution."""
        self._deadlines.pop(descriptor.skill_id, None)
        return True, None

    def check_timeout(self, skill_id: str) -> bool:
        """Check if execution has exceeded timeout."""
        if skill_id not in self._deadlines:
            return False
        return time.time() > self._deadlines[skill_id]

    def get_remaining_time(self, skill_id: str) -> Optional[float]:
        """Get remaining time before timeout."""
        if skill_id not in self._deadlines:
            return None
        remaining = self._deadlines[skill_id] - time.time()
        return max(0.0, remaining)


class RateLimitHook(SkillHook):
    """Rate limiting for skill invocations."""

    def __init__(
        self,
        max_calls_per_minute: int = 60,
        max_calls_per_hour: int = 1000,
    ) -> None:
        self.max_per_minute = max_calls_per_minute
        self.max_per_hour = max_calls_per_hour
        self._call_history: Dict[str, List[datetime]] = {}

    def get_name(self) -> str:
        return "rate_limit_hook"

    def pre_execute(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
    ) -> Tuple[bool, Optional[str]]:
        """Check rate limits before execution."""
        skill_id = descriptor.skill_id
        now = datetime.now()

        if skill_id not in self._call_history:
            self._call_history[skill_id] = []

        # Clean old history
        minute_ago = now.timestamp() - 60
        hour_ago = now.timestamp() - 3600
        self._call_history[skill_id] = [
            t for t in self._call_history[skill_id]
            if t.timestamp() > hour_ago
        ]

        # Check limits
        minute_calls = sum(1 for t in self._call_history[skill_id] if t.timestamp() > minute_ago)
        hour_calls = len(self._call_history[skill_id])

        if minute_calls >= self.max_per_minute:
            return False, f"Rate limit exceeded: {minute_calls} calls per minute"

        if hour_calls >= self.max_per_hour:
            return False, f"Rate limit exceeded: {hour_calls} calls per hour"

        # Record this call
        self._call_history[skill_id].append(now)

        return True, None

    def get_usage_stats(self, skill_id: str) -> Dict[str, Any]:
        """Get current usage statistics for a skill."""
        if skill_id not in self._call_history:
            return {"minute_calls": 0, "hour_calls": 0}

        now = datetime.now()
        minute_ago = now.timestamp() - 60
        hour_ago = now.timestamp() - 3600

        minute_calls = sum(
            1 for t in self._call_history[skill_id]
            if t.timestamp() > minute_ago
        )
        hour_calls = sum(
            1 for t in self._call_history[skill_id]
            if t.timestamp() > hour_ago
        )

        return {
            "minute_calls": minute_calls,
            "minute_limit": self.max_per_minute,
            "hour_calls": hour_calls,
            "hour_limit": self.max_per_hour,
        }


def create_production_hooks(
    config: HookConfig,
    evidence_dir: Path,
) -> List[SkillHook]:
    """Create standard production hook chain."""
    return [
        LoggingHook(config),
        EvidenceChainHook(evidence_dir),
        DegradationMonitorHook(config),
        CacheHook(config),
        CircuitBreakerHook(),
        TimeoutHook(),
        RateLimitHook(),
    ]
