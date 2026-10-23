FROM rust:1.99.1-slim-bookworm

WORKDIR /workspace

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        build-essential \
        ca-certificates \
        cmake \
        make \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN rustup component add rustfmt clippy

COPY Cargo.toml Cargo.lock* rust-toolchain.toml Makefile ./
COPY backend ./backend

RUN cargo test --workspace --locked

CMD ["cargo", "run", "-p", "audiosync-sim", "--locked"]
