FROM debian:bookworm-slim

LABEL org.opencontainers.image.source=https://github.com/cpg314/sqlsonnet
LABEL org.opencontainers.image.licenses=MIT

RUN apt update && \
    apt install -y curl && \
    curl -L https://github.com/google/go-jsonnet/releases/download/v0.20.0/jsonnetfmt-go_0.20.0_linux_amd64.deb --output jsonnetfmt.deb && \
    dpkg -i jsonnetfmt.deb && \
    apt clean

COPY target-cross/x86_64-unknown-linux-gnu/release/sqlsonnet /usr/bin/sqlsonnet
COPY target-cross/x86_64-unknown-linux-gnu/release/sqlsonnet_clickhouse_proxy /usr/bin/sqlsonnet_clickhouse_proxy


CMD ["/usr/bin/sqlsonnet"]
