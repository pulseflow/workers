FROM rust:1.81.0 as build
ENV PKG_CONFIG_ALLOW_CROSS=1

WORKDIR /usr/src/workers
COPY . .
RUN cargo build --release


FROM debian:bookworm-slim

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates openssl \
 && apt-get clean \
 && rm -rf /var/lib/apt/lists/*

RUN update-ca-certificates

COPY --from=build /usr/src/workers/target/release/meta_workers /workers/meta_workers
WORKDIR /meta_workers

CMD /workers/meta_workers
