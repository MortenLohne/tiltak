FROM clux/muslrust:stable AS builder
COPY Cargo.toml .
COPY Cargo.lock .
COPY src src
RUN cargo build --release
RUN mv target/x86_64-unknown-linux-musl/release/main /.

FROM alpine:3.18
COPY --from=builder /main /app/
EXPOSE 8080
ENTRYPOINT ["/app/main"]
