FROM rust:1.78.0
ENV PKG_CONFIG_ALLOW_CROSS=1
RUN apt-get update \
	&& apt-get instal -y --no-install-recommends ca-certificates openssl libssl-dev \
	&& apt-get clean \
	&& rm -rf /var/lib/apt/lists/*
RUN update-ca-certificates

WORKDIR /usr/src/workers
COPY . .
RUN cargo build --release
RUN cp target/release/meta_workers ./meta_binary

CMD ["./meta_binary"]
