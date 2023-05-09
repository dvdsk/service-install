# cargo chef primary use-case is to speed up container builds by 
# running BEFORE the actual source code is copied over. 
# Don't run it on existing codebases to avoid having files being overwritten.

# FROM lukemathwalker/cargo-chef:latest-rust-1 as chef
# WORKDIR /app
#
# FROM chef as planner
# COPY . .
# RUN cargo chef prepare --recipe-path recipe.json
#
# FROM chef as builder
# COPY --from=planner /app/recipe.json recipe.json
# # Build our project dependencies, not our application!
# RUN cargo chef cook --release --recipe-path recipe.json
# COPY . .
# # Build our project
# RUN cargo build -v --example cli
#
# FROM debian:bullseye-slim AS runtime
FROM ubuntu AS runtime
# WORKDIR /app
# COPY --from=builder /app/target/debug/examples/cli cli

# run systemd
USER root
RUN apt-get update
# RUN apt-get -y install systemd
# ENTRYPOINT ["/usr/sbin/init"]

#notes
# https://yast.opensuse.org/blog/2023-02-28/systemd-podman-github-ci
# https://developers.redhat.com/blog/2019/04/24/how-to-run-systemd-in-a-container#other_cool_features_about_podman_and_systemd
