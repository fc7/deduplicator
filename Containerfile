####################################################################################################
## Builder
####################################################################################################
FROM registry.access.redhat.com/ubi9/ubi:9.6 AS builder
RUN dnf upgrade -y && dnf install -y rust cargo git

WORKDIR /app

ENV RUSTFLAGS="-C target-feature=+aes,+sha2"

#RUN git clone --branch v0.3.1 --depth=1 https://github.com/sreedevk/deduplicator.git
#RUN cd deduplicator && cargo build  --release
COPY . .
RUN cargo build --release

####################################################################################################
## Final image
####################################################################################################
FROM registry.access.redhat.com/ubi9/ubi-micro:9.6

WORKDIR /app

COPY --from=builder /app/target/release/deduplicator ./
