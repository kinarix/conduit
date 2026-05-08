use serde_json::Value;
use std::collections::HashMap;

/// Tiny `{{var:NAME}}` and `{{task_id}}` substitution. We deliberately
/// stay below mustache because anything fancier (conditionals, loops)
/// belongs in the BPMN, not in the worker config.
pub fn render(template: &str, vars: &HashMap<String, Value>, task_id: &str) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after_open = &rest[start + 2..];
        let Some(end) = after_open.find("}}") else {
            out.push_str("{{");
            rest = after_open;
            continue;
        };
        let key = after_open[..end].trim();
        let replacement = resolve(key, vars, task_id);
        out.push_str(&replacement);
        rest = &after_open[end + 2..];
    }
    out.push_str(rest);
    out
}

fn resolve(key: &str, vars: &HashMap<String, Value>, task_id: &str) -> String {
    if key == "task_id" {
        return task_id.to_string();
    }
    if let Some(name) = key.strip_prefix("var:") {
        return match vars.get(name) {
            Some(Value::String(s)) => s.clone(),
            Some(other) => other.to_string(),
            None => String::new(),
        };
    }
    // Unknown placeholder — leave it intact so the operator notices.
    format!("{{{{{key}}}}}")
}

/// Render a JSON template, recursively replacing strings.
pub fn render_json(template: &Value, vars: &HashMap<String, Value>, task_id: &str) -> Value {
    match template {
        Value::String(s) => {
            let rendered = render(s, vars, task_id);
            // Try to keep the type — if the rendered string is exactly a
            // variable substitution and the variable's underlying type is
            // not a string, return the original JSON value.
            if let Some(v) = single_var_passthrough(s, vars) {
                v.clone()
            } else {
                Value::String(rendered)
            }
        }
        Value::Array(arr) => {
            Value::Array(arr.iter().map(|v| render_json(v, vars, task_id)).collect())
        }
        Value::Object(obj) => Value::Object(
            obj.iter()
                .map(|(k, v)| (k.clone(), render_json(v, vars, task_id)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// If `s` is exactly `{{var:NAME}}` and `NAME` is in `vars`, return the
/// underlying JSON value so non-string types (numbers, booleans) survive
/// the round-trip.
fn single_var_passthrough<'a>(s: &str, vars: &'a HashMap<String, Value>) -> Option<&'a Value> {
    let trimmed = s.trim();
    let inner = trimmed.strip_prefix("{{")?.strip_suffix("}}")?.trim();
    let name = inner.strip_prefix("var:")?.trim();
    vars.get(name)
}

/// Apply a JSON-path-ish expression to a response body. We support a
/// minimal subset: `$.foo`, `$.foo.bar`, `$.foo[0]`. Anything else
/// returns `None` so the operator notices via the missing variable.
pub fn jsonpath<'a>(expr: &str, root: &'a Value) -> Option<&'a Value> {
    let expr = expr.strip_prefix("$.")?.trim();
    if expr.is_empty() {
        return Some(root);
    }
    let mut current = root;
    for segment in expr.split('.') {
        let (name, idx) = match segment.find('[') {
            Some(i) => {
                let name = &segment[..i];
                let rest = &segment[i + 1..];
                let idx = rest.strip_suffix(']')?.parse::<usize>().ok()?;
                (name, Some(idx))
            }
            None => (segment, None),
        };
        if !name.is_empty() {
            current = current.get(name)?;
        }
        if let Some(i) = idx {
            current = current.get(i)?;
        }
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn vars(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn render_substitutes_variables() {
        let v = vars(&[("name", json!("alice"))]);
        assert_eq!(render("hi {{var:name}}!", &v, "tid"), "hi alice!");
    }

    #[test]
    fn render_substitutes_task_id() {
        let v = HashMap::new();
        assert_eq!(render("k-{{task_id}}", &v, "abc"), "k-abc");
    }

    #[test]
    fn render_unknown_placeholder_left_intact() {
        let v = HashMap::new();
        assert_eq!(render("{{var:missing}}", &v, "tid"), "");
        assert_eq!(render("{{weird}}", &v, "tid"), "{{weird}}");
    }

    #[test]
    fn render_json_preserves_number_via_passthrough() {
        let v = vars(&[("amount", json!(1000))]);
        let template = json!({ "amount": "{{var:amount}}" });
        assert_eq!(render_json(&template, &v, "tid"), json!({ "amount": 1000 }));
    }

    #[test]
    fn render_json_interpolates_inside_string() {
        let v = vars(&[("name", json!("alice"))]);
        let template = json!({ "greeting": "hi {{var:name}}" });
        assert_eq!(
            render_json(&template, &v, "tid"),
            json!({ "greeting": "hi alice" })
        );
    }

    #[test]
    fn jsonpath_simple_field() {
        let body = json!({ "id": "abc" });
        assert_eq!(jsonpath("$.id", &body), Some(&json!("abc")));
    }

    #[test]
    fn jsonpath_nested() {
        let body = json!({ "a": { "b": 42 } });
        assert_eq!(jsonpath("$.a.b", &body), Some(&json!(42)));
    }

    #[test]
    fn jsonpath_array_index() {
        let body = json!({ "items": [10, 20, 30] });
        assert_eq!(jsonpath("$.items[1]", &body), Some(&json!(20)));
    }

    #[test]
    fn jsonpath_missing_returns_none() {
        let body = json!({});
        assert_eq!(jsonpath("$.missing", &body), None);
    }
}
