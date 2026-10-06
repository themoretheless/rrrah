# -*- coding: utf-8 -*-
"""Flexible Skill Registry System for dmca-takedown-assistant.

This module provides a production-grade skill registry that supports:
- Dynamic skill registration and resolution
- Input/output schema validation
- Skill dependency management
- Execution lifecycle hooks
- Performance metrics and monitoring

The registry follows the principle of lack of surprise: every skill must
declare its capabilities, inputs, outputs, and dependencies explicitly.
"""
from __future__ import annotations

import hashlib
import json
import time
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, Tuple, Type, TypeVar, Union
from functools import wraps
import hashlib
import hmac

# Type aliases for clarity
SkillID = str
Version = str
ContextKey = str
ContextValue = Any


class SkillState(Enum):
    """States in the skill execution lifecycle."""
    REGISTERED = "registered"
    VALIDATING = "validating"
    EXECUTING = "executing"
    SUCCEEDED = "succeeded"
    FAILED = "failed"
    CANCELLED = "cancelled"
    DEGRADED = "degraded"


class ExecutionPriority(Enum):
    """Priority levels for skill execution."""
    CRITICAL = 0  # Must succeed for analysis to continue
    HIGH = 1     # Should succeed; fallback available
    NORMAL = 2   # Best effort; graceful degradation OK
    LOW = 3      # Optional enhancement


@dataclass
class SkillSchema:
    """JSON schema definition for skill inputs/outputs."""
    schema_type: str  # "object", "array", "string", etc.
    properties: Dict[str, 'SkillSchema'] = field(default_factory=dict)
    required: List[str] = field(default_factory=list)
    items: Optional['SkillSchema'] = None  # For array types
    description: str = ""
    enum: Optional[List[Any]] = None
    pattern: Optional[str] = None
    min_items: Optional[int] = None
    max_items: Optional[int] = None

    def to_dict(self) -> Dict[str, Any]:
        """Convert to JSON-serializable dictionary."""
        result = {"type": self.schema_type, "description": self.description}
        if self.properties:
            result["properties"] = {
                k: v.to_dict() for k, v in self.properties.items()
            }
        if self.required:
            result["required"] = self.required
        if self.items:
            result["items"] = self.items.to_dict()
        if self.enum is not None:
            result["enum"] = self.enum
        if self.pattern:
            result["pattern"] = self.pattern
        if self.min_items is not None:
            result["minItems"] = self.min_items
        if self.max_items is not None:
            result["maxItems"] = self.max_items
        return result

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'SkillSchema':
        """Create from JSON-serializable dictionary."""
        properties = {
            k: cls.from_dict(v) for k, v in data.get("properties", {}).items()
        }
        items = cls.from_dict(data["items"]) if data.get("items") else None
        return cls(
            schema_type=data["type"],
            properties=properties,
            required=data.get("required", []),
            items=items,
            description=data.get("description", ""),
            enum=data.get("enum"),
            pattern=data.get("pattern"),
            min_items=data.get("minItems"),
            max_items=data.get("maxItems"),
        )


@dataclass
class SkillMetrics:
    """Performance and execution metrics for a skill."""
    invocation_count: int = 0
    success_count: int = 0
    failure_count: int = 0
    total_duration_ms: float = 0.0
    avg_duration_ms: float = 0.0
    last_execution: Optional[datetime] = None
    last_success: Optional[datetime] = None
    last_failure: Optional[datetime] = None
    degradation_count: int = 0

    def record_execution(
        self,
        duration_ms: float,
        success: bool,
        degraded: bool = False,
    ) -> None:
        """Record an execution outcome."""
        self.invocation_count += 1
        self.total_duration_ms += duration_ms
        self.avg_duration_ms = self.total_duration_ms / self.invocation_count
        now = datetime.now()
        self.last_execution = now

        if success:
            self.success_count += 1
            self.last_success = now
        else:
            self.failure_count += 1
            self.last_failure = now

        if degraded:
            self.degradation_count += 1

    @property
    def success_rate(self) -> float:
        """Calculate success rate (0.0 to 1.0)."""
        if self.invocation_count == 0:
            return 1.0
        return self.success_count / self.invocation_count


@dataclass
class SkillDescriptor:
    """Complete description of a skill's capabilities and contract."""
    skill_id: SkillID
    name: str
    description: str
    version: Version
    author: str = ""
    input_schema: Optional[SkillSchema] = None
    output_schema: Optional[SkillSchema] = None
    dependencies: List[SkillID] = field(default_factory=list)
    priority: ExecutionPriority = ExecutionPriority.NORMAL
    timeout_s: float = 120.0
    retry_count: int = 2
    backoff_base: float = 2.0
    tags: List[str] = field(default_factory=list)
    metadata: Dict[str, Any] = field(default_factory=dict)
    requires_tools: List[str] = field(default_factory=list)
    produces_artifacts: List[str] = field(default_factory=list)

    def to_dict(self) -> Dict[str, Any]:
        """Serialize to dictionary."""
        return {
            "skill_id": self.skill_id,
            "name": self.name,
            "description": self.description,
            "version": self.version,
            "author": self.author,
            "input_schema": self.input_schema.to_dict() if self.input_schema else None,
            "output_schema": self.output_schema.to_dict() if self.output_schema else None,
            "dependencies": self.dependencies,
            "priority": self.priority.value,
            "timeout_s": self.timeout_s,
            "retry_count": self.retry_count,
            "backoff_base": self.backoff_base,
            "tags": self.tags,
            "metadata": self.metadata,
            "requires_tools": self.requires_tools,
            "produces_artifacts": self.produces_artifacts,
        }

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'SkillDescriptor':
        """Deserialize from dictionary."""
        return cls(
            skill_id=data["skill_id"],
            name=data["name"],
            description=data["description"],
            version=data["version"],
            author=data.get("author", ""),
            input_schema=SkillSchema.from_dict(data["input_schema"]) if data.get("input_schema") else None,
            output_schema=SkillSchema.from_dict(data["output_schema"]) if data.get("output_schema") else None,
            dependencies=data.get("dependencies", []),
            priority=ExecutionPriority(data.get("priority", 2)),
            timeout_s=data.get("timeout_s", 120.0),
            retry_count=data.get("retry_count", 2),
            backoff_base=data.get("backoff_base", 2.0),
            tags=data.get("tags", []),
            metadata=data.get("metadata", {}),
            requires_tools=data.get("requires_tools", []),
            produces_artifacts=data.get("produces_artifacts", []),
        )

    def compute_hash(self) -> str:
        """Compute stable hash of descriptor for versioning."""
        canonical = json.dumps(self.to_dict(), sort_keys=True)
        return hashlib.sha256(canonical.encode()).hexdigest()[:16]


T = TypeVar('T')


class SkillHook(ABC):
    """Abstract base for skill lifecycle hooks."""

    @abstractmethod
    def get_name(self) -> str:
        """Return hook name for registration."""

    def pre_validate(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
    ) -> Tuple[bool, Optional[str]]:
        """Called before input validation. Return (should_continue, error_message)."""
        return True, None

    def post_validate(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
    ) -> Tuple[bool, Optional[str]]:
        """Called after input validation succeeds."""
        return True, None

    def pre_execute(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
    ) -> Tuple[bool, Optional[str]]:
        """Called immediately before skill execution."""
        return True, None

    def post_execute(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
        outputs: Dict[str, Any],
        duration_ms: float,
    ) -> Tuple[bool, Optional[str]]:
        """Called immediately after skill execution completes."""
        return True, None

    def on_failure(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
        error: Exception,
    ) -> None:
        """Called when skill execution fails."""
        pass

    def on_degradation(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
        degradation_level: int,
    ) -> None:
        """Called when skill executes in degraded mode."""
        pass

    def on_retry(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
        attempt: int,
        max_attempts: int,
    ) -> None:
        """Called before a retry attempt."""
        pass


class SkillExecutor(ABC):
    """Abstract base for skill execution strategies."""

    @abstractmethod
    def execute(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
        context: 'ExecutionContext',
    ) -> Tuple[Dict[str, Any], SkillState]:
        """Execute skill with given inputs. Return (outputs, final_state)."""
        pass


@dataclass
class ExecutionContext:
    """Execution context passed to skills containing shared state."""
    case_id: str
    language: str = "en"
    start_time: datetime = field(default_factory=datetime.now)
    metadata: Dict[str, Any] = field(default_factory=dict)
    state: Dict[ContextKey, ContextValue] = field(default_factory=dict)
    evidence_chain: List[Dict[str, Any]] = field(default_factory=list)
    degradation_level: int = 0

    def get(self, key: ContextKey, default: T = None) -> T:
        """Get value from context state."""
        return self.state.get(key, default)

    def set(self, key: ContextKey, value: ContextValue) -> None:
        """Set value in context state."""
        self.state[key] = value

    def add_evidence(
        self,
        source: str,
        content: Any,
        tier: int = 4,
        timestamp: Optional[datetime] = None,
    ) -> None:
        """Add evidence to the chain."""
        self.evidence_chain.append({
            "source": source,
            "content": content,
            "tier": tier,
            "timestamp": timestamp or datetime.now(),
        })


class SkillRegistry:
    """Central registry for skill descriptors and execution management.

    The registry provides:
    - Skill registration and discovery
    - Dependency resolution
    - Input/output validation
    - Execution lifecycle management
    - Performance metrics
    """

    def __init__(self) -> None:
        self._skills: Dict[SkillID, SkillDescriptor] = {}
        self._executors: Dict[SkillID, SkillExecutor] = {}
        self._hooks: List[SkillHook] = []
        self._metrics: Dict[SkillID, SkillMetrics] = {}

    def register(
        self,
        descriptor: SkillDescriptor,
        executor: SkillExecutor,
    ) -> None:
        """Register a skill with its executor.

        Raises:
            ValueError: If skill_id already registered or dependencies missing.
        """
        if descriptor.skill_id in self._skills:
            raise ValueError(f"Skill {descriptor.skill_id} already registered")

        # Validate dependencies
        for dep_id in descriptor.dependencies:
            if dep_id not in self._skills:
                raise ValueError(f"Dependency {dep_id} not found for {descriptor.skill_id}")

        self._skills[descriptor.skill_id] = descriptor
        self._executors[descriptor.skill_id] = executor
        self._metrics[descriptor.skill_id] = SkillMetrics()

    def unregister(self, skill_id: SkillID) -> None:
        """Remove a skill from the registry."""
        # Check if other skills depend on this one
        for desc in self._skills.values():
            if skill_id in desc.dependencies:
                raise ValueError(f"Cannot unregister {skill_id}: required by {desc.skill_id}")

        self._skills.pop(skill_id, None)
        self._executors.pop(skill_id, None)
        self._metrics.pop(skill_id, None)

    def get(self, skill_id: SkillID) -> Optional[SkillDescriptor]:
        """Get skill descriptor by ID."""
        return self._skills.get(skill_id)

    def list_all(self) -> List[SkillDescriptor]:
        """List all registered skills."""
        return list(self._skills.values())

    def find_by_tag(self, tag: str) -> List[SkillDescriptor]:
        """Find skills with a given tag."""
        return [s for s in self._skills.values() if tag in s.tags]

    def add_hook(self, hook: SkillHook) -> None:
        """Add a lifecycle hook to all skill executions."""
        self._hooks.append(hook)

    def remove_hook(self, hook: SkillHook) -> None:
        """Remove a lifecycle hook."""
        if hook in self._hooks:
            self._hooks.remove(hook)

    def validate_inputs(
        self,
        descriptor: SkillDescriptor,
        inputs: Dict[str, Any],
    ) -> Tuple[bool, Optional[str]]:
        """Validate inputs against skill's schema.

        Returns (is_valid, error_message).
        """
        if descriptor.input_schema is None:
            return True, None

        schema = descriptor.input_schema

        # Check required fields
        missing = [k for k in schema.required if k not in inputs]
        if missing:
            return False, f"Missing required fields: {', '.join(missing)}"

        # Type validation
        for key, value in inputs.items():
            if key not in schema.properties:
                continue  # Extra fields OK for extensibility

            prop_schema = schema.properties[key]

            if prop_schema.schema_type == "string":
                if not isinstance(value, str):
                    return False, f"Field '{key}' must be string"
                if prop_schema.pattern and not _matches_pattern(value, prop_schema.pattern):
                    return False, f"Field '{key}' does not match pattern {prop_schema.pattern}"
                if prop_schema.enum and value not in prop_schema.enum:
                    return False, f"Field '{key}' must be one of {prop_schema.enum}"

            elif prop_schema.schema_type == "number":
                if not isinstance(value, (int, float)):
                    return False, f"Field '{key}' must be number"
                if prop_schema.enum and value not in prop_schema.enum:
                    return False, f"Field '{key}' must be one of {prop_schema.enum}"

            elif prop_schema.schema_type == "boolean":
                if not isinstance(value, bool):
                    return False, f"Field '{key}' must be boolean"

            elif prop_schema.schema_type == "array":
                if not isinstance(value, list):
                    return False, f"Field '{key}' must be array"
                if prop_schema.min_items and len(value) < prop_schema.min_items:
                    return False, f"Field '{key}' must have at least {prop_schema.min_items} items"
                if prop_schema.max_items and len(value) > prop_schema.max_items:
                    return False, f"Field '{key}' must have at most {prop_schema.max_items} items"

            elif prop_schema.schema_type == "object":
                if not isinstance(value, dict):
                    return False, f"Field '{key}' must be object"

        return True, None

    def validate_outputs(
        self,
        descriptor: SkillDescriptor,
        outputs: Dict[str, Any],
    ) -> Tuple[bool, Optional[str]]:
        """Validate outputs against skill's schema."""
        if descriptor.output_schema is None:
            return True, None
        return self.validate_inputs(descriptor, outputs)

    def execute(
        self,
        skill_id: SkillID,
        inputs: Dict[str, Any],
        context: ExecutionContext,
    ) -> Tuple[Dict[str, Any], SkillState]:
        """Execute a registered skill with full lifecycle management.

        Returns (outputs, final_state).
        """
        descriptor = self._skills.get(skill_id)
        executor = self._executors.get(skill_id)
        metrics = self._metrics.get(skill_id)

        if not descriptor or not executor or not metrics:
            raise ValueError(f"Skill {skill_id} not properly registered")

        start_time = time.time()
        current_attempt = 0

        try:
            # Pre-validate hooks
            for hook in self._hooks:
                should_continue, error = hook.pre_validate(descriptor, inputs)
                if not should_continue:
                    return {}, SkillState.CANCELLED

            # Input validation
            is_valid, error = self.validate_inputs(descriptor, inputs)
            if not is_valid:
                raise ValueError(f"Input validation failed: {error}")

            # Post-validate hooks
            for hook in self._hooks:
                should_continue, error = hook.post_validate(descriptor, inputs)
                if not should_continue:
                    return {}, SkillState.CANCELLED

            # Execution with retry logic
            last_error = None
            for current_attempt in range(descriptor.retry_count + 1):
                if current_attempt > 0:
                    # Notify retry hook
                    for hook in self._hooks:
                        hook.on_retry(descriptor, inputs, current_attempt, descriptor.retry_count)
                    # Exponential backoff
                    time.sleep(descriptor.backoff_base ** current_attempt)

                try:
                    # Pre-execute hooks
                    for hook in self._hooks:
                        should_continue, error = hook.pre_execute(descriptor, inputs)
                        if not should_continue:
                            return {}, SkillState.CANCELLED

                    # Execute
                    outputs, state = executor.execute(descriptor, inputs, context)

                    duration_ms = (time.time() - start_time) * 1000

                    # Post-execute hooks
                    for hook in self._hooks:
                        should_continue, error = hook.post_execute(
                            descriptor, inputs, outputs, duration_ms
                        )
                        if not should_continue:
                            return {}, SkillState.CANCELLED

                    # Output validation
                    is_valid, error = self.validate_outputs(descriptor, outputs)
                    if not is_valid:
                        raise ValueError(f"Output validation failed: {error}")

                    # Record success
                    metrics.record_execution(
                        duration_ms,
                        success=True,
                        degraded=(state == SkillState.DEGRADED),
                    )

                    return outputs, state

                except Exception as e:
                    last_error = e
                    # Notify failure hook
                    for hook in self._hooks:
                        hook.on_failure(descriptor, inputs, e)

            # All retries exhausted
            raise last_error or RuntimeError("Execution failed after retries")

        except Exception as e:
            duration_ms = (time.time() - start_time) * 1000
            metrics.record_execution(duration_ms, success=False)
            raise

    def get_metrics(self, skill_id: SkillID) -> Optional[SkillMetrics]:
        """Get execution metrics for a skill."""
        return self._metrics.get(skill_id)

    def export_registry(self) -> Dict[str, Any]:
        """Export registry state for debugging/monitoring."""
        return {
            "skills": {
                skill_id: desc.to_dict()
                for skill_id, desc in self._skills.items()
            },
            "metrics": {
                skill_id: {
                    "invocation_count": m.invocation_count,
                    "success_count": m.success_count,
                    "failure_count": m.failure_count,
                    "success_rate": m.success_rate,
                    "avg_duration_ms": m.avg_duration_ms,
                    "last_execution": m.last_execution.isoformat() if m.last_execution else None,
                }
                for skill_id, m in self._metrics.items()
            },
        }


def _matches_pattern(value: str, pattern: str) -> bool:
    """Simple pattern matching (extend with regex as needed)."""
    import re
    try:
        return re.match(pattern, value) is not None
    except re.error:
        return False
