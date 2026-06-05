FROM node:22-bookworm-slim
ARG CONFORMANCE_VERSION
RUN test -n "${CONFORMANCE_VERSION}"
RUN npm install -g "@modelcontextprotocol/conformance@${CONFORMANCE_VERSION}"
ENTRYPOINT ["conformance"]
