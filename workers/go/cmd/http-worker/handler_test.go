package main

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"strings"
	"testing"

	cw "github.com/kinarix/conduit/workers/go/conduitworker"
)

// helper: build an ExternalTask with a fixed task id and inline variables.
func mkTask(t *testing.T, id string, vars map[string]any) *cw.ExternalTask {
	t.Helper()
	out := &cw.ExternalTask{ID: id}
	for k, v := range vars {
		raw, err := json.Marshal(v)
		if err != nil {
			t.Fatalf("marshal var %q: %v", k, err)
		}
		out.Variables = append(out.Variables, cw.Variable{
			Name:  k,
			Value: raw,
		})
	}
	return out
}

func TestHandle_RendersURLAndBody_AndMapsResponse(t *testing.T) {
	var captured *http.Request
	var capturedBody []byte
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		capturedBody, _ = io.ReadAll(r.Body)
		captured = r
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"id":"ord-42","total":1500}`))
	}))
	defer srv.Close()

	cfg := HandlerConfig{
		URLTemplate: srv.URL + "/orders",
		Method:      "POST",
		Headers:     map[string]string{"X-Trace": "{{task_id}}"},
		RequestTemplate: map[string]any{
			"customer_id": "{{var:customer_id}}",
			"amount":      "{{var:amount}}",
		},
		ResponseMapping: map[string]string{
			"order_id":     "$.id",
			"order_total":  "$.total",
		},
		Idempotency: IdempotencyConfig{Header: "Idempotency-Key", KeyTemplate: "task-{{task_id}}"},
		TimeoutSecs: 5,
	}
	h := NewHTTPHandler("http.call", cfg)
	task := mkTask(t, "abc-123", map[string]any{
		"customer_id": "cust-7",
		"amount":      1500,
	})

	res, err := h.Handle(context.Background(), task)
	if err != nil {
		t.Fatalf("handle returned error: %v", err)
	}

	if got := captured.Header.Get("Idempotency-Key"); got != "task-abc-123" {
		t.Errorf("Idempotency-Key = %q, want task-abc-123", got)
	}
	if got := captured.Header.Get("X-Trace"); got != "abc-123" {
		t.Errorf("X-Trace = %q, want abc-123", got)
	}
	var body map[string]any
	if err := json.Unmarshal(capturedBody, &body); err != nil {
		t.Fatalf("decode body: %v", err)
	}
	if body["customer_id"] != "cust-7" {
		t.Errorf("customer_id = %v, want cust-7", body["customer_id"])
	}
	// {{var:amount}} alone in a string slot should pass through as the
	// underlying number, not a stringified one.
	if amount, ok := body["amount"].(float64); !ok || amount != 1500 {
		t.Errorf("amount = %v (%T), want 1500 (number)", body["amount"], body["amount"])
	}

	if res.Kind != cw.ResultComplete {
		t.Fatalf("result kind = %v, want Complete", res.Kind)
	}
	want := map[string]string{"order_id": `"ord-42"`, "order_total": "1500"}
	for _, v := range res.Variables {
		if got := strings.TrimSpace(string(v.Value)); got != want[v.Name] {
			t.Errorf("%s = %s, want %s", v.Name, got, want[v.Name])
		}
		delete(want, v.Name)
	}
	if len(want) != 0 {
		t.Errorf("missing response_mapping vars: %v", want)
	}
}

func TestHandle_BearerAuthHeader(t *testing.T) {
	t.Setenv("TEST_TOKEN", "shhh")
	var auth string
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		auth = r.Header.Get("Authorization")
		w.WriteHeader(204)
	}))
	defer srv.Close()

	cfg := HandlerConfig{
		URLTemplate: srv.URL,
		Method:      "POST",
		Auth:        &AuthConfig{Type: "bearer", TokenEnv: "TEST_TOKEN"},
		Idempotency: IdempotencyConfig{Header: "Idempotency-Key", KeyTemplate: "task-{{task_id}}"},
		TimeoutSecs: 5,
	}
	h := NewHTTPHandler("http.call", cfg)
	if _, err := h.Handle(context.Background(), mkTask(t, "tid", nil)); err != nil {
		t.Fatalf("handle: %v", err)
	}
	if auth != "Bearer shhh" {
		t.Errorf("Authorization = %q, want Bearer shhh", auth)
	}
}

func TestHandle_BpmnErrorOn4xx(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(422)
		_, _ = w.Write([]byte(`{"error":"invalid_payload"}`))
	}))
	defer srv.Close()

	cfg := HandlerConfig{
		URLTemplate:    srv.URL,
		Method:         "POST",
		BpmnErrorOn4xx: "ORDER_REJECTED",
		Idempotency:    IdempotencyConfig{Header: "Idempotency-Key", KeyTemplate: "task-{{task_id}}"},
		TimeoutSecs:    5,
	}
	h := NewHTTPHandler("http.call", cfg)
	res, err := h.Handle(context.Background(), mkTask(t, "tid", nil))
	if err != nil {
		t.Fatalf("expected nil error (BPMN error path), got %v", err)
	}
	if res.Kind != cw.ResultBpmnError {
		t.Fatalf("kind = %v, want BpmnError", res.Kind)
	}
	if res.ErrorCode != "ORDER_REJECTED" {
		t.Errorf("error_code = %q, want ORDER_REJECTED", res.ErrorCode)
	}
}

func TestHandle_5xx_ReturnsTransientError(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(503)
	}))
	defer srv.Close()

	cfg := HandlerConfig{
		URLTemplate: srv.URL,
		Method:      "POST",
		Idempotency: IdempotencyConfig{Header: "Idempotency-Key", KeyTemplate: "task-{{task_id}}"},
		TimeoutSecs: 5,
	}
	h := NewHTTPHandler("http.call", cfg)
	_, err := h.Handle(context.Background(), mkTask(t, "tid", nil))
	if err == nil {
		t.Fatal("expected transient error for 5xx, got nil")
	}
	if !strings.Contains(err.Error(), "503") {
		t.Errorf("error %v doesn't mention 503", err)
	}
}

func TestLoadConfig_ValidatesRequiredFields(t *testing.T) {
	dir := t.TempDir()
	path := dir + "/worker.yaml"
	if err := writeFile(path, `engine:
  url: http://localhost:8080
handlers:
  http.call:
    url_template: ""
`); err != nil {
		t.Fatal(err)
	}
	if _, err := LoadConfig(path); err == nil {
		t.Fatal("LoadConfig should reject empty url_template")
	}
}

func writeFile(path, content string) error {
	return os.WriteFile(path, []byte(content), 0o600)
}
