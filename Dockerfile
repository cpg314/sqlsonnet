FROM debian:bookworm-slim

LABEL org.opencontainers.image.source=https://github.com/cpg314/sqlsonnet
LABEL org.opencontainers.image.licenses=MIT

COPY target-cross/x86_64-unknown-linux-gnu/release/sqlsonnet /usr/bin/sqlsonnet
COPY target-cross/x86_64-unknown-linux-gnu/release/sqlsonnet_clickhouse_proxy /usr/bin/sqlsonnet_clickhouse_proxy

CMD ["/usr/bin/sqlsonnet"]
