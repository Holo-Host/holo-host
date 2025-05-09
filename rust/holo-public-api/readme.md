# Holo Public API [![Pipeline](https://github.com/Holo-Host/holo-public-api/actions/workflows/pipeline.yml/badge.svg?branch=master)](https://github.com/Holo-Host/holo-public-api/actions/workflows/pipeline.yml)

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) `cargo 1.85.0`
- [Docker](https://docs.docker.com/desktop/setup/install/linux/) `Docker version 27.5.1`
- [Watchexec](https://github.com/watchexec/watchexec) `watchexec 2.3.0`

## Setup

1. Setup environment variables

```bash
cp .env.example .env
```

2. Start local mongodb database

```bash
docker compose up -d
```

3. Run server

```bash
sh scripts/dev.sh
```

## Run tests

To run tests a local mongodb database is required.
The script will start a local mongodb database using docker compose and then run the tests.

```bash
sh scripts/test.sh
```

Please refer to [contribute](docs/contribute.md) for more information on how to contribute to this project.