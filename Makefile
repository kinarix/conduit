DATABASE_URL ?= postgres://conduit:conduit_secret@localhost/conduit

.PHONY: help db db-stop db-reset migrate test test-watch check fmt lint build run clean

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

migrate: ## Run pending migrations
	DATABASE_URL=$(DATABASE_URL) cargo sqlx migrate run

test: db migrate ## Run all tests against a live database
	TEST_DATABASE_URL=$(DATABASE_URL) cargo test

test-watch: db migrate ## Re-run tests on file changes (requires cargo-watch)
	TEST_DATABASE_URL=$(DATABASE_URL) cargo watch -x test

check: fmt lint test ## Full pre-commit check (fmt + lint + tests)

fmt: ## Format code
	cargo fmt

lint: ## Run clippy (warnings are errors)
	cargo clippy -- -D warnings

build: ## Build the project
	cargo build

run: db migrate ## Start the dev server
	DATABASE_URL=$(DATABASE_URL) cargo run

clean: ## Remove build artifacts
	cargo clean
