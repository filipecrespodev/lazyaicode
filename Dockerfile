# LazyAICode na web: o app (wasm) e o motor (lazyaicode-cli serve) num contêiner.
#
#   docker build -t lazyaicode-web .
#   docker run --rm -p 8080:8080 -e LAZYAICODE_TOKEN=$(openssl rand -hex 32) \
#     -v lazyaicode-data:/data -v $HOME/.claude:/root/.claude -v $HOME/code:/work lazyaicode-web
#
# Abra http://localhost:8080/#token=<o token>. O motor roda os agentes e o git
# dentro do contêiner, sobre /work. Exponha só atrás de um proxy com TLS.

ARG RUST_VERSION=1
FROM rust:${RUST_VERSION}-bookworm AS build
ARG TRUNK_VERSION=0.21.14
RUN rustup target add wasm32-unknown-unknown \
 && curl -fsSL "https://github.com/trunk-rs/trunk/releases/download/v${TRUNK_VERSION}/trunk-x86_64-unknown-linux-gnu.tar.gz" | tar -xz -C /usr/local/bin
WORKDIR /src
COPY . .
RUN cd apps/lazyaicode-web && trunk build --release
RUN cargo build --release -p lazyaicode-cli

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates git openssh-client curl nodejs npm \
 && npm install -g @anthropic-ai/claude-code \
 && apt-get clean && rm -rf /var/lib/apt/lists/* /root/.npm
COPY --from=build /src/target/release/lazyaicode-cli /usr/local/bin/lazyaicode-cli
COPY --from=build /src/apps/lazyaicode-web/dist /srv/web
ENV LAZYAICODE_DATA_DIR=/data
VOLUME ["/data", "/work"]
WORKDIR /work
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s CMD curl -fsS http://127.0.0.1:8080/ >/dev/null || exit 1
ENTRYPOINT ["lazyaicode-cli"]
CMD ["serve", "--web", "0.0.0.0:8080", "--web-root", "/srv/web"]
