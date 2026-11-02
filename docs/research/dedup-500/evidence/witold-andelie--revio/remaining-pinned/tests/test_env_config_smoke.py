"""Smoke test for the `REVIO_*` env layer and the container deployment.

Env vars are the only config source that needs no writable filesystem, which
makes them the load-bearing path for Docker and CI. Two things can break it
silently: the layer itself regressing, and `docker-compose.yml` drifting to
name a config field that no longer exists — pydantic ignores unknown keys, so
a typo there produces a container that runs with defaults and reviews nothing.

Covers:
  · REVIO_<SECTION>__<FIELD> maps to the nested config, with type coercion
  · Deep nesting (mcp.servers.<name>.command) works
  · Stray REVIO_-prefixed vars are ignored, not fatal
  · Env beats every config file
  · config_exists() counts [llm] from the env, so a container skips the wizard
  · every REVIO_* var in docker-compose.yml names a field that really exists
  · the deployment files exist and keep the two settings that quietly matter

Run:
    .venv/bin/python tests/test_env_config_smoke.py
"""

from __future__ import annotations

import os
import sys
import tempfile
from contextlib import contextmanager
from pathlib import Path
from typing import get_origin

REPO_ROOT = Path(__file__).resolve().parent.parent


@contextmanager
def clean_env(**overrides: str):
    """Run with only the given REVIO_* vars set, restoring os.environ after.

    Every REVIO_ var is stripped first so the developer's own environment
    can't make a check pass (or fail) for the wrong reason.
    """
    saved = dict(os.environ)
    try:
        for key in [k for k in os.environ if k.startswith("REVIO_")]:
            del os.environ[key]
        os.environ.update(overrides)
        yield
    finally:
        os.environ.clear()
        os.environ.update(saved)


def test_env_maps_to_nested_config():
    print("\n[1] REVIO_<SECTION>__<FIELD> lands in the right place, coerced")
    from revio.config import env_overrides

    with clean_env(
        REVIO_LLM__MODEL="qwen2.5:7b",
        REVIO_LLM__API_URL="http://ollama:11434/v1",
        REVIO_AGENT__MAX_TOOL_CALLS="30",
        REVIO_MCP__SERVERS__JIRA__COMMAND="jira-mcp",
    ):
        nested = env_overrides()

    assert nested["llm"] == {"model": "qwen2.5:7b", "api_url": "http://ollama:11434/v1"}
    assert nested["agent"] == {"max_tool_calls": "30"}
    assert nested["mcp"]["servers"]["jira"]["command"] == "jira-mcp", \
        "arbitrary nesting depth is what makes MCP servers configurable in a container"
    print("  · flat + 4-level-deep keys both nest correctly ✓")


def test_stray_vars_are_ignored():
    print("\n[2] REVIO_-prefixed noise doesn't reach the config")
    from revio.config import env_overrides

    with clean_env(
        REVIO_DEBUG="1",                 # documented, but not a config field
        REVIO_LLM="whole-section",       # would replace a sub-model with a str
        REVIO_NOT_A_SECTION__FIELD="x",
        REVIO_LLM__="empty-leaf",
    ):
        nested = env_overrides()

    assert nested == {}, f"expected all four to be ignored, got {nested}"
    print("  · REVIO_DEBUG / bare section / unknown section / empty leaf all skipped ✓")


def test_env_beats_config_files():
    print("\n[3] env wins over ~/.config and ./.revio.toml")
    from revio.config import load_config

    with tempfile.TemporaryDirectory() as home, tempfile.TemporaryDirectory() as proj:
        cfg_dir = Path(home) / "revio"
        cfg_dir.mkdir(parents=True)
        (cfg_dir / "config.toml").write_text(
            '[llm]\nmodel = "from-user-config"\napi_url = "https://from-user-config"\n',
            encoding="utf-8",
        )
        (Path(proj) / ".revio.toml").write_text(
            '[llm]\nmodel = "from-project-config"\n', encoding="utf-8"
        )

        with clean_env(XDG_CONFIG_HOME=home, REVIO_LLM__MODEL="from-env"):
            cfg = load_config(Path(proj))

        assert cfg.llm.model == "from-env", f"env lost to a file: {cfg.llm.model}"
        # The file layer must still apply where the env is silent — env is an
        # override, not a replacement.
        assert cfg.llm.api_url == "https://from-user-config", \
            "env override wiped an unrelated field from the file layer"
    print("  · REVIO_LLM__MODEL overrides both files; untouched fields survive ✓")


def test_env_only_config_skips_the_wizard():
    print("\n[4] config_exists() counts the env, so a container doesn't hang")
    from revio.config import config_exists

    with tempfile.TemporaryDirectory() as home, tempfile.TemporaryDirectory() as proj:
        cwd = Path.cwd()
        os.chdir(proj)  # so project_config_paths() finds nothing either
        try:
            with clean_env(XDG_CONFIG_HOME=home, REVIO_LLM__MODEL="qwen2.5:7b"):
                assert config_exists() is True, \
                    "a container configured via REVIO_LLM__* would drop into the wizard"
            # [output] alone is not something you can review code with — that
            # user still wants the wizard.
            with clean_env(XDG_CONFIG_HOME=home, REVIO_OUTPUT__DEFAULT_FORMAT="json"):
                assert config_exists() is False, \
                    "a stray non-llm var must not suppress first-run setup"

            # wizard.needs_wizard() must stay the exact negation of this. It
            # used to test the user config file on its own, which would have
            # re-opened the container hang the moment anything called it.
            from revio.cli.wizard import needs_wizard

            for env_kw in ({"REVIO_LLM__MODEL": "qwen2.5:7b"},
                           {"REVIO_OUTPUT__DEFAULT_FORMAT": "json"}):
                with clean_env(XDG_CONFIG_HOME=home, **env_kw):
                    assert needs_wizard() is (not config_exists()), \
                        f"needs_wizard() disagrees with config_exists() for {env_kw}"
        finally:
            os.chdir(cwd)
    print("  · [llm] in env → configured; [output] alone → still needs the wizard ✓")
    print("  · needs_wizard() is the exact negation of config_exists() ✓")


def _names_a_real_field(parts: list[str]) -> bool:
    """True if a dotted path names a settable field on Config.

    Pydantic ignores unknown keys by default, so `model_validate` would happily
    swallow a typo — this walks the field tree instead.
    """
    from pydantic import BaseModel

    from revio.config import Config

    model: type[BaseModel] | None = Config
    for i, part in enumerate(parts):
        if model is None:
            return False
        fields = model.model_fields
        if part not in fields:
            return False
        ann = fields[part].annotation
        if get_origin(ann) is dict:
            return True  # e.g. mcp.servers.<name> — keys below are user-chosen
        if isinstance(ann, type) and issubclass(ann, BaseModel):
            model = ann
            continue
        # Scalar leaf (str / int / bool / Literal): nothing can nest below it.
        return i == len(parts) - 1
    return False


def test_compose_env_vars_name_real_fields():
    print("\n[5] every REVIO_* in docker-compose.yml maps to a real config field")
    import yaml

    from revio.config import ENV_DELIMITER, ENV_PREFIX

    compose_path = REPO_ROOT / "docker-compose.yml"
    assert compose_path.is_file(), "docker-compose.yml is part of the deliverable"
    compose = yaml.safe_load(compose_path.read_text(encoding="utf-8"))

    checked = 0
    for svc_name, svc in (compose.get("services") or {}).items():
        env = svc.get("environment") or {}
        if isinstance(env, list):  # `- KEY=value` form
            env = dict(item.split("=", 1) for item in env if "=" in item)
        for key in env:
            if not key.startswith(ENV_PREFIX):
                continue
            parts = key[len(ENV_PREFIX):].lower().split(ENV_DELIMITER)
            assert _names_a_real_field(parts), \
                f"{svc_name}: {key} does not name a config field — the container " \
                f"would silently run with defaults"
            checked += 1

    assert checked >= 4, f"expected the revio service to be configured via env, saw {checked}"
    print(f"  · {checked} env vars all resolve against the live Config schema ✓")


def test_deployment_files_keep_what_matters():
    print("\n[6] Dockerfile / compose / .dockerignore keep their load-bearing bits")
    dockerfile = (REPO_ROOT / "Dockerfile").read_text(encoding="utf-8")
    compose = (REPO_ROOT / "docker-compose.yml").read_text(encoding="utf-8")
    dockerignore = (REPO_ROOT / ".dockerignore").read_text(encoding="utf-8")

    # Without this, git refuses to read a bind-mounted repo owned by another
    # uid and every review dies before opening a file.
    assert "safe.directory" in dockerfile, "Dockerfile lost the git safe.directory fix"

    # Ollama's default context is 4096 tokens and it truncates in silence;
    # it cannot be set from the client over the OpenAI-compatible API.
    assert "OLLAMA_CONTEXT_LENGTH" in compose, "compose lost the Ollama context override"

    # Ollama serves the OpenAI-compatible API under /v1; a bare :11434 404s.
    assert "http://ollama:11434/v1" in compose, "compose lost the /v1 suffix"

    # Deny-by-default: the first rule must exclude everything, or a stray
    # .revio.local.toml / .env can end up in a published layer.
    rules = [ln.strip() for ln in dockerignore.splitlines()
             if ln.strip() and not ln.strip().startswith("#")]
    assert rules and rules[0] == "*", \
        ".dockerignore must start by excluding everything, then re-include"
    print("  · safe.directory, OLLAMA_CONTEXT_LENGTH, /v1, deny-by-default ✓")


def main():
    # Argument-free on purpose: this file is script-style, but pytest collects
    # it too, and a bare parameter reads there as a missing fixture.
    test_env_maps_to_nested_config()
    test_stray_vars_are_ignored()
    test_env_beats_config_files()
    test_env_only_config_skips_the_wizard()
    test_compose_env_vars_name_real_fields()
    test_deployment_files_keep_what_matters()
    print("\nAll env-config / deployment smoke checks PASSED.")


if __name__ == "__main__":
    sys.exit(main() or 0)
