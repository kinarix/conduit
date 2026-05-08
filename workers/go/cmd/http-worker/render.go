package main

import (
	"fmt"
	"strconv"
	"strings"
)

// render expands {{var:NAME}} and {{task_id}} placeholders in template.
// Mirrors workers/rust/crates/http-worker/src/render.rs render().
//
// Unknown placeholders (anything other than var:NAME and task_id) are left
// in place so the operator notices via the rendered request.
func render(template string, vars map[string]any, taskID string) string {
	var out strings.Builder
	out.Grow(len(template))
	rest := template
	for {
		i := strings.Index(rest, "{{")
		if i < 0 {
			out.WriteString(rest)
			return out.String()
		}
		out.WriteString(rest[:i])
		afterOpen := rest[i+2:]
		j := strings.Index(afterOpen, "}}")
		if j < 0 {
			out.WriteString("{{")
			rest = afterOpen
			continue
		}
		key := strings.TrimSpace(afterOpen[:j])
		out.WriteString(resolve(key, vars, taskID))
		rest = afterOpen[j+2:]
	}
}

func resolve(key string, vars map[string]any, taskID string) string {
	if key == "task_id" {
		return taskID
	}
	if name, ok := strings.CutPrefix(key, "var:"); ok {
		v, present := vars[name]
		if !present {
			return ""
		}
		return scalarString(v)
	}
	// Unknown placeholder — leave intact.
	return "{{" + key + "}}"
}

func scalarString(v any) string {
	switch t := v.(type) {
	case nil:
		return ""
	case string:
		return t
	case bool:
		return strconv.FormatBool(t)
	case float64:
		// JSON numbers decode as float64; keep integer-valued ones integer-formatted.
		if t == float64(int64(t)) {
			return strconv.FormatInt(int64(t), 10)
		}
		return strconv.FormatFloat(t, 'f', -1, 64)
	case int, int32, int64:
		return fmt.Sprintf("%d", t)
	default:
		return fmt.Sprintf("%v", t)
	}
}

// renderJSON walks template recursively, expanding placeholders inside
// strings. Strings that are exactly `{{var:NAME}}` for a non-string variable
// retain the underlying type via passthrough — matches the Rust SDK's
// single_var_passthrough behaviour.
func renderJSON(template any, vars map[string]any, taskID string) any {
	switch t := template.(type) {
	case string:
		if v, ok := singleVarPassthrough(t, vars); ok {
			return v
		}
		return render(t, vars, taskID)
	case []any:
		out := make([]any, len(t))
		for i, v := range t {
			out[i] = renderJSON(v, vars, taskID)
		}
		return out
	case map[string]any:
		out := make(map[string]any, len(t))
		for k, v := range t {
			out[k] = renderJSON(v, vars, taskID)
		}
		return out
	default:
		return t
	}
}

// singleVarPassthrough: if s is exactly `{{var:NAME}}` (with optional
// whitespace) and NAME is in vars, return the underlying value.
func singleVarPassthrough(s string, vars map[string]any) (any, bool) {
	trimmed := strings.TrimSpace(s)
	inner, ok := strings.CutPrefix(trimmed, "{{")
	if !ok {
		return nil, false
	}
	inner, ok = strings.CutSuffix(inner, "}}")
	if !ok {
		return nil, false
	}
	name, ok := strings.CutPrefix(strings.TrimSpace(inner), "var:")
	if !ok {
		return nil, false
	}
	v, present := vars[strings.TrimSpace(name)]
	if !present {
		return nil, false
	}
	return v, true
}

// jsonpath applies a minimal JSON-path subset to root: $.foo, $.foo.bar,
// $.foo[0]. Anything else returns ok=false so the operator notices via a
// missing variable.
func jsonpath(expr string, root any) (any, bool) {
	expr, ok := strings.CutPrefix(expr, "$.")
	if !ok {
		return nil, false
	}
	if expr == "" {
		return root, true
	}
	current := root
	for _, segment := range strings.Split(expr, ".") {
		name := segment
		var idx *int
		if i := strings.Index(segment, "["); i >= 0 {
			name = segment[:i]
			rest, ok := strings.CutSuffix(segment[i+1:], "]")
			if !ok {
				return nil, false
			}
			n, err := strconv.Atoi(rest)
			if err != nil {
				return nil, false
			}
			idx = &n
		}
		if name != "" {
			obj, ok := current.(map[string]any)
			if !ok {
				return nil, false
			}
			current, ok = obj[name]
			if !ok {
				return nil, false
			}
		}
		if idx != nil {
			arr, ok := current.([]any)
			if !ok {
				return nil, false
			}
			if *idx < 0 || *idx >= len(arr) {
				return nil, false
			}
			current = arr[*idx]
		}
	}
	return current, true
}
