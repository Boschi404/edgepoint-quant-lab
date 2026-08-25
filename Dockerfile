# syntax=docker/dockerfile:1.7
FROM rust:1.80-bookworm AS dev

ARG USERNAME=dev
ARG USER_UID=1000
ARG USER_GID=1000

ENV DEBIAN_FRONTEND=noninteractive \
    CARGO_HOME=/usr/local/cargo \
    RUSTUP_HOME=/usr/local/rustup \
    RUST_BACKTRACE=1

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    git \
    make \
    pkg-config \
    libssl-dev \
    sqlite3 \
    libsqlite3-dev \
    protobuf-compiler \
    nodejs \
    npm \
    jq \
    bash-completion \
    && rm -rf /var/lib/apt/lists/*

RUN rustup component add rustfmt clippy

# Useful but non-essential tools. Kept in the image so a sterile container is productive immediately.
RUN cargo install cargo-nextest cargo-deny cargo-watch --locked || true

RUN groupadd --gid ${USER_GID} ${USERNAME} \
    && useradd --uid ${USER_UID} --gid ${USER_GID} -m ${USERNAME} \
    && mkdir -p /workspace \
    && chown -R ${USERNAME}:${USERNAME} /workspace /usr/local/cargo

WORKDIR /workspace
USER ${USERNAME}

CMD ["bash"]
