# Stage 1: build
FROM rust:1.88-alpine AS builder

WORKDIR /app

RUN apk add --no-cache musl-dev ca-certificates

# copy manifest files
COPY Cargo.toml Cargo.lock* ./

# copy source code
COPY src ./src

# build binary
RUN cargo build --release --bin worker

# Stage 2: minimal image
FROM scratch

# copy ca certificates
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt

# copy the built binary
COPY --from=builder /app/target/release/worker /worker

USER 10001:10001

ENTRYPOINT ["/worker"]