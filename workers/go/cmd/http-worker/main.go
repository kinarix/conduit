// Command http-worker is a reference Go worker for the Conduit BPMN engine.
// Subscribes to one or more topics declared in worker.yaml and runs HTTP
// requests with templated URL/body, secret-bearing auth headers, and an
// Idempotency-Key derived from the engine's task id.
//
// Mirrors workers/rust/crates/http-worker — same YAML schema.
package main

import (
	"context"
	"flag"
	"fmt"
	"log/slog"
	"os"
	"os/signal"
	"syscall"

	cw "github.com/kinarix/conduit/workers/go/conduitworker"
)

func main() {
	configPath := flag.String("config", envOr("CONDUIT_WORKER_CONFIG", "worker.yaml"), "Path to worker.yaml")
	workerID := flag.String("worker-id", envOr("CONDUIT_WORKER_ID", defaultWorkerID()), "Worker id reported to the engine (lock owner)")
	flag.Parse()

	cfg, err := LoadConfig(*configPath)
	if err != nil {
		slog.Error("load config", "path", *configPath, "err", err)
		os.Exit(1)
	}

	apiKey := ""
	if cfg.Engine.APIKeyEnv != "" {
		apiKey = os.Getenv(cfg.Engine.APIKeyEnv)
	}
	client := cw.NewClient(cw.ClientConfig{
		BaseURL: cfg.Engine.URL,
		APIKey:  apiKey,
	})

	runner := cw.NewRunner(client, *workerID)
	for topic, hc := range cfg.Handlers {
		h := NewHTTPHandler(topic, hc)
		runner.Register(topic, h.Handle)
		slog.Info("subscribed", "topic", topic, "url_template", hc.URLTemplate)
	}

	slog.Info("starting http-worker",
		"worker_id", *workerID,
		"engine_url", cfg.Engine.URL,
		"handlers", len(cfg.Handlers),
	)

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	if err := runner.Run(ctx); err != nil && err != context.Canceled {
		slog.Error("runner stopped", "err", err)
		os.Exit(1)
	}
}

func envOr(key, fallback string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return fallback
}

func defaultWorkerID() string {
	host, err := os.Hostname()
	if err != nil || host == "" {
		host = "unknown"
	}
	return fmt.Sprintf("http-worker-%s-%d", host, os.Getpid())
}
