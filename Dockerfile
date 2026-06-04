FROM rust:1.88-alpine AS builder

WORKDIR /app

RUN apk add --no-cache musl-dev ca-certificates

COPY Cargo.toml Cargo.lock* ./
COPY main.rs ./main.rs
COPY automation_core ./automation_core

RUN cargo build --release --bin worker


FROM scratch

COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=builder /app/target/release/worker /worker

USER 10001:10001

ENTRYPOINT ["/worker"]