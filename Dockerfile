# Dev/CI toolchain for Aerial Analytics Platform (Tauri 2, headless).
# Mirrors .github/workflows/ci.yml: Ubuntu 24.04, Node 24, Rust stable + Tauri Linux deps.
# No source COPY — mount the repo via compose; this image is only the toolchain.

FROM ubuntu:24.04 AS runtime

ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

# System deps first (change least often): build tools + Tauri Linux prerequisites.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
      build-essential \
      ca-certificates \
      curl \
      file \
      git \
      gnupg \
      libayatana-appindicator3-dev \
      libssl-dev \
      libwebkit2gtk-4.1-dev \
      libxdo-dev \
      librsvg2-dev \
      patchelf \
      pkg-config \
      wget \
      xz-utils \
    && rm -rf /var/lib/apt/lists/*

# Node.js 24 (matches CI NODE_VERSION).
RUN curl -fsSL https://deb.nodesource.com/setup_24.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*

# Rust stable with clippy + rustfmt (CI gate components).
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain stable \
    && rustup component add clippy rustfmt \
    && chmod -R a+rX /usr/local/cargo /usr/local/rustup

# Non-root user. UID/GID overridable to match host for bind-mount ownership.
# ubuntu:24.04 ships an 'ubuntu' user at 1000:1000 — drop it so our UID/GID can claim that slot.
ARG USERNAME=dev
ARG USER_UID=1000
ARG USER_GID=1000
RUN userdel -r ubuntu 2>/dev/null || true \
    && groupdel ubuntu 2>/dev/null || true \
    && groupadd --gid "${USER_GID}" "${USERNAME}" \
    && useradd --uid "${USER_UID}" --gid "${USER_GID}" --create-home "${USERNAME}" \
    && mkdir -p \
      /usr/local/cargo/registry \
      /usr/local/cargo/git \
      /home/${USERNAME}/.npm \
      /workspace \
      /workspace/node_modules \
      /workspace/src-tauri/target \
    && chown -R ${USERNAME}:${USERNAME} /usr/local/cargo /usr/local/rustup /home/${USERNAME} /workspace

USER ${USERNAME}
ENV HOME=/home/${USERNAME} \
    npm_config_cache=/home/${USERNAME}/.npm

WORKDIR /workspace

CMD ["bash"]
