package main

import (
	"errors"
	"fmt"
	"os"
	"strings"
	"time"

	"gopkg.in/yaml.v3"
)

// WorkerConfig is the top-level worker.yaml schema.
// Mirrors workers/rust/crates/http-worker/src/config.rs.
type WorkerConfig struct {
	Engine   EngineConfig             `yaml:"engine"`
	Handlers map[string]HandlerConfig `yaml:"handlers"`
}

type EngineConfig struct {
	URL       string `yaml:"url"`
	APIKeyEnv string `yaml:"api_key_env"`
}

// HandlerConfig configures one http.call topic. Templates use a tiny
// mustache subset — {{var:NAME}} interpolates a task variable; {{task_id}}
// interpolates the engine's task id.
type HandlerConfig struct {
	URLTemplate     string                 `yaml:"url_template"`
	Method          string                 `yaml:"method"`
	Headers         map[string]string      `yaml:"headers"`
	RequestTemplate any                    `yaml:"request_template"`
	ResponseMapping map[string]string      `yaml:"response_mapping"`
	Auth            *AuthConfig            `yaml:"auth"`
	Idempotency     IdempotencyConfig      `yaml:"idempotency"`
	TimeoutSecs     uint64                 `yaml:"timeout_secs"`
	BpmnErrorOn4xx  string                 `yaml:"bpmn_error_on_4xx"`
}

// AuthConfig is a tagged union: type=bearer reads token_env,
// type=basic reads user_env + password_env. Neither secret value is
// in the YAML — we only carry the env-var names.
type AuthConfig struct {
	Type        string `yaml:"type"`
	TokenEnv    string `yaml:"token_env"`
	UserEnv     string `yaml:"user_env"`
	PasswordEnv string `yaml:"password_env"`
}

type IdempotencyConfig struct {
	Enabled     *bool  `yaml:"enabled"`
	Header      string `yaml:"header"`
	KeyTemplate string `yaml:"key_template"`
}

func (c HandlerConfig) Timeout() time.Duration {
	return time.Duration(c.TimeoutSecs) * time.Second
}

// IdempotencyEnabled returns true unless the operator set enabled=false explicitly.
func (i IdempotencyConfig) IdempotencyEnabled() bool {
	if i.Enabled == nil {
		return true
	}
	return *i.Enabled
}

// LoadConfig reads + parses worker.yaml and applies defaults.
func LoadConfig(path string) (*WorkerConfig, error) {
	raw, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("read worker config: %w", err)
	}
	var cfg WorkerConfig
	if err := yaml.Unmarshal(raw, &cfg); err != nil {
		return nil, fmt.Errorf("parse worker config: %w", err)
	}
	if cfg.Engine.URL == "" {
		return nil, errors.New("engine.url is required")
	}
	if len(cfg.Handlers) == 0 {
		return nil, errors.New("at least one handler is required")
	}
	for topic, h := range cfg.Handlers {
		if h.URLTemplate == "" {
			return nil, fmt.Errorf("handlers[%q].url_template is required", topic)
		}
		if h.Method == "" {
			h.Method = "POST"
		} else {
			h.Method = strings.ToUpper(h.Method)
		}
		if h.TimeoutSecs == 0 {
			h.TimeoutSecs = 30
		}
		if h.Idempotency.Header == "" {
			h.Idempotency.Header = "Idempotency-Key"
		}
		if h.Idempotency.KeyTemplate == "" {
			h.Idempotency.KeyTemplate = "task-{{task_id}}"
		}
		if h.Auth != nil {
			switch h.Auth.Type {
			case "bearer":
				if h.Auth.TokenEnv == "" {
					return nil, fmt.Errorf("handlers[%q].auth.token_env is required for type=bearer", topic)
				}
			case "basic":
				if h.Auth.UserEnv == "" || h.Auth.PasswordEnv == "" {
					return nil, fmt.Errorf("handlers[%q].auth requires user_env+password_env for type=basic", topic)
				}
			default:
				return nil, fmt.Errorf("handlers[%q].auth.type must be 'bearer' or 'basic', got %q", topic, h.Auth.Type)
			}
		}
		cfg.Handlers[topic] = h
	}
	return &cfg, nil
}
