# cargo chef primary use-case is to speed up container builds by 
# running BEFORE the actual source code is copied over. 
# Don't run it on existing codebases to avoid having files being overwritten.

FROM lukemathwalker/cargo-chef:latest-rust-1 as chef
WORKDIR /app

FROM chef as planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef as builder
COPY --from=planner /app/recipe.json recipe.json
# Build our project dependencies, not our application!
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
# Build our project
RUN cargo build -v --example cli

FROM debian:bullseye-slim AS runtime
WORKDIR /app
COPY --from=builder /app/target/debug/examples/cli cli
