FROM rust:1.85.0 AS builder

ARG PROJECT_NAME

WORKDIR /usr/src
COPY . .

RUN --mount=type=cache,target=/root/.cargo \
    --mount=type=cache,target=/usr/src/target \
    cargo build -p ${PROJECT_NAME} --release && \
    cp /usr/src/target/release/${PROJECT_NAME} /usr/src/build

FROM gcr.io/distroless/cc
COPY --from=builder /usr/src/build /build
CMD ["/build"]