FROM rustlang/rust:nightly-slim
RUN apt-get update && apt-get install -y --no-install-recommends build-essential cmake pkg-config libclang-dev libheif-dev libopenjp2-7-dev libjxr-dev liblcms2-dev zlib1g-dev fonts-dejavu-core && rm -rf /var/lib/apt/lists/*
