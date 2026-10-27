# Imagem do painel do backup na nuvem (ADR 0017). Publicada em ghcr.io/felipecaninnovaes/emissor-nuvem.
FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src src
COPY migrations migrations
COPY templates templates
COPY estatico estatico
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked && cp target/release/emissor-nuvem /usr/local/bin/

FROM debian:bookworm-slim
RUN useradd --system --uid 10001 --no-create-home nuvem
COPY --from=build /usr/local/bin/emissor-nuvem /usr/local/bin/emissor-nuvem
USER nuvem
ENV ENDERECO=0.0.0.0:8080
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s CMD ["emissor-nuvem", "saude"]
LABEL org.opencontainers.image.source="https://github.com/felipecaninnovaes/emissor-de-nota" \
      org.opencontainers.image.description="Painel e API do backup na nuvem do Emissor NFS-e" \
      org.opencontainers.image.licenses="AGPL-3.0-only"
CMD ["emissor-nuvem"]
