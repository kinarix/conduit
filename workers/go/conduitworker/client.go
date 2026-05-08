package conduitworker

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"
)

// ClientConfig configures connectivity to the Conduit engine.
type ClientConfig struct {
	BaseURL        string
	APIKey         string
	RequestTimeout time.Duration
}

// Client is a typed wrapper over the engine's /api/v1/external-tasks/* endpoints.
type Client struct {
	http    *http.Client
	baseURL string
	apiKey  string
}

// NewClient constructs a Client. RequestTimeout defaults to 30s when zero.
func NewClient(cfg ClientConfig) *Client {
	timeout := cfg.RequestTimeout
	if timeout == 0 {
		timeout = 30 * time.Second
	}
	return &Client{
		http:    &http.Client{Timeout: timeout},
		baseURL: strings.TrimRight(cfg.BaseURL, "/"),
		apiKey:  cfg.APIKey,
	}
}

type fetchAndLockReq struct {
	WorkerID         string  `json:"worker_id"`
	Topic            *string `json:"topic,omitempty"`
	MaxJobs          int     `json:"max_jobs"`
	LockDurationSecs int     `json:"lock_duration_secs"`
}

// FetchAndLock long-polls for tasks on the given topic.
func (c *Client) FetchAndLock(ctx context.Context, workerID, topic string, maxJobs, lockDurationSecs int) ([]ExternalTask, error) {
	body := fetchAndLockReq{
		WorkerID:         workerID,
		Topic:            &topic,
		MaxJobs:          maxJobs,
		LockDurationSecs: lockDurationSecs,
	}
	resp, err := c.post(ctx, "/api/v1/external-tasks/fetch-and-lock", body)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	var tasks []ExternalTask
	if err := json.NewDecoder(resp.Body).Decode(&tasks); err != nil {
		return nil, fmt.Errorf("decode fetch-and-lock response: %w", err)
	}
	return tasks, nil
}

type completeReq struct {
	WorkerID  string     `json:"worker_id"`
	Variables []Variable `json:"variables"`
}

// Complete reports the task as completed successfully.
func (c *Client) Complete(ctx context.Context, taskID, workerID string, vars []Variable) error {
	resp, err := c.post(ctx, fmt.Sprintf("/api/v1/external-tasks/%s/complete", taskID), completeReq{
		WorkerID:  workerID,
		Variables: vars,
	})
	if err != nil {
		return err
	}
	resp.Body.Close()
	return nil
}

type failureReq struct {
	WorkerID     string `json:"worker_id"`
	ErrorMessage string `json:"error_message"`
}

// Failure reports a transient/system failure. The engine decrements retries
// and re-locks the task after the lock TTL.
func (c *Client) Failure(ctx context.Context, taskID, workerID, errorMessage string) error {
	resp, err := c.post(ctx, fmt.Sprintf("/api/v1/external-tasks/%s/failure", taskID), failureReq{
		WorkerID:     workerID,
		ErrorMessage: errorMessage,
	})
	if err != nil {
		return err
	}
	resp.Body.Close()
	return nil
}

type bpmnErrorReq struct {
	WorkerID     string     `json:"worker_id"`
	ErrorCode    string     `json:"error_code"`
	ErrorMessage string     `json:"error_message"`
	Variables    []Variable `json:"variables"`
}

// BpmnError throws a BPMN error that branches through a matching boundaryErrorEvent.
func (c *Client) BpmnError(ctx context.Context, taskID, workerID, code, message string, vars []Variable) error {
	resp, err := c.post(ctx, fmt.Sprintf("/api/v1/external-tasks/%s/bpmn-error", taskID), bpmnErrorReq{
		WorkerID:     workerID,
		ErrorCode:    code,
		ErrorMessage: message,
		Variables:    vars,
	})
	if err != nil {
		return err
	}
	resp.Body.Close()
	return nil
}

type extendLockReq struct {
	WorkerID         string `json:"worker_id"`
	LockDurationSecs int    `json:"lock_duration_secs"`
}

// ExtendLock refreshes the lock without completing.
func (c *Client) ExtendLock(ctx context.Context, taskID, workerID string, lockDurationSecs int) error {
	resp, err := c.post(ctx, fmt.Sprintf("/api/v1/external-tasks/%s/extend-lock", taskID), extendLockReq{
		WorkerID:         workerID,
		LockDurationSecs: lockDurationSecs,
	})
	if err != nil {
		return err
	}
	resp.Body.Close()
	return nil
}

func (c *Client) post(ctx context.Context, path string, body any) (*http.Response, error) {
	buf, err := json.Marshal(body)
	if err != nil {
		return nil, fmt.Errorf("marshal request: %w", err)
	}
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, c.baseURL+path, bytes.NewReader(buf))
	if err != nil {
		return nil, err
	}
	req.Header.Set("Content-Type", "application/json")
	if c.apiKey != "" {
		req.Header.Set("Authorization", "Bearer "+c.apiKey)
	}
	resp, err := c.http.Do(req)
	if err != nil {
		return nil, fmt.Errorf("post %s: %w", path, err)
	}
	if resp.StatusCode/100 != 2 {
		body, _ := io.ReadAll(resp.Body)
		resp.Body.Close()
		return nil, &HTTPError{Status: resp.StatusCode, Body: string(body)}
	}
	return resp, nil
}

// HTTPError is returned when the engine responds with a non-2xx status.
type HTTPError struct {
	Status int
	Body   string
}

func (e *HTTPError) Error() string {
	return fmt.Sprintf("engine returned %d: %s", e.Status, e.Body)
}
