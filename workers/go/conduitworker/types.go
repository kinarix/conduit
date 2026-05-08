// Package conduitworker is the Go SDK for the Conduit external-task API.
//
// See workers/PROTOCOL.md for the wire contract.
package conduitworker

import (
	"encoding/json"
	"time"
)

// ExternalTask is one task delivered by /external-tasks/fetch-and-lock.
type ExternalTask struct {
	ID           string     `json:"id"`
	Topic        *string    `json:"topic"`
	InstanceID   string     `json:"instance_id"`
	ExecutionID  string     `json:"execution_id"`
	LockedUntil  *time.Time `json:"locked_until"`
	Retries      int        `json:"retries"`
	RetryCount   int        `json:"retry_count"`
	Variables    []Variable `json:"variables"`
}

// Variable lookup convenience.
func (t *ExternalTask) Variable(name string) (json.RawMessage, bool) {
	for _, v := range t.Variables {
		if v.Name == name {
			return v.Value, true
		}
	}
	return nil, false
}

// Variable is the wire shape of a process variable.
type Variable struct {
	Name      string          `json:"name"`
	ValueType string          `json:"value_type"`
	Value     json.RawMessage `json:"value"`
}

// VarString constructs a String-typed Variable.
func VarString(name, value string) Variable {
	v, _ := json.Marshal(value)
	return Variable{Name: name, ValueType: "String", Value: v}
}

// VarLong constructs a Long-typed Variable (int64).
func VarLong(name string, value int64) Variable {
	v, _ := json.Marshal(value)
	return Variable{Name: name, ValueType: "Long", Value: v}
}

// VarDouble constructs a Double-typed Variable.
func VarDouble(name string, value float64) Variable {
	v, _ := json.Marshal(value)
	return Variable{Name: name, ValueType: "Double", Value: v}
}

// VarBool constructs a Boolean-typed Variable.
func VarBool(name string, value bool) Variable {
	v, _ := json.Marshal(value)
	return Variable{Name: name, ValueType: "Boolean", Value: v}
}

// VarJSON constructs a Json-typed Variable from any JSON-serialisable value.
func VarJSON(name string, value any) (Variable, error) {
	v, err := json.Marshal(value)
	if err != nil {
		return Variable{}, err
	}
	return Variable{Name: name, ValueType: "Json", Value: v}, nil
}

// VarNull constructs a Null-typed Variable.
func VarNull(name string) Variable {
	return Variable{Name: name, ValueType: "Null", Value: json.RawMessage("null")}
}
