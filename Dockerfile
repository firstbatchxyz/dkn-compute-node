FROM --platform=$BUILDPLATFORM rust:1.81 AS builder

# https://docs.docker.com/engine/reference/builder/#automatic-platform-args-in-the-global-scope
#
# We use distroless, which allow the following platforms:
#   linux/amd64
#   linux/arm64
#   linux/arm
#
# To build an image & push them to Docker hub for this Dockerfile:
#
# docker buildx build --platform=linux/amd64,linux/arm64,linux/arm . -t firstbatch/dria-compute-node:latest --builder=dria-builder --push   
ARG BUILDPLATFORM
ARG TARGETPLATFORM
RUN echo "Build platform:  $BUILDPLATFORM"
RUN echo "Target platform: $TARGETPLATFORM"

# build release binary
WORKDIR /usr/src/app
COPY . .
RUN cargo build --bin dkn-compute --release

# copy release binary to distroless
FROM --platform=$BUILDPLATFORM gcr.io/distroless/cc
COPY --from=builder /usr/src/app/target/release/dkn-compute /

EXPOSE 8080

CMD ["./dkn-compute"]
