FROM rust:1.85.0 AS builder

ARG PROJECT_NAME

# install musl-tools for static linking
RUN apt-get update && apt-get install -y musl-tools pkg-config

# set the rust target to musl
RUN rustup target add x86_64-unknown-linux-musl

# set working directory and copy source code
WORKDIR /usr/src
COPY . .

# Build the project with the musl target
RUN --mount=type=cache,target=/root/.cargo \
    --mount=type=cache,target=/usr/src/target \
    cargo build --target x86_64-unknown-linux-musl -p ${PROJECT_NAME} --release && \
    cp /usr/src/target/x86_64-unknown-linux-musl/release/${PROJECT_NAME} /usr/src/build

# copy build binary to empty image
FROM scratch
COPY --from=builder /usr/src/build /build
CMD ["/build"]