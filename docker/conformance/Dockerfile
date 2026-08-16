FROM node:22-bookworm-slim
ARG CONFORMANCE_VERSION
RUN test -n "${CONFORMANCE_VERSION}"
# 40-char hex SHA: clone that commit, build, keep the bundled dist (untagged
# 0.2.0-alpha.11 at c321dd32035556e6769d3724a8ee97d87c3faaac). `npm install -g`
# of a local path only symlinks, so we install the binary ourselves. Any other
# VERSION value is an npm version or dist-tag.
RUN if echo "${CONFORMANCE_VERSION}" | grep -Eq '^[0-9a-f]{40}$'; then \
      apt-get update \
      && apt-get install -y --no-install-recommends git ca-certificates \
      && git clone --filter=blob:none https://github.com/modelcontextprotocol/conformance.git /opt/conformance \
      && git -C /opt/conformance checkout "${CONFORMANCE_VERSION}" \
      && npm --prefix /opt/conformance ci \
      && npm --prefix /opt/conformance run build \
      && npm --prefix /opt/conformance prune --omit=dev \
      && ln -sf /opt/conformance/dist/index.js /usr/local/bin/conformance \
      && rm -rf /opt/conformance/src /root/.npm \
      && apt-get purge -y git \
      && apt-get autoremove -y \
      && rm -rf /var/lib/apt/lists/*; \
    else \
      npm install -g "@modelcontextprotocol/conformance@${CONFORMANCE_VERSION}"; \
    fi
ENTRYPOINT ["conformance"]
