"""Profile registry and base class.

A "profile" is a per-language pack that supplies:
- Which file extensions it handles
- Which static-analysis tools to wire into Layer 2
- Optional framework-specific hints to inject into Layer 3 prompts

Profiles register themselves via `@register("name")` decorator at import time.
The CLI's auto-detect picks one (or several) based on ProjectFingerprint.

This file only defines the abstract base + registry. Concrete profiles live in
revio.profiles.js / .plc / .python and are imported by `load_all_profiles()`.
"""

from __future__ import annotations

from typing import ClassVar


# --- Registry ------------------------------------------------------------------


_REGISTRY: dict[str, type["ProfileBase"]] = {}


def register(name: str):
    """Decorator: associate a profile class with a name."""

    def deco(cls: type["ProfileBase"]) -> type["ProfileBase"]:
        if name in _REGISTRY:
            raise ValueError(f"Profile name '{name}' already registered")
        cls.name = name  # type: ignore[misc]
        _REGISTRY[name] = cls
        return cls

    return deco


def get_profile(name: str) -> type["ProfileBase"] | None:
    return _REGISTRY.get(name)


def list_profiles() -> list[str]:
    return sorted(_REGISTRY.keys())


def load_all_profiles() -> None:
    """Import every profile module so they self-register.

    Called once at startup. Safe to call multiple times (decorator raises on
    duplicate, so we swallow that). New profiles just add an `import` line.
    """
    # Local imports to avoid circular deps at module-load time.
    # Each profile's import is wrapped in try/except so a missing optional
    # dependency (e.g. tree-sitter grammar package) doesn't crash startup.
    for module_name in (
        # Full Layer 1 + Layer 2 profiles
        "js", "python", "rust", "java", "go", "cpp",
        # Phase-2 deep profiles (Tree-sitter + dedicated linter)
        "shell", "lua", "sql", "ruby", "php", "kotlin",
        # PLC vendor parsers + 30 PLCopen rules
        "plc",
        # Generic Tree-sitter fallback (Julia/Scala/Swift/C# — no dedicated linter yet)
        "generic",
        # LLM-only fallback profiles (no Tree-sitter grammar packaged)
        "matlab", "r", "verilog", "sas", "cobol", "solidity",
        "zig", "objc", "dart",
    ):
        try:
            __import__(f"revio.profiles.{module_name}")
        except (ImportError, ValueError):
            # ValueError catches the "already registered" raise when this
            # function is called more than once in the same process.
            pass


# --- Base class ----------------------------------------------------------------


class ProfileBase:
    """Abstract base for a language profile.

    Subclasses override class attributes for declarative metadata, plus the two
    hooks below to expose their analyzers to the agent.
    """

    # Set by @register at registration time
    name: ClassVar[str] = ""

    # Declarative metadata (override in subclass)
    description: ClassVar[str] = ""
    extensions: ClassVar[tuple[str, ...]] = ()
    languages: ClassVar[tuple[str, ...]] = ()  # internal language tags

    # Optional: extra deps the user needs to `pip install revio[<name>]` for
    optional_dep_group: ClassVar[str | None] = None

    # --- Profile hooks ---
    # Layer 1/2 analyzers are not constructed here — they hang off ToolContext,
    # lazily and with per-analyzer availability fallback, and reach the agent as
    # tools via make_tools() below.

    @classmethod
    def make_reasoning_hints(cls) -> str:
        """Layer 3: extra context to inject into the LLM system prompt.

        E.g. "This project uses React 18 — be alert to dangerouslySetInnerHTML."
        """
        return ""

    @classmethod
    def make_tools(cls, ctx) -> list:
        """Profile-specific agent tools (bound to a ToolContext).

        Default: no extra tools. Subclasses override to expose their
        Layer-1/Layer-2 capabilities to the LLM.
        """
        return []

    # --- Convenience ---

    @classmethod
    def handles_extension(cls, ext: str) -> bool:
        return ext.lower() in cls.extensions
