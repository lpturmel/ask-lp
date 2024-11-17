FROM rust:1.82.0-slim-bullseye AS builder

WORKDIR /app
COPY . .
RUN --mount=type=cache,target=/app/target \
		--mount=type=cache,target=/usr/local/cargo/registry \
		--mount=type=cache,target=/usr/local/cargo/git \
		--mount=type=cache,target=/usr/local/rustup \
		set -eux; \
    apt-get update; \
    apt-get install -y musl-tools libfindbin-libs-perl perl build-essential checkinstall zlib1g-dev; \
    rustup default stable; \
	  cargo build --release; \
		objcopy --compress-debug-sections target/release/asklp ./asklp

################################################################################
FROM debian:12.1-slim

RUN set -eux; \
		export DEBIAN_FRONTEND=noninteractive; \
	  apt update; \
		apt install --yes --no-install-recommends bind9-dnsutils iputils-ping ca-certificates; \
		apt clean autoclean; \
		apt autoremove --yes; \
		rm -rf /var/lib/{apt,dpkg,cache,log}/; \
		echo "Installed base utils!"

WORKDIR app

COPY --from=builder /app/asklp ./asklp
COPY --from=builder /app/static ./static
CMD ["./asklp"]


