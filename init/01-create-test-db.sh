#!/usr/bin/env bash
# Postgres entrypoint init script: runs once on first container start
# (see https://hub.docker.com/_/postgres "Initialization scripts").
# Creates a dedicated database for integration tests so they can be
# truncated / recreated without touching the dev database.
set -euo pipefail

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
    CREATE DATABASE conduit_test OWNER $POSTGRES_USER;
EOSQL
