# Installation

## Prerequisites

- **Rust toolchain** -- stable channel (1.75+). The project pins to stable via `rust-toolchain.toml`.
- **cargo** -- comes with the Rust toolchain.

No other system dependencies are required. The server uses `rustls` for TLS (no OpenSSL).

## Building from source

```bash
git clone https://github.com/thewillyhuman/dns.git
cd dns
cargo build --release
```

The binary is at `target/release/cern-dns`.

## Cross-compilation (static binary)

For deployment on minimal containers (`scratch`, `distroless`), build a fully static binary with musl:

```bash
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
```

The resulting binary has no runtime dependencies beyond the kernel.

## Verify the build

```bash
# Check the binary runs
./target/release/cern-dns --help

# Validate the example config
./target/release/cern-dns --check-config config/config.toml

# Run the test suite
cargo test --workspace
```

## Docker

Example `Dockerfile` for a minimal image:

```dockerfile
FROM rust:latest AS builder
WORKDIR /build
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /build/target/release/cern-dns /usr/local/bin/cern-dns
COPY config/ /etc/dns/
EXPOSE 53/udp 53/tcp 853/tcp 443/tcp 9153/tcp
ENTRYPOINT ["cern-dns", "--config", "/etc/dns/config.toml"]
```

For a static musl build, replace the builder target and use `FROM scratch` as the runtime image.
