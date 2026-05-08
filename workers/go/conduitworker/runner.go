package conduitworker

import (
	"context"
	"errors"
	"log/slog"
	"sync"
	"time"
)

// HandlerFunc executes one task. Returning (nil, err) reports a transient
// failure (POST /failure). Returning (Result, nil) reports complete or
// bpmn-error depending on Result.Kind.
type HandlerFunc func(ctx context.Context, task *ExternalTask) (*Result, error)

// RunnerConfig tunes the polling loop.
type RunnerConfig struct {
	WorkerID         string
	MaxJobs          int
	LockDurationSecs int
	PollInterval     time.Duration
}

// Runner is the fetch-handle-report loop. Register handlers with Register,
// then call Run.
//
// Go has no annotations; Register is the idiomatic registration form.
type Runner struct {
	client *Client
	cfg    RunnerConfig

	mu       sync.RWMutex
	handlers map[string]HandlerFunc
}

// NewRunner builds a Runner with sensible defaults: max_jobs=10, lock=30s, poll=1s.
func NewRunner(client *Client, workerID string) *Runner {
	return &Runner{
		client: client,
		cfg: RunnerConfig{
			WorkerID:         workerID,
			MaxJobs:          10,
			LockDurationSecs: 30,
			PollInterval:     time.Second,
		},
		handlers: make(map[string]HandlerFunc),
	}
}

// Register binds a HandlerFunc to a topic. Calling Register with the same
// topic replaces the previous binding.
func (r *Runner) Register(topic string, fn HandlerFunc) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.handlers[topic] = fn
}

// SetConfig replaces tuning after construction.
func (r *Runner) SetConfig(cfg RunnerConfig) {
	if cfg.MaxJobs == 0 {
		cfg.MaxJobs = 10
	}
	if cfg.LockDurationSecs == 0 {
		cfg.LockDurationSecs = 30
	}
	if cfg.PollInterval == 0 {
		cfg.PollInterval = time.Second
	}
	if cfg.WorkerID == "" {
		cfg.WorkerID = r.cfg.WorkerID
	}
	r.cfg = cfg
}

// Run loops until ctx is cancelled, fetching one topic at a time round-robin.
func (r *Runner) Run(ctx context.Context) error {
	if len(r.handlers) == 0 {
		return errors.New("no handlers registered")
	}
	for {
		if err := ctx.Err(); err != nil {
			return err
		}
		didWork := r.tick(ctx)
		if !didWork {
			select {
			case <-ctx.Done():
				return ctx.Err()
			case <-time.After(r.cfg.PollInterval):
			}
		}
	}
}

func (r *Runner) tick(ctx context.Context) bool {
	r.mu.RLock()
	topics := make([]string, 0, len(r.handlers))
	for t := range r.handlers {
		topics = append(topics, t)
	}
	r.mu.RUnlock()

	didWork := false
	for _, topic := range topics {
		tasks, err := r.client.FetchAndLock(ctx, r.cfg.WorkerID, topic, r.cfg.MaxJobs, r.cfg.LockDurationSecs)
		if err != nil {
			slog.Error("fetch-and-lock failed", "topic", topic, "err", err)
			continue
		}
		for i := range tasks {
			r.dispatch(ctx, &tasks[i])
		}
		if len(tasks) > 0 {
			didWork = true
		}
	}
	return didWork
}

func (r *Runner) dispatch(ctx context.Context, task *ExternalTask) {
	topic := ""
	if task.Topic != nil {
		topic = *task.Topic
	}
	r.mu.RLock()
	fn, ok := r.handlers[topic]
	r.mu.RUnlock()
	if !ok {
		slog.Warn("no handler for topic", "topic", topic, "task_id", task.ID)
		return
	}

	result, err := fn(ctx, task)
	switch {
	case err != nil:
		if ferr := r.client.Failure(ctx, task.ID, r.cfg.WorkerID, err.Error()); ferr != nil {
			slog.Error("failure call failed", "task_id", task.ID, "err", ferr)
		}
	case result != nil && result.Kind == ResultBpmnError:
		if berr := r.client.BpmnError(ctx, task.ID, r.cfg.WorkerID, result.ErrorCode, result.ErrorMessage, result.Variables); berr != nil {
			slog.Error("bpmn-error call failed", "task_id", task.ID, "err", berr)
		}
	default:
		var vars []Variable
		if result != nil {
			vars = result.Variables
		}
		if cerr := r.client.Complete(ctx, task.ID, r.cfg.WorkerID, vars); cerr != nil {
			slog.Error("complete call failed", "task_id", task.ID, "err", cerr)
		}
	}
}
