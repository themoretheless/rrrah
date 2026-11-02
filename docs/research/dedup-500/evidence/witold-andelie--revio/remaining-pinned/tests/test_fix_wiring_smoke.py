"""Smoke test for `--fix` as a cross-cutting stage (not a dedup-only flag).

`fix` is not a review mode — it has no prompt, no graph node, and makes no
LLM call. It is a post-processing stage over whatever `propose_patch` calls
the agent made, and `propose_patch` is in every mode's toolbox. These checks
pin that wiring down so review/audit can't silently lose the flag again.

Covers:
  · `--fix` (and its 4 companion flags) exist on review / audit / dedup
  · `propose_patch` survives every mode's tool blacklist
  · The last-session patch cache round-trips
  · `/fix` is registered in BOTH the help table and the handler table
  · The NL router can reach fix, and does NOT hijack "fix this bug"
  · `run_fix_flow` returns cleanly on an empty patch list
  · A keyless local endpoint builds an LLM; Anthropic without a key still raises
  · Every registered profile is writable into config (not just the old four)

Run:
    .venv/bin/python tests/test_fix_wiring_smoke.py
"""

from __future__ import annotations

import shutil
import sys
import tempfile
from pathlib import Path


FIX_FLAGS = ("--fix", "--dry-run", "--yes", "--min-confidence", "--allow-dirty")
MODES = ("review", "audit", "dedup")


def test_fix_flags_on_every_mode():
    print("\n[1] --fix and companions declared on all three mode commands")
    import importlib
    import inspect

    # NOT `from revio.cli import main` — revio/cli/__init__.py rebinds that
    # name to the entry-point function, so the attribute is the function, not
    # the module.
    cli_main = importlib.import_module("revio.cli.main")

    for mode in MODES:
        fn = getattr(cli_main, mode)
        params = inspect.signature(fn).parameters
        for name in ("fix", "dry_run", "yes", "min_confidence", "allow_dirty"):
            assert name in params, f"`revio {mode}` is missing the {name!r} parameter"
        # The Typer metadata must carry the actual CLI flag strings, not just
        # the Python names — a renamed param would still pass the check above.
        declared = {
            s
            for name in ("fix", "dry_run", "yes", "min_confidence", "allow_dirty")
            for s in getattr(params[name].default, "param_decls", None) or ()
        }
        missing = set(FIX_FLAGS) - declared
        assert not missing, f"`revio {mode}` is missing flags: {sorted(missing)}"
        print(f"  · revio {mode}: {', '.join(FIX_FLAGS)} ✓")


def test_propose_patch_in_every_mode_toolset():
    print("\n[2] propose_patch survives every mode's tool blacklist")
    from revio.agent.graph import _MODE_TOOL_BLACKLIST, _filter_tools_for_mode
    from revio.agent.tools import propose_patch

    for mode in MODES:
        blacklist = _MODE_TOOL_BLACKLIST.get(mode, set())
        assert "propose_patch" not in blacklist, \
            f"propose_patch is blacklisted in {mode} — --fix would be a no-op there"
        kept = _filter_tools_for_mode([propose_patch], mode)
        assert [t.name for t in kept] == ["propose_patch"], \
            f"propose_patch filtered out of {mode}'s toolset"
        print(f"  · {mode}: propose_patch present ✓")


def test_patch_cache_roundtrip():
    print("\n[3] last-session patch cache round-trips")
    from revio.agent.patch import (
        PatchOp,
        PatchSet,
        get_last_session_patches,
        set_last_session_patches,
    )

    set_last_session_patches([])
    assert get_last_session_patches() == [], "cache did not start empty"

    ps = PatchSet(
        title="dedupe helper",
        description="test",
        confidence=0.95,
        ops=[PatchOp(op_type="delete_lines", file_path="a.py", line_start=1,
                     line_end=1, old_content="x\n", reason="test")],
    )
    set_last_session_patches([ps])
    got = get_last_session_patches()
    assert len(got) == 1 and got[0].title == "dedupe helper", f"round-trip failed: {got}"

    # A returned list must be a copy — a caller mutating it must not poison
    # the cache for the next reader.
    got.clear()
    assert len(get_last_session_patches()) == 1, "cache handed out its internal list"
    set_last_session_patches([])
    print("  · set → get → copy-on-read ✓")


def test_runner_caches_patches_for_every_mode():
    print("\n[4] the runner caches patches regardless of mode")
    import inspect

    from revio.agent import runner

    src = inspect.getsource(runner)
    assert "set_last_session_patches" in src, \
        "runner no longer feeds the patch cache — --fix and /fix would see nothing"
    print("  · runner calls set_last_session_patches ✓")


def test_repl_fix_command_registered():
    print("\n[5] /fix registered in both the help table and the handler table")
    import inspect

    from revio.cli import repl

    assert "/fix" in repl.SLASH_COMMANDS, "/fix missing from SLASH_COMMANDS (no help/completion)"
    handler_src = inspect.getsource(repl._handle_slash)
    assert '"/fix": _cmd_fix' in handler_src, "/fix missing from the _handle_slash dispatch table"
    assert callable(repl._cmd_fix)
    print("  · SLASH_COMMANDS + _handle_slash both know /fix ✓")


def test_nl_router_reaches_fix():
    print("\n[6] NL router can reach fix without hijacking 'fix this bug'")
    from revio.cli import repl

    assert "fix" in repl._VALID_INTENTS, "'fix' would be rejected by the intent validator"
    assert "fix" in repl._INTENT_SYSTEM, "classifier prompt never mentions the fix intent"

    for phrase in ("apply the fix", "please apply patches", "应用补丁", "把补丁应用上"):
        got = repl._keyword_classify(phrase)["intent"]
        assert got == "fix", f"{phrase!r} classified as {got!r}, expected 'fix'"
        print(f"  · {phrase!r} → fix ✓")

    # Deliberately NOT fix: this reads as "go find and fix it", and routing it
    # to /fix would silently apply a stale earlier run's patches.
    got = repl._keyword_classify("fix this bug in src/a.py")["intent"]
    assert got != "fix", "'fix this bug' hijacked into applying stale patches"
    print(f"  · 'fix this bug in src/a.py' → {got} (not fix) ✓")

    # And the dispatcher must actually have a branch for it.
    import inspect

    dispatch = inspect.getsource(repl._handle_nl_input)
    assert 'intent == "fix"' in dispatch, "_handle_nl_input has no fix branch"
    print("  · _handle_nl_input dispatches fix ✓")


def test_empty_patches_returns_cleanly():
    print("\n[7] run_fix_flow on an empty patch list returns without raising")
    from revio.cli.fix import run_fix_flow

    repo = Path(tempfile.mkdtemp(prefix="revio_fix_wiring_empty_"))
    try:
        result = run_fix_flow(patches=[], repo_root=repo)
    finally:
        shutil.rmtree(repo, ignore_errors=True)
    assert not result.applied, f"applied something from an empty list: {result}"
    assert not result.failed, f"reported failures from an empty list: {result}"
    print("  · empty patch list → clean no-op ✓")


def test_keyless_local_endpoint():
    print("\n[8] keyless local endpoint builds; Anthropic without a key still raises")
    from revio.agent.llm import APIKeyMissingError, make_llm
    from revio.config import Config, LLMConfig

    def cfg(provider: str, url: str, model: str) -> Config:
        return Config(llm=LLMConfig(
            provider=provider, api_url=url, api_key="",
            # Point at a var that cannot be set, so resolve_api_key() can't
            # accidentally pick up the developer's real ANTHROPIC_API_KEY.
            api_key_env="REVIO_TEST_KEY_THAT_IS_NEVER_SET",
            model=model,
        ))

    for provider, url in (("openai_compat", "http://localhost:11434/v1"),
                          ("custom", "http://localhost:8000/v1")):
        llm = make_llm(cfg(provider, url, "qwen2.5:7b"))
        assert llm is not None, f"{provider} with no key produced no LLM"
        print(f"  · {provider} + empty key → {type(llm).__name__} ✓")

    try:
        make_llm(cfg("anthropic", "https://api.anthropic.com", "claude-sonnet-4-5"))
    except APIKeyMissingError:
        print("  · anthropic + empty key → APIKeyMissingError ✓")
    else:
        raise AssertionError("anthropic accepted an empty API key")


def test_every_profile_is_configurable():
    print("\n[9] every registered profile can be written into config")
    from revio.cli.main import _resolve_profile_name
    from revio.config import ProfileConfig
    from revio.profiles.base import list_profiles, load_all_profiles

    load_all_profiles()
    names = list_profiles()
    assert len(names) > 4, f"expected the full registry, got only {names}"
    for name in names:
        assert ProfileConfig(default=name).default == name, \
            f"config rejects the registered profile {name!r}"
    print(f"  · {len(names)} profiles accepted by ProfileConfig ✓")

    assert _resolve_profile_name("rust") == "rust"
    assert _resolve_profile_name("auto") == "auto"
    assert _resolve_profile_name("not-a-language") == "auto", \
        "an unknown profile should fall back to auto, not propagate"
    print("  · _resolve_profile_name validates against the live registry ✓")


def main():
    # Each check is argument-free on purpose: this file is script-style, but
    # pytest also collects it, and a bare parameter there reads as a missing
    # fixture and turns into a collection error.
    test_fix_flags_on_every_mode()
    test_propose_patch_in_every_mode_toolset()
    test_patch_cache_roundtrip()
    test_runner_caches_patches_for_every_mode()
    test_repl_fix_command_registered()
    test_nl_router_reaches_fix()
    test_empty_patches_returns_cleanly()
    test_keyless_local_endpoint()
    test_every_profile_is_configurable()
    print("\nAll fix-wiring smoke checks PASSED.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
