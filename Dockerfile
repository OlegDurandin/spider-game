FROM rust:1.95-bookworm AS tooling
RUN rustup target add wasm32-unknown-unknown && cargo install trunk --locked --version 0.21.14
# Current wasm-bindgen Linux releases require glibc >= 2.38.
FROM rust:1.95-trixie AS build
COPY --from=tooling /usr/local/cargo/bin/trunk /usr/local/cargo/bin/trunk
COPY --from=tooling /usr/local/rustup/toolchains /usr/local/rustup/toolchains
COPY --from=tooling /usr/local/cargo/registry /usr/local/cargo/registry
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY engine ./engine
COPY web ./web
COPY server ./server
RUN --mount=type=cache,id=spider-target,target=/build/target,sharing=locked \
    --mount=type=cache,id=spider-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=spider-tools,target=/root/.cache,sharing=locked \
    cargo test --workspace --locked && cargo build --release --locked -p spider-server && cp /build/target/release/spider-server /build/spider-server && cd web && trunk --skip-version-check build --release --locked

FROM debian:trixie-slim AS api
RUN mkdir /data && chown 65532:65532 /data
COPY --from=build /build/spider-server /usr/local/bin/spider-server
USER 65532:65532
EXPOSE 8082
HEALTHCHECK --interval=20s --timeout=3s --start-period=5s --retries=3 CMD ["/usr/local/bin/spider-server", "--healthcheck"]
ENTRYPOINT ["/usr/local/bin/spider-server"]

FROM nginx:1.28-alpine AS runtime
COPY deploy/nginx.conf /etc/nginx/nginx.conf
COPY --from=build /build/web/dist /usr/share/nginx/html
USER nginx
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 CMD wget -q -O /dev/null http://127.0.0.1:8080/healthz || exit 1
ENTRYPOINT ["nginx", "-g", "daemon off;"]

