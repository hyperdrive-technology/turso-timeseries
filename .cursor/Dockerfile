FROM mcr.microsoft.com/devcontainers/rust:1-bookworm

SHELL ["/bin/bash", "-lc"]

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates curl git xz-utils \
  && rm -rf /var/lib/apt/lists/*

COPY .nvmrc /tmp/hyperdrive-node-version

RUN set -euxo pipefail; \
  NODE_VERSION="$(tr -d '[:space:]' < /tmp/hyperdrive-node-version)"; \
  ARCH="$(dpkg --print-architecture)"; \
  case "${ARCH}" in \
    amd64) NODE_ARCH="x64" ;; \
    arm64) NODE_ARCH="arm64" ;; \
    *) echo "Unsupported architecture: ${ARCH}" >&2; exit 1 ;; \
  esac; \
  curl -fsSL "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-${NODE_ARCH}.tar.xz" -o /tmp/node.tar.xz; \
  mkdir -p /usr/local/lib/nodejs; \
  tar -xJf /tmp/node.tar.xz -C /usr/local/lib/nodejs --strip-components=1; \
  rm /tmp/node.tar.xz /tmp/hyperdrive-node-version; \
  ln -sf /usr/local/lib/nodejs/bin/node /usr/local/bin/node; \
  ln -sf /usr/local/lib/nodejs/bin/npm /usr/local/bin/npm; \
  ln -sf /usr/local/lib/nodejs/bin/npx /usr/local/bin/npx; \
  node --version; \
  npm --version

# Browser E2E builds a custom Turso WASM branch; install the target in the image layer.
RUN rustup target add wasm32-wasip1-threads
