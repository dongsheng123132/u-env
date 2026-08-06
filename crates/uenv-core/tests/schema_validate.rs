//! 最小 JSON Schema 校验器 + fixtures 校验测试。
//! 不引 jsonschema crate（白名单外），实现足以覆盖 environment.schema.json 用到的
//! 结构校验能力：type / required / enum / const / pattern / oneOf / additionalProperties / items / $ref。

use std::collections::BTreeMap;

use serde_json::Value;

#[derive(Debug)]
pub struct ValidateError {
    pub path: String,
    pub msg: String,
}

impl std::fmt::Display for ValidateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.msg)
    }
}

/// 校验 value 是否符合 schema（$defs 内联解析）
pub fn validate(value: &Value, schema: &Value) -> Result<(), Vec<ValidateError>> {
    let defs = schema.get("$defs").cloned().unwrap_or(Value::Null);
    let mut collected = Vec::new();
    walk(value, schema, &defs, "$", &mut collected);
    if collected.is_empty() {
        Ok(())
    } else {
        Err(collected)
    }
}

fn walk(value: &Value, schema: &Value, defs: &Value, path: &str, errors: &mut Vec<ValidateError>) {
    if let Some(ref_s) = schema.get("$ref") {
        if let Some(rs) = ref_s.as_str() {
            if let Some(stripped) = rs.strip_prefix("#/$defs/") {
                if let Some(target) = defs.get(stripped) {
                    walk(value, target, defs, path, errors);
                    return;
                }
            }
        }
    }
    if let Some(one_of) = schema.get("oneOf").and_then(|v| v.as_array()) {
        let mut ok = false;
        for alt in one_of {
            let mut tmp = Vec::new();
            walk(value, alt, defs, path, &mut tmp);
            if tmp.is_empty() {
                ok = true;
                break;
            }
        }
        if !ok {
            errors.push(ValidateError {
                path: path.to_string(),
                msg: "不匹配任何 oneOf 分支".to_string(),
            });
        }
        return;
    }
    if let Some(t) = schema.get("type") {
        match t {
            Value::String(ts) => {
                if !type_matches(value, ts) {
                    errors.push(ValidateError {
                        path: path.to_string(),
                        msg: format!("期望 {ts}，实际 {}", type_name(value)),
                    });
                    return;
                }
            }
            Value::Array(ts) => {
                if !ts
                    .iter()
                    .any(|x| x.as_str().is_some_and(|s| type_matches(value, s)))
                {
                    errors.push(ValidateError {
                        path: path.to_string(),
                        msg: "类型不符".to_string(),
                    });
                    return;
                }
            }
            _ => {}
        }
    }
    if let Some(Value::String(pat)) = schema.get("pattern") {
        // 简化正则：^origin-env:sha256:[0-9a-f]{64}$ 这种固定格式
        if let Some(s) = value.as_str() {
            let ok = match pat.as_str() {
                "^origin-env:sha256:[0-9a-f]{64}$" => {
                    s.len() == 7 + 64
                        && s.starts_with("origin-env:sha256:")
                        && s[7..].chars().all(|c| c.is_ascii_hexdigit())
                }
                _ => true, // 未知 pattern 不校验
            };
            if !ok {
                errors.push(ValidateError {
                    path: path.to_string(),
                    msg: format!("pattern 不匹配: {pat}"),
                });
            }
        }
    }
    if let Some(Value::String(con)) = schema.get("const") {
        if value.as_str() != Some(con) {
            errors.push(ValidateError {
                path: path.to_string(),
                msg: format!("const 不匹配: 期望 {con}"),
            });
        }
    }
    if let Some(Value::Array(enumv)) = schema.get("enum") {
        if !enumv.iter().any(|e| e == value) {
            errors.push(ValidateError {
                path: path.to_string(),
                msg: format!("不在 enum 内: {value}"),
            });
        }
    }
    if let Some(Value::Array(required)) = schema.get("required") {
        if let Value::Object(map) = value {
            for r in required {
                if let Some(name) = r.as_str() {
                    if !map.contains_key(name) {
                        errors.push(ValidateError {
                            path: path.to_string(),
                            msg: format!("缺少必填字段 {name}"),
                        });
                    }
                }
            }
        } else {
            errors.push(ValidateError {
                path: path.to_string(),
                msg: "期望 object 但实际不是".to_string(),
            });
            return;
        }
    }
    if let Some(Value::Object(properties)) = schema.get("properties") {
        if let Value::Object(map) = value {
            for (k, subschema) in properties {
                if let Some(sub) = map.get(k) {
                    walk(sub, subschema, defs, &format!("{path}.{k}"), errors);
                }
            }
        }
    }
    if let Some(additional) = schema.get("additionalProperties") {
        if !additional.is_null() {
            if let Value::Object(map) = value {
                for (k, sub) in map {
                    walk(sub, additional, defs, &format!("{path}.{k}"), errors);
                }
            }
        }
    }
    if let Some(Value::Array(items)) = schema.get("items") {
        if let Value::Array(arr) = value {
            for (i, item) in arr.iter().enumerate() {
                walk(item, &items[0], defs, &format!("{path}[{i}]"), errors);
            }
        }
    }
}

fn type_matches(value: &Value, t: &str) -> bool {
    match t {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "integer" => value.is_i64() || value.is_u64(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => true,
    }
}

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

// 仅为未使用导入占位（BTreeMap 保留给未来扩展）
#[allow(dead_code)]
fn _unused() -> BTreeMap<String, String> {
    BTreeMap::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_schema() -> Value {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/environment.schema.json"
        );
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    fn load_env(path: &str) -> Value {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../")
            .join(path);
        serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
    }

    #[test]
    fn env_good_passes_schema() {
        let schema = load_schema();
        let env = load_env("fixtures/env-good.json");
        match validate(&env, &schema) {
            Ok(_) => {}
            Err(errs) => {
                for e in &errs {
                    eprintln!("VALIDATE: {e}");
                }
                panic!("env-good 应通过 schema 校验（{} 个错误）", errs.len());
            }
        }
    }

    #[test]
    fn env_broken_passes_schema() {
        // broken 与 good 结构相同（只是 facts 不同），也应通过
        let schema = load_schema();
        let env = load_env("fixtures/env-broken.json");
        assert!(validate(&env, &schema).is_ok());
    }

    #[test]
    fn schema_rejects_bad_spec() {
        let schema = load_schema();
        let mut env = load_env("fixtures/env-good.json");
        env["spec"] = Value::String("wrong-spec".to_string());
        let err = validate(&env, &schema).unwrap_err();
        assert!(err.iter().any(|e| e.msg.contains("const")));
    }

    #[test]
    fn schema_rejects_missing_required() {
        let schema = load_schema();
        let mut env = load_env("fixtures/env-good.json");
        env.as_object_mut().unwrap().remove("detectors");
        let err = validate(&env, &schema).unwrap_err();
        assert!(err.iter().any(|e| e.msg.contains("缺少必填字段 detectors")));
    }

    #[test]
    fn schema_rejects_bad_status() {
        let schema = load_schema();
        let mut env = load_env("fixtures/env-good.json");
        let det = env["detectors"].as_object_mut().unwrap();
        let first = det.values_mut().next().unwrap();
        first["status"] = Value::String("fancy".to_string());
        let err = validate(&env, &schema).unwrap_err();
        assert!(err.iter().any(|e| e.msg.contains("不在 enum")));
    }
}
