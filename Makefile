DATABASE_URL      ?= postgres://conduit:conduit_secret@localhost/conduit
TEST_DATABASE_URL ?= postgres://conduit:conduit_secret@localhost/conduit_test
# Dev-only AEAD key for the secrets table. Override in production with
# `openssl rand -base64 32`. 32-byte base64-decoded value.
CONDUIT_SECRETS_KEY ?= AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
# Dev-only JWT signing key and bootstrap admin credentials. Override in production.
CONDUIT_JWT_SIGNING_KEY          ?= dev-only-insecure-signing-key
CONDUIT_TENANT_ISOLATION         ?= Single
CONDUIT_BOOTSTRAP_ADMIN_EMAIL    ?= admin@local
CONDUIT_BOOTSTRAP_ADMIN_PASSWORD ?= admin
CONDUIT_BOOTSTRAP_ADMIN_ORG_SLUG ?= root

.PHONY: help db db-stop db-reset migrate migrate-test clean-db clean-test-db test test-watch check fmt lint build run clean

help:
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  %-15s %s\n", $$1, $$2}'

db: ## Start PostgreSQL container
	docker compose up -d
	@echo "Waiting for postgres..."
	@until docker exec conduit-postgres pg_isready -U conduit -d conduit -q; do sleep 1; done
	@echo "Postgres ready"

db-stop: ## Stop PostgreSQL container
	docker compose stop

db-reset: ## Destroy and recreate the database volume
	docker compose down -v
	$(MAKE) db

clean-db: ## Truncate all data in the dev database (keeps schema)
	docker exec conduit-postgres psql -U conduit -d conduit -c \
		"TRUNCATE event_subscriptions, jobs, tasks, variables, execution_history, executions, process_instances, process_definitions, users, orgs RESTART IDENTITY CASCADE;"

clean-test-db: ## Truncate all data in the test database (keeps schema)
	docker exec conduit-postgres psql -U conduit -d conduit_test -c \
		"TRUNCATE event_subscriptions, jobs, tasks, variables, execution_history, executions, process_instances, process_definitions, users, orgs RESTART IDENTITY CASCADE;"

migrate: ## Run pending migrations against the dev database
	DATABASE_URL=$(DATABASE_URL) cargo sqlx migrate run

migrate-test: ## Run pending migrations against the test database
	DATABASE_URL=$(TEST_DATABASE_URL) cargo sqlx migrate run

test: db migrate-test ## Run all tests against the test database
	TEST_DATABASE_URL=$(TEST_DATABASE_URL) cargo test

test-watch: db migrate-test ## Re-run tests on file changes (requires cargo-watch)
	TEST_DATABASE_URL=$(TEST_DATABASE_URL) cargo watch -x test

check: fmt lint test ## Full pre-commit check (fmt + lint + tests)

fmt: ## Format code
	cargo fmt

lint: ## Run clippy (warnings are errors)
	cargo clippy -- -D warnings

build: ## Build the project
	cargo build

run: db migrate ## Start the dev server
	DATABASE_URL=$(DATABASE_URL) \
	CONDUIT_SECRETS_KEY=$(CONDUIT_SECRETS_KEY) \
	CONDUIT_JWT_SIGNING_KEY=$(CONDUIT_JWT_SIGNING_KEY) \
	CONDUIT_TENANT_ISOLATION=$(CONDUIT_TENANT_ISOLATION) \
	CONDUIT_BOOTSTRAP_ADMIN_EMAIL=$(CONDUIT_BOOTSTRAP_ADMIN_EMAIL) \
	CONDUIT_BOOTSTRAP_ADMIN_PASSWORD=$(CONDUIT_BOOTSTRAP_ADMIN_PASSWORD) \
	CONDUIT_BOOTSTRAP_ADMIN_ORG_SLUG=$(CONDUIT_BOOTSTRAP_ADMIN_ORG_SLUG) \
	CONDUIT_LOG_LEVEL=info cargo run

clean: ## Remove build artifacts
	cargo clean
