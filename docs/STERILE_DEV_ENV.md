# Sterile developer environment

This repository can be developed with Docker only. No host Rust installation is required.

## Recommended workflow

```bash
make dev
make shell
make doctor
make check
```

## Why this setup

- The host machine only needs Docker.
- Rust, Clippy, rustfmt, SQLite headers and common tooling live inside the container.
- Cargo registry, git cache and `target/` are Docker volumes, so rebuilds remain fast without polluting the host.
- VS Code or compatible editors can open the repository as a Dev Container.

## Commands

```bash
make dev      # build/start the container
make shell    # open an interactive shell
make down     # stop the container
make check    # cargo check --workspace
make fmt      # cargo fmt --all
make clippy   # strict clippy
make test     # cargo test --workspace
```

## Ports

The compose file exposes:

- `3000` for a future frontend dev server
- `8080` for the Axum API

## Production note

This `Dockerfile` is a developer image. A production image should be multi-stage:

1. builder stage with Rust toolchain
2. runtime stage with only the compiled `qs-app` binary, config and required CA certificates
3. non-root user
4. mounted `/data/runs` volume for run storage
