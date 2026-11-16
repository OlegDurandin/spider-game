FROM node:24-alpine
WORKDIR /qa
COPY scripts/wasm-test/package.json scripts/wasm-test/package-lock.json ./
RUN npm ci --ignore-scripts --no-audit --no-fund
COPY scripts/wasm-test/test.mjs ./test.mjs
USER node
ENTRYPOINT ["node", "/qa/test.mjs"]
