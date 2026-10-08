FROM rust:1.99.0-bookworm AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src
RUN cargo build --release --locked --bin knowledge

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libgcc-s1 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 knowledge \
    && useradd --uid 10001 --gid 10001 --no-create-home --home-dir /data --shell /usr/sbin/nologin knowledge \
    && mkdir /data \
    && chown 10001:10001 /data
COPY --from=build /build/target/release/knowledge /usr/local/bin/knowledge
USER 10001:10001
WORKDIR /data
ENV KNOWLEDGE_CACHE=/data
ENV KNOWLEDGE_LISTEN=0.0.0.0:8787
ENV KNOWLEDGE_ALLOW_NETWORK=true
LABEL org.opencontainers.image.source="https://github.com/Studio-Fug/knowledge"
LABEL org.opencontainers.image.description="Content-addressed designs and traceable verification"
EXPOSE 8787
VOLUME ["/data"]
HEALTHCHECK --interval=15s --timeout=3s --start-period=5s --retries=3 \
    CMD ["knowledge", "healthcheck"]
ENTRYPOINT ["knowledge"]
CMD ["serve"]
