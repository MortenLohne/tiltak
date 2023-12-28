FROM --platform=$BUILDPLATFORM rust:alpine3.18 AS builder
ARG TARGETARCH

COPY Cargo.toml .
COPY Cargo.lock .
COPY src src

# Annoying hack because Go and Rust have different names for the CPU instruction sets
RUN export TARGET_TRIPLE=$(echo $TARGETARCH-unknown-linux-musl | sed 's/arm64/aarch64/' | sed 's/amd64/x86_64/') && \
    rustup target add $TARGET_TRIPLE && \
    RUSTFLAGS="-Clinker=rust-lld" cargo build --target $TARGET_TRIPLE --bin playtak --release && \
    mv target/$TARGET_TRIPLE/release/playtak /.

FROM alpine:3.18
COPY --from=builder /playtak /app/
EXPOSE 8080
ENV BENCH=true
ENTRYPOINT ["/app/playtak"]
