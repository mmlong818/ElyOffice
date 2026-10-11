# Linux builds on glibc 2.43 run on Ubuntu 26.04 onward.
FROM ubuntu:26.04@sha256:f144425ff09be612d6d9ad965196e9cdc23dae1f42110a8a11a3e9a8198759f7

ENV DEBIAN_FRONTEND=noninteractive
RUN sed -i 's/^Types: deb$/Types: deb deb-src/' /etc/apt/sources.list.d/ubuntu.sources \
  && apt-get update \
  && apt-get build-dep -y libreoffice \
  && rm -rf /var/lib/apt/lists/*
RUN apt-get update \
  && apt-get install -y --no-install-recommends \
    curl ca-certificates git rsync patch patchelf file jq python3 nodejs npm rustup \
    cmake ninja-build pkg-config nasm \
    libssl-dev libjitterentropy3-dev libzstd-dev zlib1g-dev \
    libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libx11-xcb-dev libxcb1-dev libfontconfig-dev \
  && rm -rf /var/lib/apt/lists/*
ENV RUSTUP_HOME=/opt/rustup
# As rust-toolchain.toml pins it.
RUN rustup toolchain install 1.96.0 --profile minimal --component rustfmt,clippy \
  && rustup default 1.96.0 \
  && chmod -R a+rX /opt/rustup
