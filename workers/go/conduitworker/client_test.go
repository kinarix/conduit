package conduitworker

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"
)

func TestFetchAndLockRoundTrip(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/api/v1/external-tasks/fetch-and-lock" {
			t.Fatalf("unexpected path: %s", r.URL.Path)
		}
		body, _ := io.ReadAll(r.Body)
		if !strings.Contains(string(body), `"worker_id":"go-1"`) {
			t.Fatalf("missing worker_id: %s", body)
		}
		if !strings.Contains(string(body), `"topic":"http.call"`) {
			t.Fatalf("missing topic: %s", body)
		}
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`[{"id":"t1","topic":"http.call","instance_id":"i1","execution_id":"e1","retries":3,"retry_count":0,"variables":[{"name":"order_id","value_type":"String","value":"ord-42"}]}]`))
	}))
	defer srv.Close()

	c := NewClient(ClientConfig{BaseURL: srv.URL, RequestTimeout: time.Second})
	tasks, err := c.FetchAndLock(context.Background(), "go-1", "http.call", 10, 30)
	if err != nil {
		t.Fatalf("FetchAndLock: %v", err)
	}
	if len(tasks) != 1 || tasks[0].ID != "t1" {
		t.Fatalf("unexpected tasks: %+v", tasks)
	}
	if v, ok := tasks[0].Variable("order_id"); !ok || string(v) != `"ord-42"` {
		t.Fatalf("variable mismatch: %s ok=%v", v, ok)
	}
}

func TestCompleteSerialisesVariables(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		body, _ := io.ReadAll(r.Body)
		var dec struct {
			WorkerID  string     `json:"worker_id"`
			Variables []Variable `json:"variables"`
		}
		if err := json.Unmarshal(body, &dec); err != nil {
			t.Fatalf("bad body: %v %s", err, body)
		}
		if dec.WorkerID != "go-1" || len(dec.Variables) != 2 {
			t.Fatalf("unexpected payload: %+v", dec)
		}
		w.WriteHeader(http.StatusNoContent)
	}))
	defer srv.Close()

	c := NewClient(ClientConfig{BaseURL: srv.URL})
	err := c.Complete(context.Background(), "task-id", "go-1", []Variable{
		VarString("status", "ok"),
		VarLong("count", 7),
	})
	if err != nil {
		t.Fatalf("Complete: %v", err)
	}
}

func TestRunnerDispatchesByTopic(t *testing.T) {
	var completeHits int
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch {
		case strings.HasSuffix(r.URL.Path, "/fetch-and-lock"):
			body, _ := io.ReadAll(r.Body)
			if strings.Contains(string(body), `"topic":"http.call"`) {
				w.Header().Set("Content-Type", "application/json")
				_, _ = w.Write([]byte(`[{"id":"t1","topic":"http.call","instance_id":"i1","execution_id":"e1","retries":3,"retry_count":0,"variables":[]}]`))
				return
			}
			_, _ = w.Write([]byte(`[]`))
		case strings.HasSuffix(r.URL.Path, "/complete"):
			completeHits++
			w.WriteHeader(http.StatusNoContent)
		default:
			w.WriteHeader(http.StatusInternalServerError)
		}
	}))
	defer srv.Close()

	c := NewClient(ClientConfig{BaseURL: srv.URL})
	r := NewRunner(c, "go-1")
	r.Register("http.call", func(ctx context.Context, task *ExternalTask) (*Result, error) {
		return Complete(VarString("status", "ok")), nil
	})

	ctx, cancel := context.WithTimeout(context.Background(), 200*time.Millisecond)
	defer cancel()
	_ = r.Run(ctx)
	if completeHits == 0 {
		t.Fatalf("expected at least one /complete call")
	}
}
