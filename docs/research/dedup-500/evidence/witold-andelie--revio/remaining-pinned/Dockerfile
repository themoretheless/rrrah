# revio — agentic code-review CLI
#
# Two stages so the C toolchain needed to build a few wheels never ships:
# `builder` produces a self-contained venv at /opt/revio, `runtime` copies it.
#
# Build:
#   docker build -t revio:latest .
#
# Run against a repo on the host (read-write, so `--fix` can apply patches):
#   docker run --rm -it -v "$PWD:/work" \
#     -e REVIO_LLM__PROVIDER=openai_compat \
#     -e REVIO_LLM__API_URL=https://api.deepseek.com \
#     -e REVIO_LLM__API_KEY=$DEEPSEEK_API_KEY \
#     -e REVIO_LLM__MODEL=deepseek-chat \
#     revio:latest audit .
#
# For a fully offline stack (revio + Ollama), use docker-compose.yml instead.

# --- Stage 1: build the venv -------------------------------------------------

FROM python:3.11-slim-bookworm AS builder

# Which optional extras to install. `rag` is excluded by default — it pulls
# torch + sentence-transformers (~1 GB) and only affects `revio guidelines`.
# Opt in with: docker build --build-arg REVIO_EXTRAS=js,plc,python,languages,rag .
ARG REVIO_EXTRAS="js,plc,python,languages"

ENV PIP_NO_CACHE_DIR=1 \
    PIP_DISABLE_PIP_VERSION_CHECK=1

# Some tree-sitter grammars have no wheel for every platform and fall back to
# building from source; this stage is discarded, so the cost is build-time only.
RUN apt-get update \
 && apt-get install -y --no-install-recommends build-essential git \
 && rm -rf /var/lib/apt/lists/*

RUN python -m venv /opt/revio
ENV PATH="/opt/revio/bin:$PATH"

WORKDIR /src
COPY pyproject.toml README.md LICENSE NOTICE ./
COPY src/ ./src/

RUN pip install --upgrade pip setuptools wheel \
 && pip install ".[${REVIO_EXTRAS}]" \
 # Layer-2 analyzers that are plain Python packages. bandit arrives with the
 # `python` extra; sqlfluff has no extra of its own.
 && pip install sqlfluff

# --- Stage 2: runtime --------------------------------------------------------

FROM python:3.11-slim-bookworm AS runtime

LABEL org.opencontainers.image.title="revio" \
      org.opencontainers.image.description="Agentic code review CLI — diff review, full-repo audit, AI-redundancy dedup" \
      org.opencontainers.image.licenses="Apache-2.0" \
      org.opencontainers.image.source="https://github.com/witold-andelie/revio"

# Runtime dependencies:
#   git                    — gitpython reads the repo under review
#   ca-certificates, curl  — TLS to the LLM endpoint; fetching oxlint below
#   cppcheck / shellcheck  — Layer-2 analyzers available as Debian packages
#
# The other analyzers (clippy, spotbugs, golangci-lint, luacheck, rubocop,
# phpstan, detekt, verilator) each need a full language toolchain and would
# multiply the image size, so they are left out. `revio analyzers list` inside
# the container shows what is present; add more in your own derived image.
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      git ca-certificates curl cppcheck shellcheck \
 && rm -rf /var/lib/apt/lists/*

# oxlint (JS/TS) as a standalone binary rather than `npm i -g oxlint`: the npm
# package is a node native addon, so it would drag Debian's whole node stack
# (600+ packages, ~400 MB) into the image for one 6 MB linter.
#
# Bump with: docker build --build-arg OXLINT_VERSION=1.76.0 .
ARG OXLINT_VERSION=1.75.0
ARG TARGETARCH
RUN set -eu; \
    case "${TARGETARCH:-amd64}" in \
      amd64) ox_arch=x86_64 ;; \
      arm64) ox_arch=aarch64 ;; \
      *)     ox_arch="" ;; \
    esac; \
    url="https://github.com/oxc-project/oxc/releases/download/apps_v${OXLINT_VERSION}/oxlint-${ox_arch}-unknown-linux-gnu.tar.gz"; \
    # A missing analyzer is a documented, graceful degradation (revio falls
    # back to AST + LLM), so an unsupported arch or a network blip warns
    # instead of failing the build.
    if [ -n "$ox_arch" ] && curl -fsSL -o /tmp/oxlint.tgz "$url"; then \
      tar -xzf /tmp/oxlint.tgz -C /tmp; \
      install -m 0755 "/tmp/oxlint-${ox_arch}-unknown-linux-gnu" /usr/local/bin/oxlint; \
      rm -f /tmp/oxlint.tgz "/tmp/oxlint-${ox_arch}-unknown-linux-gnu"; \
      oxlint --version; \
    else \
      echo "WARNING: no oxlint build for '${TARGETARCH:-amd64}' — JS/TS will fall back to AST + LLM"; \
    fi

COPY --from=builder /opt/revio /opt/revio
ENV PATH="/opt/revio/bin:$PATH" \
    PYTHONUNBUFFERED=1 \
    PYTHONDONTWRITEBYTECODE=1

# A bind-mounted repo is owned by the host user, whose uid rarely matches the
# container's. Without this, every git call fails with "detected dubious
# ownership" and review/audit die before reading a single file. Safe here
# because the container's only job is to inspect whatever you mounted.
RUN git config --system --add safe.directory '*'

# Non-root by default. --fix writes to the mounted repo, so pass
# `--user $(id -u):$(id -g)` if you need the new files owned by you.
RUN useradd --create-home --shell /bin/bash revio \
 && mkdir -p /home/revio/.config/revio /home/revio/.cache/revio /work \
 && chown -R revio:revio /home/revio /work
USER revio

WORKDIR /work

# Config comes from REVIO_* env vars (see README → Docker), or from a
# config.toml bind-mounted at /home/revio/.config/revio/config.toml.
ENTRYPOINT ["revio"]
CMD ["--help"]
