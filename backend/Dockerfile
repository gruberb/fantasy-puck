# 1) Builder stage
FROM rust:1.88 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

# 2) Runtime stage
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends libssl3 ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/fantasy-hockey /app/
EXPOSE 3000
CMD ["./fantasy-hockey", "serve", "--port", "3000"]
