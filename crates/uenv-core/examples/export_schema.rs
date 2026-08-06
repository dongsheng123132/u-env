//! 从 uenv-core 数据结构导出 JSON Schema（规格：schemas/environment.schema.json 由 uenv-core 导出，勿手写）。
//!
//! 依赖白名单里没有 schemars，这里手工构造 schema（与 core 的 serde 派生保持一致），
//! 输出到 schemas/environment.schema.json。
//!
//! 用法：cargo run -p uenv-core --example export_schema
//! 校验：fixtures/env-good.json 必须能通过基本结构校验（测试在 uenv-core tests/schema_validate.rs）。

use std::fs;
use std::path::Path;

/// 手工构造 environment.schema.json（与 uenv-core 的 serde 派生字段一致）
fn build_schema() -> serde_json::Value {
    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://u-king.org/schemas/environment.schema.json",
        "title": "Origin Environment",
        "description": "本境协议 (Origin Environment Protocol) v0.1 环境对象。由 uenv scan 产出。",
        "type": "object",
        "required": ["spec", "generated_at", "uenv_version", "identity", "detectors"],
        "properties": {
            "spec": {
                "type": "string",
                "const": "origin-environment/v0.1",
                "description": "协议版本标记，diff/load 时校验"
            },
            "generated_at": { "type": "string", "description": "RFC3339 时间戳，不进指纹" },
            "uenv_version": { "type": "string" },
            "identity": {
                "type": "object",
                "required": ["host_alias", "os", "architecture"],
                "properties": {
                    "host_alias": { "type": "string" },
                    "os": { "$ref": "#/$defs/operating_system" },
                    "architecture": { "enum": ["x64", "arm64", "x86", "unknown"] },
                    "project": {
                        "oneOf": [
                            { "$ref": "#/$defs/project_manifest" },
                            { "type": "null" }
                        ]
                    }
                }
            },
            "detectors": {
                "type": "object",
                "additionalProperties": { "$ref": "#/$defs/detector_record" },
                "description": "key = detector id"
            },
            "fingerprint": {
                "oneOf": [
                    { "$ref": "#/$defs/environment_fingerprint" },
                    { "type": "null" }
                ]
            }
        },
        "$defs": {
            "operating_system": {
                "type": "object",
                "required": ["family", "product_name", "product_name_raw", "version", "build"],
                "properties": {
                    "family": { "type": "string" },
                    "product_name": { "type": "string" },
                    "product_name_raw": { "type": "string" },
                    "version": { "type": "string" },
                    "build": { "type": "integer" },
                    "ubr": { "type": ["integer", "null"] },
                    "edition": { "type": ["string", "null"] },
                    "display_version": { "type": ["string", "null"] }
                }
            },
            "detector_record": {
                "type": "object",
                "required": ["id", "layer", "title", "status", "summary", "facts"],
                "properties": {
                    "id": { "type": "string" },
                    "layer": { "enum": ["host", "toolchain", "project"] },
                    "title": { "type": "string" },
                    "status": { "enum": ["ok", "absent", "degraded", "error", "skipped"] },
                    "summary": { "type": "string" },
                    "facts": { "type": "object", "additionalProperties": { "$ref": "#/$defs/fact_value" } },
                    "volatile": { "type": "object", "additionalProperties": { "$ref": "#/$defs/fact_value" } },
                    "evidence": { "type": "array", "items": { "$ref": "#/$defs/evidence" } },
                    "elapsed_ms": { "type": "integer" }
                }
            },
            "fact_value": {
                "description": "有限值类型，带类型标记（与 serde 派生 snake_case 一致）",
                "oneOf": [
                    { "type": "object", "required": ["str"], "properties": { "str": { "type": "string" } } },
                    { "type": "object", "required": ["int"], "properties": { "int": { "type": "integer" } } },
                    { "type": "object", "required": ["bool"], "properties": { "bool": { "type": "boolean" } } },
                    { "type": "object", "required": ["version"], "properties": { "version": { "type": "string" } } },
                    { "type": "object", "required": ["path"], "properties": { "path": { "type": "string" } } },
                    { "type": "object", "required": ["list"], "properties": { "list": { "type": "array", "items": { "$ref": "#/$defs/fact_value" } } } },
                    { "type": "object", "required": ["set"], "properties": { "set": { "type": "array", "items": { "$ref": "#/$defs/fact_value" } } } },
                    { "type": "object", "required": ["map"], "properties": { "map": { "type": "object", "additionalProperties": { "$ref": "#/$defs/fact_value" } } } }
                ]
            },
            "evidence": {
                "type": "object",
                "required": ["kind", "source", "excerpt"],
                "properties": {
                    "kind": { "enum": ["command", "registry", "file", "env"] },
                    "source": { "type": "string" },
                    "exit_code": { "type": ["integer", "null"] },
                    "excerpt": { "type": "string" }
                }
            },
            "project_manifest": {
                "type": "object",
                "required": ["root", "kind"],
                "properties": {
                    "root": { "type": "string" },
                    "kind": { "type": "array", "items": { "enum": ["tauri", "electron", "node", "rust", "dotnet", "winui", "python", "unknown"] } },
                    "declared_toolchains": { "type": "object", "additionalProperties": { "type": "string" } },
                    "lockfiles": { "type": "object", "additionalProperties": { "type": "string" } },
                    "git": { "type": ["object", "null"] }
                }
            },
            "environment_fingerprint": {
                "type": "object",
                "required": ["host", "toolchain", "full"],
                "properties": {
                    "host": { "type": "string", "pattern": "^origin-env:sha256:[0-9a-f]{64}$" },
                    "toolchain": { "type": "string", "pattern": "^origin-env:sha256:[0-9a-f]{64}$" },
                    "project": { "type": ["string", "null"] },
                    "full": { "type": "string", "pattern": "^origin-env:sha256:[0-9a-f]{64}$" }
                }
            }
        }
    })
}

fn main() {
    let schema = build_schema();
    let json = serde_json::to_string_pretty(&schema).expect("schema serialization");
    let out_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/environment.schema.json");
    fs::create_dir_all(out_path.parent().unwrap()).expect("create schemas dir");
    fs::write(&out_path, json).expect("write schema");
    println!("schema written to {}", out_path.display());
}
