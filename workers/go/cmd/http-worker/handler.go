package main

import (
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"strings"

	cw "github.com/kinarix/conduit/workers/go/conduitworker"
)

// HTTPHandler binds one HandlerConfig to one topic and runs the side
// effect on each task. Mirrors workers/rust/crates/http-worker/src/handler.rs.
type HTTPHandler struct {
	topic  string
	cfg    HandlerConfig
	client *http.Client
}

func NewHTTPHandler(topic string, cfg HandlerConfig) *HTTPHandler {
	return &HTTPHandler{
		topic:  topic,
		cfg:    cfg,
		client: &http.Client{Timeout: cfg.Timeout()},
	}
}

// Handle implements cw.HandlerFunc.
func (h *HTTPHandler) Handle(ctx context.Context, task *cw.ExternalTask) (*cw.Result, error) {
	vars := decodeVariables(task)
	taskID := task.ID

	url := render(h.cfg.URLTemplate, vars, taskID)

	var body io.Reader
	if h.cfg.RequestTemplate != nil {
		rendered := renderJSON(h.cfg.RequestTemplate, vars, taskID)
		buf, err := json.Marshal(rendered)
		if err != nil {
			return nil, fmt.Errorf("marshal request body: %w", err)
		}
		body = bytes.NewReader(buf)
	}

	req, err := http.NewRequestWithContext(ctx, h.cfg.Method, url, body)
	if err != nil {
		return nil, fmt.Errorf("build request: %w", err)
	}

	for k, v := range h.cfg.Headers {
		req.Header.Set(k, render(v, vars, taskID))
	}
	if name, value, err := h.authHeader(); err != nil {
		return nil, err
	} else if name != "" {
		req.Header.Set(name, value)
	}
	if h.cfg.Idempotency.IdempotencyEnabled() {
		key := render(h.cfg.Idempotency.KeyTemplate, vars, taskID)
		req.Header.Set(h.cfg.Idempotency.Header, key)
	}
	if body != nil && req.Header.Get("Content-Type") == "" {
		req.Header.Set("Content-Type", "application/json")
	}

	resp, err := h.client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("transport: %w", err)
	}
	defer resp.Body.Close()
	bodyBytes, _ := io.ReadAll(resp.Body)

	switch {
	case resp.StatusCode >= 200 && resp.StatusCode < 300:
		return h.successResult(bodyBytes), nil
	case resp.StatusCode >= 400 && resp.StatusCode < 500 && h.cfg.BpmnErrorOn4xx != "":
		return cw.BpmnError(
			h.cfg.BpmnErrorOn4xx,
			fmt.Sprintf("HTTP %d: %s", resp.StatusCode, truncate(string(bodyBytes), 256)),
			cw.VarLong("http_status", int64(resp.StatusCode)),
			cw.VarString("http_body", string(bodyBytes)),
		), nil
	default:
		return nil, fmt.Errorf("HTTP %d: %s", resp.StatusCode, truncate(string(bodyBytes), 256))
	}
}

func (h *HTTPHandler) successResult(bodyBytes []byte) *cw.Result {
	var bodyJSON any
	_ = json.Unmarshal(bodyBytes, &bodyJSON)

	out := make([]cw.Variable, 0, len(h.cfg.ResponseMapping))
	for varName, path := range h.cfg.ResponseMapping {
		v, ok := jsonpath(path, bodyJSON)
		if !ok {
			out = append(out, cw.VarNull(varName))
			continue
		}
		varJSON, err := cw.VarJSON(varName, v)
		if err != nil {
			out = append(out, cw.VarNull(varName))
			continue
		}
		out = append(out, varJSON)
	}
	return cw.Complete(out...)
}

// decodeVariables unmarshals each wire variable's raw JSON into a Go value
// so render() can substitute templates against typed values.
func decodeVariables(task *cw.ExternalTask) map[string]any {
	out := make(map[string]any, len(task.Variables))
	for _, v := range task.Variables {
		var decoded any
		if err := json.Unmarshal(v.Value, &decoded); err != nil {
			out[v.Name] = nil
			continue
		}
		out[v.Name] = decoded
	}
	return out
}

func (h *HTTPHandler) authHeader() (name, value string, err error) {
	if h.cfg.Auth == nil {
		return "", "", nil
	}
	switch h.cfg.Auth.Type {
	case "bearer":
		token := os.Getenv(h.cfg.Auth.TokenEnv)
		if token == "" {
			return "", "", fmt.Errorf("env var %s (auth.token_env) not set", h.cfg.Auth.TokenEnv)
		}
		return "Authorization", "Bearer " + token, nil
	case "basic":
		user := os.Getenv(h.cfg.Auth.UserEnv)
		pass := os.Getenv(h.cfg.Auth.PasswordEnv)
		if user == "" || pass == "" {
			return "", "", fmt.Errorf(
				"env vars %s/%s (auth.user_env/password_env) not set",
				h.cfg.Auth.UserEnv, h.cfg.Auth.PasswordEnv,
			)
		}
		creds := base64.StdEncoding.EncodeToString([]byte(user + ":" + pass))
		return "Authorization", "Basic " + creds, nil
	default:
		return "", "", fmt.Errorf("unsupported auth.type: %q", h.cfg.Auth.Type)
	}
}

func truncate(s string, max int) string {
	if len(s) <= max {
		return s
	}
	// Walk back to a UTF-8 boundary.
	for i := max; i > 0; i-- {
		if !strings.HasPrefix(s[i:], "¿½") && (s[i]&0xc0) != 0x80 {
			return s[:i] + "…"
		}
	}
	return s[:max] + "…"
}
