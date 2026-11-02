# Changelog

Every entry below is a real commit in this repository — the short hash lets
you read the diff for any line with `git show <hash>`. The canonical history
is the git log itself; this file is the human-readable index into it.

`git log --oneline --reverse` reproduces the same sequence.

Dates are the commit dates. There are no tagged releases yet — the package
version is `0.1.0` and everything below is pre-release work.

---

## Unreleased (`0.1.0`)

### 2026-07-26 — Documentation completeness

- `fc3bba3` **docs(mcp)**: the client half of the MCP docs covered 3 of
  `MCPServerSpec`'s 6 fields and only the stdio transport, leaving the whole
  SSE/HTTP path (`url`, `api_key_env`) undocumented. Now documents all six
  fields, both transports, which config file the table belongs in, the
  leading-`$` env substitution, the `🔌 MCP servers` status block, and the
  container caveat that stdio servers spawn inside the image. On the server
  side, records that only the 3 pipeline tools need an LLM key — the 13
  analyzer and 3 discovery tools don't. Also repoints
  `wizard.needs_wizard()` at `config_exists()`; it had no callers and tested
  the user config file alone, so wiring it up would have reintroduced the
  container wizard hang.
- `a9cdaae` **docs(readme)**: the change history was satisfied by a link
  alone; inlined as a per-phase table so the README answers the question on
  its own.

### 2026-07-26 — Container deployment

- `a1095af` **feat(docker)**: `Dockerfile`, `.dockerignore` and
  `docker-compose.yml`. Two deployment paths — `docker run` against any cloud
  provider, or `docker compose run` against a bundled Ollama service for a
  fully offline review. Two build stages so the C toolchain never ships;
  oxlint from its standalone release tarball rather than npm; non-root; git
  `safe.directory` so a bind-mounted repo is readable.
- `5a175b0` **feat(config)**: the `REVIO_*` environment layer, which the
  module header had advertised as the top config source since the first commit
  while `load_config` still had it as a TODO. `config_exists()` counts the env
  too, so an env-configured container no longer drops into the interactive
  wizard. Drops the unused `pydantic-settings` dependency.

### 2026-07-26 — `--fix` promoted to a first-class stage

- `be29496` **feat(fix)**: `--fix` moved out from under `dedup` and became a
  stage that *any* mode can end with. `review`/`audit`/`dedup` all take
  `--fix / --dry-run / --yes / --min-confidence / --allow-dirty`, sharing one
  implementation instead of two near-duplicate code paths. Adds the REPL
  `/fix` command (applies what the last run proposed, never starts a new run),
  a local-model preset in the setup wizard, and a working keyless path for
  Ollama / LM Studio / vLLM.
- `48e5e49` **fix(detect)**: one PLC file no longer repaints the whole repo as
  a PLC project.
- `8a17427` **refactor(profiles)**: dropped dead Layer-4 scaffolding.

### 2026-06-27 — Endpoint ergonomics

- `6ebbc22` **docs(readme)**: explicit `<YOUR_..._API_KEY>` placeholders in the
  config examples.
- `c2a6894` **feat(llm)**: opt-in OpenAI Responses API mode (`/responses`
  toggle) for endpoints that speak `/v1/responses` instead of chat completions.
- `2a099cb` **feat(repl, wizard)**: endpoint connectivity self-check and
  automatic `/v1` normalization.
- `469f1b7` **fix(repl)**: live slash-command filtering; `/url` is interactive
  and re-detects models.
- `424fe84` **feat(memory)**: count-based auto-pruning for every on-disk store,
  so findings history, checkpoints and REPL history can't grow unbounded.

### 2026-06-25 — Licensing + in-chat review

- `1681932` **chore**: Apache-2.0 `NOTICE` file.
- `19cf3ec` **chore**: relicensed from MIT to Apache-2.0.
- `83c36f7` **docs(readme)**: single-file and pasted-snippet review documented;
  PLC analyzer row added.
- `c4a8d23` **feat(repl)**: review a single file by path, or paste a code
  snippet straight into the chat — no repo or diff required.
- `2f03124` **docs(install)**: note that PLC is covered by built-in rules.

### 2026-06-01 — Accuracy pass

- `84dab5f` **fix(thinking-mode)**: correct Anthropic/reasoner sampling
  parameters; strip `reasoning_content` from tool-call turns.
- `0471579` README accuracy pass; all 13 analyzers exposed over MCP.
- `21ac554` **docs**: corrected the language matrix — Verilog is not LLM-only,
  and there are 13 analyzers, not 6.
- `484372c` **docs**: `BENCHMARKS.md` removed from the repo.

### 2026-05-31 — Natural-language control

- `df53d26` **feat(repl)**: plain-language commands drive reviews *and* change
  settings (model / endpoint / key / budget), with an explicit capability
  boundary for out-of-scope requests.

### 2026-05-24 — Distribution, analyzers, UX

- `cc32138`, `d8160b8` **marketing**: promo copy, later removed from the repo.
- `9f249bb`, `9ea7770` **feat/docs**: `revio analyzers` subcommand and
  post-install workflow docs.
- `514a942` **i18n**: agent output language follows the user's input language;
  owl mascot between tasks.
- `a44c1ca` **fix(repl)**: English example in the welcome banner.
- `b83da6e` **feat(installer)**: letter-code analyzer selection, replacing 13
  consecutive Y/N prompts.
- `bb968ed` **fix(repl)**: keyword fallback when the LLM intent classifier
  returns non-JSON.
- `1699844` **fix(config)**: the TOML serializer wrote nested empty dicts as
  the string `'{}'`, which broke the next startup.
- `a2275a3`, `c950bd0`, `550378b`, `1536105` **fix(installer)**: five Windows
  install bugs, RAG made opt-in, uninstall scripts, PATH refreshed in the
  current shell, `$Name:` PowerShell parsing, stderr no longer trips Stop mode.
- `1e6c17f` **feat**: Mistral promoted to a first-class EU-sovereign provider.
- `d7de30d` **docs(internals)**: local-LLM routing and verilator debugging.
- `d14e612` **feat**: verilator (Verilog Layer 2) and the first-class local-LLM
  story.
- `8c2ca5c` **feat**: six new static analyzers — shellcheck, luacheck,
  sqlfluff, rubocop, phpstan, detekt.
- `4b98c7c` **feat**: cross-platform one-click installer.
- `6cb6933`, `53bdd18` **feat**: snapshot-based fix-undo history — multi-step,
  no git required.
- `415de4e` **docs(readme)**: tightened for readers outside the project.
- `86c0b89`, `81ad6d8`, `177dc3d`, `ed4b2f4` **feat**: real-time token and cost
  tracking; `/model` picker with live discovery and a per-provider catalog.
- `799c3e2` **docs**: MCP covered in both directions.
- `45b80d0` **feat(cli)**: owl mascot startup animation.
- `e14a093` **M5**: revio as an MCP *server* — other agents can call it.
- `87bd6b0`, `2337e3c`, `ac80f59` **docs**: architecture, provider-impact
  analysis, LangGraph rationale, demo script.

### 2026-05-23 — Core build-out (M1–M4)

- `3125524` **M4**: `dedup --fix` — real patch application.
- `9ef40e6` **M4**: PLC profile implementation — 7 vendor parsers, 30+ PLCopen
  rules, ~6k LOC ported from the predecessor project.
- `f0b2afa` **M3++**: Java / Go / C++ Layer 2 plus nine LLM-only fallback
  profiles.
- `4537b6f` **M3+**: 18-language Tree-sitter coverage; Python and Rust Layer 2.
- `475105f` **M3**: findings history, mode diff, end-to-end tests.
- `054ccc5` **M3**: Skills (Anthropic Agent Skills spec) and the MCP
  client/bridge.
- `900bca6` **M3**: RAG over coding guidelines via ChromaDB.
- `093c397` **M1+M2**: project skeleton, agent graph, JS Layer 1/2 — the first
  commit.
