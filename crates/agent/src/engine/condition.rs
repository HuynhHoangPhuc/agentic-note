use agentic_note_core::error::{AgenticError, Result};
use serde_json::Value;
use std::collections::HashMap;

/// Evaluate a simple condition expression against stage outputs.
///
/// Supported syntax:
/// - `stage_name.output.field == "value"`
/// - `stage_name.output.field != "value"`
///
/// The left-hand side must be in the form `<output_key>.<field>` where
/// `output_key` matches a key in `outputs` and `field` is a JSON object key.
///
/// Returns `Ok(true)` / `Ok(false)`, or `Err` for unsupported expressions.
pub fn evaluate_condition(expr: &str, outputs: &HashMap<String, Value>) -> Result<bool> {
    let expr = expr.trim();

    // Detect operator.
    let (lhs, op, rhs) = if let Some(pos) = expr.find("!=") {
        (&expr[..pos], "!=", &expr[pos + 2..])
    } else if let Some(pos) = expr.find("==") {
        (&expr[..pos], "==", &expr[pos + 2..])
    } else {
        return Err(AgenticError::Pipeline(format!(
            "unsupported condition (no == or !=): {expr}"
        )));
    };

    let lhs = lhs.trim();
    let rhs = rhs.trim().trim_matches('"');

    // LHS must be `output_key.field`.
    let (output_key, field) = lhs.split_once('.').ok_or_else(|| {
        AgenticError::Pipeline(format!(
            "condition LHS must be 'output_key.field', got: {lhs}"
        ))
    })?;

    let actual = outputs
        .get(output_key)
        .and_then(|v| v.get(field))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    Ok(match op {
        "==" => actual == rhs,
        "!=" => actual != rhs,
        _ => unreachable!(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_outputs() -> HashMap<String, Value> {
        let mut m = HashMap::new();
        m.insert("keywords".into(), json!({ "status": "ok" }));
        m.insert("summary".into(), json!({ "lang": "en" }));
        m
    }

    #[test]
    fn eq_true() {
        let out = make_outputs();
        assert!(evaluate_condition(r#"keywords.status == "ok""#, &out).unwrap());
    }

    #[test]
    fn eq_false() {
        let out = make_outputs();
        assert!(!evaluate_condition(r#"keywords.status == "fail""#, &out).unwrap());
    }

    #[test]
    fn neq_true() {
        let out = make_outputs();
        assert!(evaluate_condition(r#"keywords.status != "fail""#, &out).unwrap());
    }

    #[test]
    fn missing_key_is_empty_string() {
        let out = make_outputs();
        // Missing field treats as "" so != "anything" is true.
        assert!(evaluate_condition(r#"keywords.missing != "x""#, &out).unwrap());
    }

    #[test]
    fn unsupported_expr_returns_err() {
        let out = make_outputs();
        assert!(evaluate_condition("keywords.status > 0", &out).is_err());
    }

    #[test]
    fn lhs_without_dot_returns_err() {
        let out = make_outputs();
        assert!(evaluate_condition(r#"keywords == "ok""#, &out).is_err());
    }
}
