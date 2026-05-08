// Command http-worker is a scaffolded Go binary that subscribes to the
// http.call topic. The handler logic is not implemented yet — this scaffold
// exists to validate the SDK shape end-to-end. The Rust http-worker
// (workers/rust/crates/http-worker) is the reference implementation.
package main

import (
	"context"
	"fmt"
	"os"
	"os/signal"
	"syscall"

	"github.com/kinarix/conduit/workers/go/conduitworker"
)

func main() {
	engineURL := envOr("CONDUIT_ENGINE_URL", "http://localhost:8080")
	workerID := envOr("CONDUIT_WORKER_ID", "go-http-worker-1")

	client := conduitworker.NewClient(conduitworker.ClientConfig{
		BaseURL: engineURL,
		APIKey:  os.Getenv("CONDUIT_API_KEY"),
	})

	runner := conduitworker.NewRunner(client, workerID)
	runner.Register("http.call", func(ctx context.Context, task *conduitworker.ExternalTask) (*conduitworker.Result, error) {
		return nil, fmt.Errorf("Go http-worker is scaffolded only; see workers/rust/crates/http-worker for the reference implementation")
	})

	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer cancel()
	if err := runner.Run(ctx); err != nil && err != context.Canceled {
		fmt.Fprintln(os.Stderr, "runner stopped:", err)
		os.Exit(1)
	}
}

func envOr(key, fallback string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return fallback
}
