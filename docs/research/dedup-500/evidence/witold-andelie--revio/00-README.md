# revio

**Agentic code review CLI** that combines LangGraph-orchestrated LLM
reasoning with **13 deterministic static analyzers** + 30+ PLC rules
across **23 language profiles**.

```
$ revio review --commit HEAD
🔍 auth.js ──────────────────────────────────────────────
  → read_file(auth.js:42)
  💭 Line 42 builds SQL via template literal. Where does user_id come from?
  → get_call_sites(getUserById)
  💭 Called from POST /user/:id with req.params.id
  ⚠️ CRITICAL  SQL injection at auth.js:42  (confidence 0.95)
     Evidence: req.params.id → getUserById → `WHERE id = ${id}`
     Counter-considered: ORM auto-escape — ruled out, raw query path
```

## Demo

**▶ [Watch the 2–3 min product walkthrough](https://drive.google.com/drive/folders/1rmjUsD4hptc3YdRm3jLFcCsWvyyV7tR_?usp=drive_link)**
— install, a real `review` on a live repo, `--fix` applying a patch, and the
natural-language REPL.

## Features

| | What |
|---|---|
| **3 modes** | `review` (diff) · `audit` (full repo) · `dedup` (find AI redundancy) |
| **Targeted review** | Scan a **single file** by path, or **paste code straight into the chat** — reviewed in place, no repo or diff needed |
| **23 languages** | JS/TS · Python · Rust · Java · Go · C/C++ · Shell · Lua · SQL · Ruby · PHP · Kotlin · **Verilog/SystemVerilog** · 4 generic · PLC · 8 LLM-only |
| **13 static analyzers** | oxlint · bandit · clippy · spotbugs · golangci-lint · cppcheck · shellcheck · luacheck · sqlfluff · rubocop · phpstan · detekt · **verilator** |
| **Local / self-hosted LLM** | Point at any Ollama / vLLM / private endpoint — code never leaves the machine. **Free + air-gap + FERPA/GDPR-safe.** |
| **PLC support** | 7 vendor parsers · 30+ PLCopen rules · HW audit · LD/FBD/SFC → ST |
| **RAG** | Index your company's coding guidelines, cited inline in findings |
| **Skills** | Anthropic Agent Skills spec, dual-layer (project + user) |
| **MCP** | Client + server — connects to your tools, exposes its own |
| **`--fix` + undo** | Add `--fix` to *any* mode (or `/fix` in the REPL) to apply the agent's patches; every session is snapshotted, so `revio fix undo` reverts it |
| **Cross-session memory** | "🆕 New since last run: 3" — SQLite-backed, **auto-pruned** (count-based caps, oldest dropped — never grows unbounded) |
| **Multi-LLM** | Anthropic · DeepSeek · **Mistral** (EU-sovereign) · OpenAI · OpenRouter · any OpenAI-compatible endpoint |
| **Multilingual REPL** | Any human language (en / 中 / de / es / 日 ...) → English findings |
| **Natural-language control** | Plain-language commands run reviews *and* change settings (model / endpoint / key / budget …); out-of-scope asks are flagged at the capability boundary |

---

## Install

Three routes: the **one-click installer** below, a **manual venv** install, or
**[Docker](#docker)** if you'd rather not put Python and a dozen analyzers on
your machine.

### One-click

**macOS / Linux** — copy-paste into a terminal:
```bash
curl -sSL https://raw.githubusercontent.com/witold-andelie/revio/main/scripts/install.sh | bash
```

**Windows** — copy-paste into PowerShell:
```powershell
iwr https://raw.githubusercontent.com/witold-andelie/revio/main/scripts/install.ps1 | iex
```

The installer walks 7 stages, **each with progress visible**, and asks before
doing anything that takes more than a few seconds:

1. Verifies Python ≥ 3.11 (offers to install via `winget` on Windows if missing)
2. Verifies git
3. **Asks where to install** — defaults to `~/.local/share/revio` (macOS / Linux) or `%LOCALAPPDATA%\revio` (Windows); if you're on a different drive than the default (e.g. you `cd D:\tools` first on Windows), the prompt offers to install there instead
4. Clones the repo (with `git --progress`)
5. Installs `revio` core + Tree-sitter grammars (~150 MB)
6. **Asks** which optional pieces to install:
   - **RAG support** (~1 GB: torch + sentence-transformers) — opt-in; skip if you don't index company guidelines
   - **Per-language static analyzers** — `[A]` all / `[C]` pick per language / `[N]` none. *Selecting the languages you actually use significantly improves revio's accuracy on those languages.*
7. Adds a `revio` launcher to `~/.local/bin` (macOS / Linux) or `%LOCALAPPDATA%\revio\bin` on PATH (Windows)

After install, **open a new terminal** so the updated PATH loads, then `cd`
into any code folder and run `revio` — works just like the `claude` command.

Re-run the same install command anytime to update to the latest `main`.

### Disk footprint

| What | Size |
|---|---|
| Core (agent runtime + CLI + Tree-sitter grammars + 23 profiles) | **~150 MB** |
| + RAG (chromadb + sentence-transformers + torch) | +~1 GB |
| + HuggingFace embedding model (first RAG use) | +~80 MB |
| Per-language analyzer binaries (oxlint / cppcheck / shellcheck / etc.) | ~1-30 MB each |

A typical install with RAG off and only the user's daily languages
selected sits around **180-250 MB**. With everything on it's ~1.5 GB.

### Uninstall

```bash
# macOS / Linux
curl -sSL https://raw.githubusercontent.com/witold-andelie/revio/main/scripts/uninstall.sh | bash

# Windows
iwr https://raw.githubusercontent.com/witold-andelie/revio/main/scripts/uninstall.ps1 | iex
```

The uninstaller asks separately whether to also remove your cache
(`~/.cache/revio` — fix history, findings DB, RAG index) and config
(`~/.config/revio` — `config.toml` + custom skills). System-wide analyzers
installed via brew/winget/scoop are NOT touched — they may be useful to
other tools.

### Manual install (any OS)

```bash
git clone https://github.com/witold-andelie/revio.git
cd revio
python3 -m venv .venv
# macOS / Linux:
.venv/bin/pip install -e ".[js,plc,python,languages]"
# Windows PowerShell:
.venv\Scripts\pip install -e ".[js,plc,python,languages]"
```

Or straight from GitHub without cloning:
```bash
pip install "git+https://github.com/witold-andelie/revio.git#egg=revio[js,plc,python,languages]"
```

### Optional static analyzers (per language)

| Language | Tool | macOS / Linux | Windows |
|---|---|---|---|
| JS / TS | oxlint | `brew install oxlint` / `npm i -g oxlint` | `npm i -g oxlint` |
| Python | bandit | (already in `[python]` extra) | (already in `[python]` extra) |
| C / C++ | cppcheck | `brew install cppcheck` / `apt install cppcheck` | `winget install Cppcheck.Cppcheck` |
| Go | golangci-lint | `brew install golangci-lint` | `winget install golangci-lint.golangci-lint` |
| Rust | clippy | `rustup component add clippy` | `rustup component add clippy` |
| Java | spotbugs | `brew install spotbugs` (needs JDK) | download from spotbugs.github.io |
| Shell | shellcheck | `brew install shellcheck` / `apt install shellcheck` | `winget install koalaman.shellcheck` |
| Lua | luacheck | `brew install luacheck` / `luarocks install luacheck` | `scoop install luacheck` |
| SQL | sqlfluff | (auto-installed into revio's venv) | (auto-installed into revio's venv) |
| Ruby | rubocop | `gem install rubocop` | `gem install rubocop` (needs Ruby) |
| PHP | phpstan | `composer global require phpstan/phpstan` | (same; needs PHP + Composer) |
| Kotlin | detekt | `brew install detekt` (needs JDK) | download `detekt-cli` from GitHub |
| Verilog / SystemVerilog | verilator | `brew install verilator` / `apt install verilator` | `scoop install verilator` |
| PLC (Structured Text) | built-in rules | (already in `[plc]` extra — nothing to install) | (already in `[plc]` extra — nothing to install) |

**Missing analyzers don't break anything** — revio detects what's installed and falls back to AST + LLM reasoning for the rest.

---

## Docker

No Python, no venv, no per-language analyzer installs — the image ships
`git`, `oxlint`, `bandit`, `cppcheck`, `shellcheck` and `sqlfluff` ready to
run. Mount the repo you want reviewed at `/work`.

### Option A — `docker run` with a cloud model

```bash
git clone https://github.com/witold-andelie/revio.git && cd revio
docker build -t revio:latest .

# then, from any repo on your machine:
docker run --rm -it -v "$PWD:/work" \
  -e REVIO_LLM__PROVIDER=openai_compat \
  -e REVIO_LLM__API_URL=https://api.deepseek.com \
  -e REVIO_LLM__API_KEY="$DEEPSEEK_API_KEY" \
  -e REVIO_LLM__MODEL=deepseek-chat \
  revio:latest audit .
```

Swap the four env vars for any provider — `https://api.mistral.ai/v1` +
`codestral-latest`, `https://api.anthropic.com` + `provider=anthropic`, and so
on. Omit the command (`audit .`) to land in the interactive REPL.

### Option B — `docker compose` with a local model (nothing leaves the box)

`docker-compose.yml` brings up Ollama alongside revio, wired together over the
compose network:

```bash
docker compose run --rm revio audit .              # full-repo audit
docker compose run --rm revio review --commit HEAD # review the last commit
docker compose run --rm revio                      # interactive REPL
```

The first run builds the image and pulls the model (a few GB, once). After
that the stack is fully offline — no API key, no egress, nothing billed.

Two knobs, both via a `.env` file next to `docker-compose.yml`:

```dotenv
REVIO_MODEL=qwen2.5:7b   # any tool-calling Ollama model
REVIO_TARGET=.           # host path of the repo to review
```

Stop and clean up with `docker compose down` (add `-v` to also delete the
downloaded model).

### Notes

| | |
|---|---|
| **Config** | The image has no `config.toml`. Everything comes from `REVIO_*` env vars — see [Environment variables](#environment-variables). Bind-mount a file at `/home/revio/.config/revio/config.toml` if you prefer. |
| **`--fix` writes to your repo** | The mount is read-write on purpose. Files land owned by the container's user; pass `--user "$(id -u):$(id -g)"` (or uncomment the `user:` line in the compose file) if that matters. |
| **`revio fix undo` across runs** | Compose keeps the undo history in a named volume, so it survives `--rm`. With plain `docker run`, add `-v revio-cache:/home/revio/.cache/revio`. |
| **git ownership** | Handled — the image sets `safe.directory '*'`, so a repo owned by your host user doesn't trip git's dubious-ownership check. |
| **Image size** | ~860 MB. The `rag` extra (torch + sentence-transformers, ~1 GB more) is left out; add it with `docker build --build-arg REVIO_EXTRAS=js,plc,python,languages,rag .` |
| **Fewer analyzers than a host install** | Only the ones that don't need a whole language toolchain are baked in. `docker run --rm revio:latest analyzers list` shows what's present. |
| **MCP servers** | stdio servers spawn inside the container, so their command must exist in the image (`uvx`/`npx` don't). SSE/HTTP servers work as-is. See [MCP](#mcp--both-directions). |

---

## First run

```bash
.venv/bin/revio
```

Triggers a 7-step wizard: pick **provider** → **API URL** → **key** →
**model** → **thinking mode** → **default profile** → **connection test**.
The final step makes one tiny API call to verify credentials, then
saves the result to `~/.config/revio/config.toml`.

Provider list includes **Local model (Ollama / LM Studio / vLLM)** — it
pre-fills `http://localhost:11434/v1` and lets you leave the API key
empty. See [Local / self-hosted LLM](#local--self-hosted-llm-zero-data-leaves-the-box).

---

## The 3 modes

### `review` — diff / commit

```bash
revio review                           # latest commit
revio review --commit abc1234
revio review --format markdown -o review.md
```

Tight tool budget. Focus: security + correctness in the changed lines.

### `audit` — full-repo scan

```bash
revio audit
revio audit --profile python --budget 30
```

Heavy on Layer 2 (static analyzers run first). LLM adds semantic context
on top.

### `dedup` — find AI-generated redundancy

```bash
revio dedup
```

Detects duplicate functions, single-use wrappers, dead code, repeated
templates — the LLM-generated patterns.

---

## `--fix` — apply what the agent found

`--fix` is **not a fourth mode**. It's a stage any mode can end with: the
agent proposes patches as it works, and `--fix` walks you through applying
them. Same five flags on all three commands.

```bash
revio audit --fix                      # interactive: review & apply each patch
revio review --fix --dry-run           # preview patches, don't write
revio dedup --fix --yes                # auto-apply high-confidence (CI)
revio audit --fix --min-confidence 0.8 # lower the --yes auto-approve bar
revio dedup --fix --allow-dirty        # allow a dirty repo (stashes first)
```

In the REPL, `/fix` applies whatever the last run proposed — no re-run, no
extra LLM call:

```
> audit this repo
  … 4 findings, 2 with patches
> /fix                                 # or: /fix dry-run
```

How much you get differs by mode, on purpose:

| Mode | Patch behaviour |
|---|---|
| `dedup` | A patch for **every** finding — its output is mechanical deletion |
| `audit` / `review` | Patches **only** where the fix is mechanical (missing null check, hardcoded credential, unparameterized SQL). Design-level findings are reported, not patched |

Forcing a patch for "redesign this auth flow" would produce confident
nonsense, so the prompts deliberately don't ask for one. A finding without
a patch is a complete result outside `dedup`.

`--fix` refuses to start on a dirty repo unless `--allow-dirty` (which
stashes first).

**Undo any past fix** — every `--fix` session snapshots the affected
files before writing, so you can roll back regardless of git state:

```bash
revio fix history                       # list past sessions
revio fix undo                          # restore from most recent session
revio fix undo 2026-05-24T10-15-32.123_a3f9  # restore a specific one
revio fix show <session_id>             # preview what would be reverted
revio fix clean --older-than-days 14    # purge old snapshots
```

History caps default to 50 sessions / 30 days / 1 MiB per file — tweak
in `~/.config/revio/config.toml` under `[fix_history]`. Works without
git; the git stash path remains as additional safety when present.

---

## Configuration

```bash
revio config show / edit / path / init
```

Or directly in `~/.config/revio/config.toml`. Examples for the major
providers:

```toml
# DeepSeek (cheapest cloud option; default in our wizard)
[llm]
provider = "openai_compat"
api_url = "https://api.deepseek.com"
api_key = "<YOUR_DEEPSEEK_API_KEY>"
model = "deepseek-v4-pro"

# Mistral (EU-sovereign — recommended for European customers)
# [llm]
# provider = "openai_compat"
# api_url = "https://api.mistral.ai/v1"
# api_key = "<YOUR_MISTRAL_API_KEY>"
# model = "codestral-latest"           # code-specialized 22B, perfect for revio

# Anthropic native
# [llm]
# provider = "anthropic"
# api_url = "https://api.anthropic.com"
# api_key = "<YOUR_ANTHROPIC_API_KEY>"
# model = "claude-sonnet-4-6"
```

### Environment variables

Any config field can be set with a `REVIO_<SECTION>__<FIELD>` variable
(section and field separated by a **double** underscore). These sit at the top
of the layering order — they beat every config file:

```bash
export REVIO_LLM__MODEL=codestral-latest
export REVIO_LLM__API_URL=https://api.mistral.ai/v1
export REVIO_AGENT__MAX_TOOL_CALLS=30
export REVIO_OUTPUT__DEFAULT_FORMAT=json
```

Nesting keeps going, so an MCP server is
`REVIO_MCP__SERVERS__JIRA__COMMAND=jira-mcp`. Values are plain strings and get
coerced to the right type on load. `REVIO_`-prefixed variables that don't name
a real section are ignored rather than raising — the environment is shared with
everything else on the machine, and a stray variable shouldn't stop a review.

This is the layer that makes [Docker](#docker) and CI work: set `[llm]` here
and revio skips the first-run wizard entirely, so it never blocks on a prompt
in a container or a pipeline. `revio config show` prints the merged result.

The full order, later winning: built-in defaults →
`~/.config/revio/config.toml` → `./.revio.toml` → `./.revio.local.toml` →
environment.

### Responses API (opt-in)

For OpenAI-compatible endpoints that support it (OpenAI, Azure, GPT-5.x / Codex
proxies), revio can route via OpenAI's **Responses API** (`/v1/responses`)
instead of chat/completions — better prompt caching and server-side reasoning
persistence across tool calls on reasoning models. Off by default; toggle with
`/responses on` in the REPL, or set `use_responses_api = true` under `[llm]`.
Most providers (DeepSeek, local models, …) only speak chat/completions — leave
it off for those.

### Memory / disk caps

revio's on-disk memory (cross-session findings history, per-repo agent
checkpoints, REPL command history) is **auto-pruned, count-based** — when a
store exceeds its cap the oldest entries are dropped, so nothing grows without
bound. Defaults are generous; tune them under `[memory]`:

```toml
[memory]
findings_max_rows        = 5000   # findings remembered per repo
checkpoint_max_runs      = 50     # past runs kept per repo checkpoint DB
repl_history_max_entries = 1000   # REPL commands kept in history
```

`fix undo` history is separately capped by `[fix_history] max_sessions`
(also count-based). All caches live under `~/.cache/revio/` and can be wiped
by hand at any time.

For per-project overrides, drop a `.revio.toml` in the repo root —
shadows the user-global config and is meant to be committed.

### Switching LLM model / endpoint / key after install

Three paths, easiest first:

| Goal | Command (in the REPL) |
|---|---|
| Browse and pick a model (auto-discovers `/v1/models`) | `/model` |
| List models without picking | `/model list` (or `/models`) |
| Set model directly | `/model deepseek-v4-pro` |
| Change endpoint URL | `/url https://api.mistral.ai/v1` |
| Rotate API key (masked input) | `/key` |
| Open the full config file in `$EDITOR` | `/config` |
| Re-run the 7-step wizard | `revio config init` |

The `/model` picker hits `GET /v1/models` on the current endpoint at
runtime, so for any new provider (Mistral, a new Xiaomi API, your
in-house vLLM, etc.) you see the model catalog **the provider is
actually serving right now** — no need to know model IDs by heart.

### Adding more static analyzers after install

The installer asked you to pick analyzers by letter code. To add more
later **without re-running the installer**:

```bash
revio analyzers              # status table — what's installed vs missing
revio analyzers install jcs  # install JS + C/C++ + Shell (same letter codes)
revio analyzers install '*'  # install ALL remaining
revio analyzers menu         # interactive picker
```

revio detects your OS and uses the right package manager (brew on macOS,
apt on Linux, winget / scoop on Windows). sqlfluff is pip-installed
into revio's own venv. Letter codes are the same as the installer's:

  `j` JS · `c` C/C++ · `g` Go · `r` Rust · `a` Java · `s` Shell ·
  `l` Lua · `q` SQL · `v` Verilog · `u` Ruby · `h` PHP · `k` Kotlin

---

## Local / self-hosted LLM (zero data leaves the box)

revio's `openai_compat` provider works with **any OpenAI-compatible
endpoint** — that's the dominant API standard for self-hosted runtimes.
revio doesn't care whether the model is a 4 GB quantized Qwen on a
laptop, a 70 B Llama on a workstation, **or a full-power 671 B
DeepSeek-V3 / 405 B Llama-3.1 / 489 B Qwen-Max running on a bank's
own GPU cluster**. The same provider config drives all of them.

Compatible runtimes (non-exhaustive): **Ollama · vLLM · SGLang ·
llama.cpp server · LM Studio · LocalAI · TGI (HuggingFace Text
Generation Inference) · OpenLLM · Triton Inference Server**. If it
exposes `/v1/chat/completions`, revio works against it.

Open-weight model families that work well with revio (verified):
**Mistral / Mixtral / Codestral** (EU-sovereign, recommended in Europe) ·
**Qwen 2.5 / Qwen 3** (multilingual, esp. strong Chinese) · **Llama 3.1 / 3.3** ·
**DeepSeek-V3 / R1** · **Gemma** · **Phi-4** · **GPT-OSS**. Any model with
function-calling support — the agent loop uses tool calls.

### Why this matters

| Constraint | Why local LLM is the only answer |
|---|---|
| Student / patient / financial code review | FERPA / HIPAA / GDPR / SOX often forbid sending code to a US API |
| **Banks / insurance / law firms** | Internal IP + regulator audit trails — code must stay inside the firewall, often with full-size models running on private GPU clusters |
| **Government / defense / aerospace** | Air-gapped by policy; both `plc` and `verilog` profiles are valuable here |
| **EU AI sovereignty** | French / German / Italian / Czech / Polish customers can run **Mistral** (EU-headquartered, open-weight) or **Mixtral** locally — GDPR-compliant by construction |
| **National AI sovereignty** | Other jurisdictions with similar mandates (China, Russia, etc.) |
| Cost at scale | A CS department doing 5 000 audits / semester pays $0 instead of $50-500 |
| Vendor independence | No provider rug-pull / pricing-tier change breaks your CI |

### Quickstart — Ollama in 4 commands

```bash
# 1. install Ollama            → https://ollama.com/download
# 2. pull a tool-calling model (7B ≈ 4.7 GB)
ollama pull qwen2.5:7b

# 3. serve it with a context window big enough for an agent loop (see Gotchas)
OLLAMA_CONTEXT_LENGTH=32768 ollama serve

# 4. point revio at it — pick "Local model (Ollama / LM Studio / vLLM)"
#    and press Enter through the API-key prompt
revio config init
```

Then `revio audit .`. Nothing leaves the machine after step 2.

Prefer to skip the wizard? The equivalent config is:

```toml
# ~/.config/revio/config.toml
[llm]
provider = "openai_compat"
api_url = "http://localhost:11434/v1"
api_key = ""                 # local servers don't check it
model = "qwen2.5:7b"
disable_thinking = true
```

### Gotchas

These five cover essentially every "it connects but produces nothing" report.

| Gotcha | What to do |
|---|---|
| **`/v1` suffix is required** | `api_url` must be `http://localhost:11434/v1`, not `:11434`. Model discovery tolerates the bare host; chat calls 404. The wizard and `/url` auto-append it when the bare host fails |
| **API key** | Not needed. Leave it empty — revio sends a placeholder token, which keyless servers ignore. (vLLM started with `--api-key` does check it; put that value here) |
| **Model must support function calling** | The agent loop is entirely tool calls. A model without tool support connects fine, answers in prose, and reports **zero findings**. Verified working: `qwen2.5` · `llama3.1` · `mistral` · `codestral` · `mixtral` |
| **Ollama's default context is 4096 tokens** | System prompt + tool schemas + tool output blow past that immediately, and the overflow is **silently truncated** — you get plausible-looking garbage. Set `OLLAMA_CONTEXT_LENGTH=32768` on the **server** before `ollama serve`. It cannot be set per-request: `num_ctx` isn't part of the OpenAI-compatible request body |
| **First response is slow** | The model cold-loads on the first call. Try `--budget 5` for the first run to confirm the wiring before a full audit |

### One-time setup (any other local server)

```toml
# ~/.config/revio/config.toml
[llm]
provider = "openai_compat"
api_url = "http://<host>:<port>/v1"   # whatever your local server exposes
api_key = ""                           # or your internal token, if any
model = "<whatever-model-id-the-endpoint-serves>"
```

Then `revio audit .` — fully offline from this point on. The setup is
**model-agnostic**: replace the `api_url` + `model` line and you're
talking to a different deployment. Examples:

| Deployment | `api_url` | `model` |
|---|---|---|
| Ollama on your laptop | `http://localhost:11434/v1` | e.g. `qwen2.5:7b`, `llama3.1:8b`, `mistral:7b`, `codestral:22b` |
| **Mistral cloud (EU-sovereign)** | `https://api.mistral.ai/v1` | `codestral-latest`, `mistral-large-latest`, `mistral-small-latest` |
| vLLM behind nginx in your DC | `https://llm.internal/v1` | e.g. `mistral-large-2`, `mixtral-8x22b`, `deepseek-v3-671b`, `llama-3.1-405b` |
| Bank's on-prem cluster (full-size frontier model) | `https://gpu-cluster.bank.local/v1` | whatever the cluster team registers |
| Air-gapped lab box | `http://192.168.x.x:8000/v1` | whatever's loaded |

`/model` REPL command **auto-discovers** whatever models the endpoint
serves by hitting `GET /v1/models`. No separate config for each model.

`/cost` shows token counts but **silently omits the `$` figure** for
local models (no cost to misrepresent).

### Hybrid setup

Embeddings used by RAG (`all-MiniLM-L6-v2`, ~80 MB) always run locally
in revio's own process — your indexed guidelines never go through any
API. You can combine local embeddings with a cloud LLM, or go fully
local. Mix and match per `.revio.toml`.

### What works · what's identical · what scales with hardware

| Feature | Local LLM (any size) | Cloud LLM |
|---|---|---|
| Agent loop · tools · streaming · MCP · `--fix` | ✅ identical | ✅ |
| All 13 Layer-2 static analyzers | ✅ identical (run as subprocesses) | ✅ |
| RAG (embeddings) | ✅ always local | ✅ embeddings local; LLM cloud |
| Per-call latency | hardware-bound (laptop: slow · 8×H100: fast) | network-bound |
| Finding quality on tricky semantic cases | scales with model size — a frontier 671 B on local hardware ≈ frontier cloud | typically high |
| Cost per audit | **$0** (you already paid for the GPUs) | $0.01-$0.30 typical |

**The point**: customers with budget for a serious local deployment
(banks, defense primes, large universities, telcos) get the **same**
quality as cloud — with full data sovereignty. Customers without that
budget can run a 7-8 B model on a developer laptop and still get
useful static-analyzer coverage + LLM reasoning. **Same product, both
extremes of the spectrum.**

---

## Guidelines (RAG)

Index your team's coding standards so findings cite them directly:

```bash
revio guidelines add docs/styleguide.md team-policies/
revio guidelines search "SQL injection prevention"
revio guidelines list / clear / reindex
```

Supported: `.md` `.txt` `.rst` `.adoc` `.pdf` `.docx`. Index lives at
`~/.cache/revio/<repo-hash>/vectorstore/` (per-repo).

During a review:
```
⚠️ CRITICAL  SQL injection
  Evidence:
    · read_file showed: query = f"SELECT * FROM users WHERE id = {id}"
    · search_guidelines → security_checklist.md / SQL Injection: ...
```

---

## Skills

Markdown files with YAML frontmatter that teach the agent how to handle
specific scenarios. Dual layer:

- `.revio/skills/<name>/SKILL.md` (project, committable)
- `~/.config/revio/skills/<name>/SKILL.md` (user-global)

```bash
revio skills list / show <name> / activated
```

Skills with matching `extensions`, `imports`, or `filename_patterns`
auto-fire; others stay catalog-only until the LLM pulls them via
`load_skill(name)`.

---

## MCP — both directions

revio speaks MCP as a **client** (it consumes your servers) and as a
**server** (other agents consume it). The two are independent — set up either
one without the other.

### As a client — consume your existing servers

Add one `[mcp.servers.<name>]` table per server to any config file revio reads
— `~/.config/revio/config.toml` for your personal servers, `./.revio.toml`
when the whole team should get them. Both transports are supported:

```toml
# stdio — revio spawns the process and talks over its stdin/stdout
[mcp.servers.jira]
command = "uvx"
args = ["mcp-server-atlassian-jira"]
env = { ATLASSIAN_TOKEN = "$ATLASSIAN_TOKEN" }   # leading $ → read from your environment

# SSE / HTTP — revio connects to a server that is already running
[mcp.servers.internal]
url = "https://mcp.corp.example.com/sse"
api_key_env = "CORP_MCP_TOKEN"   # sent as: Authorization: Bearer <value of that var>
timeout = 10.0                   # seconds to connect, default 5
enabled = true                   # false disables the server without deleting the table
```

| Field | Transport | Meaning |
|---|---|---|
| `command` + `args` | stdio | The process revio spawns. Must be on `PATH`. |
| `env` | stdio | Extra environment for that process, on top of your own. A value of `"$FOO"` is substituted with your `$FOO` — put the secret in your shell, not in the config file. |
| `url` | SSE/HTTP | Endpoint of a running server. Presence of `url` selects this transport. |
| `api_key_env` | SSE/HTTP | *Name* of the env var holding the token. Sent as a bearer header; omitted entirely if the var is unset. |
| `timeout` | both | Connect timeout, 0.5–60s. |
| `enabled` | both | Default `true`. |

revio connects to all enabled servers in parallel at session start, wraps
their tools as `mcp_<server>_<tool>`, and merges them into the agent's
toolkit. Every run prints what it got, so you can confirm the wiring:

```
🔌 MCP servers
  ✓ jira → 14 tools
  ✗ internal (not connected)
  · internal: <the underlying connection error>
```

A server that fails is reported and skipped — the run continues with the
remaining tools, so a broken server never blocks a review.

> **In Docker:** a stdio server is spawned *inside the container*, so its
> command has to exist in the image. `uvx` and `npx` do not — either extend
> the image (`FROM revio:latest` plus your own install steps) or use an
> SSE/HTTP server, which needs nothing installed. An SSE server running on
> your host is not at `localhost` from inside the container: use
> `host.docker.internal` (on Linux, add
> `--add-host=host.docker.internal:host-gateway`).

### As a server — expose revio to other agents

```bash
revio mcp-server               # stdio MCP server
```

Tools exposed (19 total):

- **Full pipelines** (LLM-backed, 20-60s): `revio_audit`, `revio_review`,
  `revio_dedup`.
- **Per-analyzer Layer 2** (no LLM, ~1-3s; one tool per analyzer):
  `revio_run_bandit` · `revio_run_oxlint` · `revio_run_cppcheck` ·
  `revio_run_clippy` · `revio_run_spotbugs` · `revio_run_golangci_lint` ·
  `revio_run_shellcheck` · `revio_run_luacheck` · `revio_run_sqlfluff` ·
  `revio_run_rubocop` · `revio_run_phpstan` · `revio_run_detekt` ·
  `revio_run_verilator`.
- **Discovery / context** (instant): `revio_search_guidelines`,
  `revio_list_profiles`, `revio_detect_profile`.

Register with Claude Code — same JSON shape for Cursor, Windsurf, Zed and any
other MCP host:
```json
{ "mcpServers": { "revio": { "command": "revio", "args": ["mcp-server"] } } }
```

Then ask the host agent in plain language: *"use revio to audit this repo"*.
Every tool takes an **absolute** `repo_path` (or `path`), so the agent stays in
control of what gets analysed and the server never guesses a working directory.

Two things worth knowing before you wire it up:

- **The three pipeline tools need a configured LLM** — run `revio config init`
  once, or pass `REVIO_LLM__*` env vars in the host's server definition.
  Without one they return
  `{"error": "APIKeyMissingError: No API key configured…"}` instead of hanging,
  so a misconfigured host fails loudly and cheaply.
- **The 13 analyzer tools and the 3 discovery tools need no key at all** — they
  are pure static analysis. Registering revio purely as a fast lint backend for
  another agent is a valid setup with zero LLM cost.

`stdout` is the JSON-RPC channel and all logs go to `stderr`, so the server is
safe to run non-interactively; it never prompts, not even on first run with no
config file.

The server returns patch *operations* via `revio_dedup` but never
applies them — the host agent decides what to write.

---

## Interactive REPL

```bash
revio              # drop into REPL
```

```
> review the last 3 commits
> 检查这个项目里有没有重复代码         ← reply comes back in Chinese
> Vérifie src/auth.py pour des fuites  ← reply comes back in French
> check this file: src/auth.py         ← scans just that one file
> (paste a fenced code block + a note)  ← reviews the snippet inline, no file needed
> clean up the duplicate / junk code   ← runs dedup
> switch the model to claude-opus-4-7  ← changes a setting, no slash needed
> set my api key                       ← prompts securely (key never typed inline)
> what can you do?                     ← lists revio's capabilities
> /model deepseek-v4-pro
```

| Slash command | |
|---|---|
| `/help` `/?` | List all commands |
| `/model` | Interactive picker (live `/v1/models` + curated catalog) |
| `/model <name>` | Set model directly |
| `/models` | List available models on current endpoint |
| `/url` | Change the API endpoint interactively — auto-matches the protocol, re-prompts for the key, re-detects the models that endpoint serves, and runs a connectivity check that **auto-adds a missing `/v1`** if the bare host 404s (no config-file editing). Or `/url <url>` to set directly |
| `/key` `/config` | Update the API key (masked) / open the config file |
| `/responses on\|off` | Opt-in: route via OpenAI's Responses API (`/v1/responses`) for endpoints that support it (off by default) |
| `/profile <name>` | Switch profile |
| `/mode <name>` | Default mode for next NL input |
| `/fix` | Apply the patches the last run proposed (`/fix dry-run` to preview). Works after any mode |
| `/budget <n>` | Tool-call budget for this session |
| `/cost` | Real token usage + USD cost for the REPL session |
| `/clear` `/history` `/exit` | Standard |

Type `/` for the full menu, then keep typing to filter it live (`/u` → `/url`,
`/mo` → `/model` · `/models` · `/mode`) — the dropdown narrows as you type and
Enter/Tab completes the highlighted command.

Non-slash input is routed by an intent LLM (multilingual by design) into:

- **`review` / `audit` / `dedup`** — run the agent in that mode. "Clean up
  the junk / duplicate code" maps to `dedup`. The target can be the whole
  repo, **a single file** (give its path — it's scanned on its own), or
  **a code snippet you paste in** (wrap it in a ```` ``` ```` fence, or just
  paste obvious multi-line code — it's reviewed from a throwaway temp file,
  no repo needed).
- **`fix`** — "apply those fixes" / "应用刚才的补丁" applies what the previous
  run proposed, same as `/fix`. Only phrasings that clearly point *back* at
  results route here; "fix this bug in auth.py" still means *go look*, so it
  runs a review rather than replaying an older run's patches.
- **`config`** — change a setting in plain language ("switch the model to
  claude-opus-4-7", "set my api key", "budget 30", "use the endpoint
  https://api.mistral.ai/v1", "show my config", "how much did this cost").
  These are translated to the matching slash command and run through the same
  dispatcher, so anything the slash commands do is reachable by natural
  language too. **The API key is never read from the text you type** — that
  path always drops into the masked prompt; endpoint changes ask to confirm.
- **`capability`** — "what can you do?" prints the capability list.
- **`out_of_scope`** — requests beyond revio (write a feature, deploy, run a
  shell command, general questions) are **declined with a clear note that
  they're outside revio's capability boundary**, plus a reminder of what it
  *can* do — instead of silently launching a review.

There's a deterministic keyword fallback for when the intent LLM is
unreachable, so settings changes and the boundary message still work offline.

### Language: input vs output

revio splits language responsibility along a deliberate boundary:

| Layer | Language |
|---|---|
| **Wizard banner · UI labels · slash commands · install scripts · docs · examples** | **English always** (so screenshots / Stack Overflow / support tickets read the same in any locale) |
| **Your natural-language requests** | **Any language** — Chinese, German, French, Spanish, Czech, Japanese, ... |
| **Findings shown to you** (title, hypothesis, suggestion, counter-consideration, reflect summary, systemic observations, plan text) | **Same language as your request** — Chinese in → Chinese out |
| **Tool args** (`read_file("src/auth.py")`, regex patterns) and **evidence quotes** (verbatim tool output) | **English always** — for log-greppability and tool compatibility |

So you can ask `检查 src/auth.py 看有没有 SQL 注入` and get back finding
cards titled `SQL 注入：query 用 f-string 拼接 user_id`, with the
suggested fix in Chinese — but the agent's internal `read_file()` call
still uses the literal path `"src/auth.py"`.

### Visual rhythm — owl between tasks

The owl mascot that animates on REPL startup also plays a short (~1 s)
loop **after every NL-driven task finishes**. Visual separator so the
next prompt feels like a fresh task instead of a continuation. Skipped
on non-TTY (CI). Slash commands (`/help`, `/cost`, etc.) don't trigger
it — they're configuration, not "tasks".

---

## Profiles

| Profile | Layer 1 (AST) | Layer 2 (static) |
|---|---|---|
| `js` | Tree-sitter + symbol graph + call graph + dedup index | **oxlint** |
| `python` | Tree-sitter | **bandit** |
| `rust` | Tree-sitter | **clippy** |
| `java` | Tree-sitter | **spotbugs** (needs .class) |
| `go` | Tree-sitter | **golangci-lint** |
| `cpp` | Tree-sitter | **cppcheck** |
| `plc` | 7 vendor parsers + LD/FBD/SFC | **30+ PLCopen rules** + HW audit |
| `shell` | Tree-sitter | **shellcheck** |
| `lua` | Tree-sitter | **luacheck** |
| `sql` | Tree-sitter | **sqlfluff** (multi-dialect) |
| `ruby` | Tree-sitter | **rubocop** |
| `php` | Tree-sitter | **phpstan** |
| `kotlin` | Tree-sitter | **detekt** (needs JDK) |
| `verilog` | Tree-sitter | **verilator** (--lint-only) |
| `generic` | Tree-sitter (Scala / C# / Swift / Julia) | — |
| LLM-only | MATLAB · R · SAS · COBOL · Solidity · Zig · ObjC · Dart | — |

Default `auto` walks the repo, counts file extensions + marker files,
picks the best match. Override with `--profile <name>`.

---

## Token usage & cost

revio reads real `usage_metadata` off every LLM response. Per call:
```
  · tokens +1.2k in, +340 out  (Σ 8.4k / 1.9k · 85 tok/s · $0.011)
```

Session footer:
```
  session: 6/6 tool calls · 7 findings · 18.1s · deepseek-v4-pro
  tokens:  12.6k in · 573 out · 5 LLM call(s) · avg 32 tok/s · $0.0040
```

Pricing is fuzzy-matched (DeepSeek / Claude / OpenAI / Mistral / Ollama).
**For models we don't have pricing for (new providers, custom endpoints)
the `$` figure is silently omitted** — token counts and throughput still
show. We never display a misleading `$0.00`.

`/cost` in the REPL shows cumulative usage across the whole REPL session.

---

## Output formats

```bash
revio audit --format stream            # default (TTY)
revio audit --format json -o report.json
revio audit --format markdown -o report.md
```

JSON / markdown include `total_input_tokens`, `total_output_tokens`,
`llm_call_count`, `est_cost_usd` on the `ReviewReport`.

---

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Clean, or only info-severity |
| `1` | Operational error (no config, invalid path, ...) |
| `2` | At least one CRITICAL finding |
| `130` | Ctrl-C |

For CI: `revio audit . --format json -o report.json` and fail the build
on exit `2`.

---

## What's special about revio

| | Other LLM review tools | revio |
|---|---|---|
| Static-analysis backbone | One LLM call per file | 13 deterministic analyzers + LLM on top |
| Hallucinated findings | Common (LLM invents paths) | Grounding validator rejects un-read files |
| Token cost on a large repo | Re-reads whole files | Pulls only enclosing functions via Tree-sitter |
| Company-specific rules | Generic prompt only | RAG over your own guidelines |
| PLC / industrial control | Not supported | 7 vendor parsers + 30+ PLCopen + HW audit |
| LLM provider | Vendor-locked | Anthropic + any OpenAI-compat |
| Cross-session memory | Stateless | SQLite history — "🆕 New since last run" |
| Auto-fix | Text suggestion only | `--fix` actually edits files |
| Undo a fix | "Hope you committed before running" | `revio fix undo` — snapshot-based, multi-step, no git required |
| Self-hosted / offline | Cloud-only, your code goes to a US API | Any OpenAI-compatible endpoint — **Ollama, vLLM, on-prem GPU**; FERPA/HIPAA/GDPR-safe |

---

## Troubleshooting

**Wizard didn't run** → `rm ~/.config/revio/config.toml` then `revio`.

**Connection test fails** → DeepSeek / Mimo need `disable_thinking = true`. OpenAI-compat gateways often have their own model IDs.

**Findings dropped as "ungrounded"** → the validator rejected findings on files the agent never read. Run with `REVIO_DEBUG=1` to see the full tool-call trace.

**`--fix` says no patches** → the model didn't emit `propose_patch` calls. In `review` / `audit` that's often correct — those modes only patch mechanical fixes, and a design-level finding is reported without one. Otherwise: a strict gateway dropped the tool call, or the model judged no mechanical fix was safe (check `suggestion` in the finding).

**Local model connects but finds nothing** → almost always one of two things: the model doesn't support function calling, or Ollama's 4096-token default context silently truncated the prompt. See [Gotchas](#gotchas).

**Tree-sitter import errors** → `.venv/bin/pip install -e ".[languages]"`.

**Docker: `docker compose run` hangs on the first call** → Ollama is still
pulling the model. Watch it with `docker compose logs -f ollama-pull`.

**Docker: files created by `--fix` are owned by root** → re-run with
`--user "$(id -u):$(id -g)"`, or uncomment the `user:` line in
`docker-compose.yml`.

---

## Change history

Development ran **2026-05-23 → 2026-07-26**, 63 commits. What landed when
(counts are a snapshot — `git log` is always authoritative):

| Date | Commits | What shipped |
|---|---|---|
| **2026-05-23** | 8 | Milestones M1–M4: project skeleton and the LangGraph agent loop · JS Layer 1/2 · RAG over guidelines (ChromaDB) · Skills (Anthropic spec) + MCP client · findings history and mode diff · 18-language Tree-sitter coverage · Python/Rust/Java/Go/C++ Layer 2 · the PLC profile (7 vendor parsers, 30+ PLCopen rules) · `dedup --fix` applying real patches |
| **2026-05-24** | 31 | M5 and the productisation push: revio **as** an MCP server · live token + cost metering · `/model` picker with endpoint discovery · snapshot-based multi-step `fix undo` (no git needed) · 6 more static analyzers, then verilator (13 total) · Mistral as a first-class EU-sovereign provider · cross-platform one-click installer, then four follow-up commits fixing Windows install bugs · `revio analyzers` subcommand · output language follows input language |
| **2026-05-31** | 1 | Natural-language settings control in the REPL, with an explicit capability boundary for out-of-scope requests |
| **2026-06-01** | 4 | Accuracy pass: corrected thinking-mode sampling per provider · fixed the language matrix · all 13 analyzers exposed over MCP |
| **2026-06-25** | 5 | Relicensed MIT → **Apache-2.0** (+ `NOTICE`) · review a single file by path, or paste a snippet straight into the chat |
| **2026-06-27** | 5 | Count-based auto-pruning for every on-disk store · endpoint connectivity self-check with automatic `/v1` normalization · opt-in OpenAI Responses API mode · live slash-command filtering |
| **2026-07-26** | 9 | Dropped dead Layer-4 scaffolding · one PLC file no longer repaints the whole repo · **`--fix` promoted from a `dedup` flag to a stage any mode can end with** (+ REPL `/fix`, local-model wizard preset, working keyless Ollama path) · the `REVIO_*` env layer, which had been a TODO · **Docker + Compose deployment** · this changelog · full MCP client docs incl. the SSE/HTTP transport |

Commit-by-commit detail: **[CHANGELOG.md](CHANGELOG.md)** — every entry carries
its short hash, so `git show <hash>` gets you the diff. The canonical record is
the git log itself:

```bash
git log --oneline --reverse     # same sequence, straight from the repo
git log -p --follow src/revio/agent/graph.py   # history of one file
```

---

## License

Apache License 2.0 — see [LICENSE](LICENSE).
