FROM rust:slim-trixie AS chef
RUN cargo install cargo-chef
WORKDIR /usr/src/autokatze

FROM chef AS planner
WORKDIR /usr/src/autokatze
COPY . .
RUN cargo chef prepare  --recipe-path recipe.json

FROM chef AS builder
WORKDIR /usr/src/autokatze
COPY --from=planner /usr/src/autokatze/recipe.json recipe.json
#RUN apt-get update && apt-get install -y pkg-config openssl libssl-dev curl && rm -rf /var/lib/apt/lists/*
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release

# Astro build stage
#FROM node:20-slim AS astro-base
#RUN npm install -g pnpm
#WORKDIR /usr/src/astro
#COPY docs/pnpm-lock.yaml docs/package.json ./
#RUN pnpm install --frozen-lockfile
#COPY docs .
#RUN pnpm run build

FROM debian:trixie-slim AS runtime
WORKDIR /autokatze
COPY --from=builder /usr/src/autokatze/target/release/autokatze /usr/local/bin
# COPY --from=astro-base /usr/src/astro/dist /kromer/docs/dist
CMD ["autokatze"]
