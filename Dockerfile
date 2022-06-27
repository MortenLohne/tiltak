FROM clux/muslrust:stable AS builder
COPY Cargo.toml .
COPY Cargo.lock .
COPY src src
RUN --mount=type=cache,target=/volume/target \
    --mount=type=cache,target=/root/.cargo/registry \
    cargo build --release && \
    mv /volume/target/x86_64-unknown-linux-musl/release/main .

FROM gcr.io/distroless/static:nonroot
COPY --from=builder --chown=nonroot:nonroot /volume/main /app/
EXPOSE 8080
ENTRYPOINT ["/app/main"]
