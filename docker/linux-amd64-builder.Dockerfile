FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        cmake \
        ninja-build \
        pkg-config \
        libvulkan-dev \
        glslang-tools \
        ca-certificates \
        python3 \
        cargo \
        rustc \
    && rm -rf /var/lib/apt/lists/*
