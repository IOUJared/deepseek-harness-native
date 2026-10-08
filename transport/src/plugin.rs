//! Bounded native plugin inventory and live-settings projection for DSH 0.2.1-alpha.1.
//! Schemastery reference graphs and raw config exist only in private wire DTOs.
use crate::{Error, Result, validate_json};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashSet;

const MAX_ROWS: usize = 512;
const MAX_FIELDS: usize = 512;
const MAX_TEXT: usize = 2048;
const MAX_PATH: usize = 16;
const MAX_ITEMS: usize = 32768;
const MAX_BYTES: usize = 1024 * 1024;
const MAX_REVISION: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, PartialEq)]
pub struct PluginInventorySnapshot {
    pub entries: Vec<PluginEntry>,
    /// Management is deliberately unavailable in this read-only native inventory.
    pub management_available: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct PluginEntry {
    pub entry_id: String,
    pub module_name: String,
    pub enabled: bool,
    pub fiber_phase: FiberPhase,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FiberPhase {
    Pending,
    Loading,
    Active,
    Failed,
    Unloading,
    Absent,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SettingsDescribeValue {
    pub writable: bool,
    pub namespaces: Vec<SettingsNamespaceView>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SettingsNamespaceView {
    pub ns: String,
    pub revision: u64,
    pub auto_generate: bool,
    pub fields: Vec<SettingsField>,
    pub unsupported_fields: usize,
    pub secret_fields: usize,
}
#[derive(Clone, Debug, PartialEq)]
pub struct SettingsField {
    pub path: Vec<String>,
    pub label: String,
    pub kind: SettingsFieldKind,
    pub value: Option<SettingsScalar>,
    pub overridden: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub enum SettingsFieldKind {
    Bool,
    Number {
        min: Option<f64>,
        max: Option<f64>,
        integer: bool,
    },
    String {
        max_length: usize,
    },
}
#[derive(Clone, PartialEq)]
pub enum SettingsScalar {
    Bool(bool),
    Number(f64),
    String(String),
}
impl std::fmt::Debug for SettingsScalar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Bool(_) => "Bool([REDACTED])",
            Self::Number(_) => "Number([REDACTED])",
            Self::String(_) => "String([REDACTED])",
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireInventory {
    entries: Vec<WireEntry>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireEntry {
    entry_id: String,
    module_name: String,
    enabled: bool,
    #[serde(deserialize_with = "wire_phase")]
    fiber_phase: Option<FiberPhase>,
}
fn wire_phase<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<FiberPhase>, D::Error> {
    let phase = Option::<FiberPhase>::deserialize(d)?;
    if phase == Some(FiberPhase::Absent) {
        return Err(serde::de::Error::custom("invalid phase"));
    }
    Ok(phase)
}
#[derive(Deserialize)]
struct WireSettings {
    writable: bool,
    namespaces: Vec<WireNamespace>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireNamespace {
    ns: String,
    revision: u64,
    auto_generate: bool,
    schema: Value,
    value: Value,
    #[serde(default)]
    base: Value,
    #[serde(default)]
    user: Value,
    applies: String,
    secrets: Vec<WireSecret>,
}
#[derive(Deserialize)]
struct WireSecret {
    path: Vec<String>,
    set: bool,
}

fn bounded(value: &Value) -> Result<()> {
    validate_json(value, MAX_ITEMS)?;
    if serde_json::to_vec(value)
        .map_err(|_| Error::InvalidJson)?
        .len()
        > MAX_BYTES
    {
        return Err(Error::Oversize);
    }
    Ok(())
}
fn text(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}
fn safe_path(path: &[String]) -> bool {
    !path.is_empty() && path.len() <= MAX_PATH && path.iter().all(|s| text(s, 128) && !sensitive(s))
}
fn sensitive(key: &str) -> bool {
    let k: String = key
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    [
        "secret",
        "apikey",
        "token",
        "password",
        "passwd",
        "authorization",
        "credential",
        "privatekey",
        "accesskey",
        "cookie",
        "auth",
        "__proto__",
        "constructor",
        "prototype",
        "jsexpr",
        "preset",
        "restrict",
        "guard",
        "permission",
        "approval",
        "sandbox",
        "policy",
    ]
    .iter()
    .any(|part| k.contains(&part.replace('_', "")))
}
fn at<'a>(value: &'a Value, path: &[String]) -> Option<&'a Value> {
    let mut v = value;
    for key in path {
        v = v.as_object()?.get(key)?;
    }
    Some(v)
}
fn expression_on_path(value: &Value, path: &[String]) -> bool {
    let mut v = value;
    for key in std::iter::once(None).chain(path.iter().map(Some)) {
        if let Some(key) = key {
            match v.as_object().and_then(|o| o.get(key)) {
                Some(next) => v = next,
                None => return false,
            }
        }
        if v.as_object().is_some_and(|o| o.contains_key("__jsExpr")) {
            return true;
        }
    }
    false
}
impl SettingsFieldKind {
    pub fn accepts(&self, value: &SettingsScalar) -> bool {
        match (self, value) {
            (Self::Bool, SettingsScalar::Bool(_)) => true,
            (Self::Number { min, max, integer }, SettingsScalar::Number(v)) => {
                v.is_finite()
                    && (v.fract() != 0.0 || v.abs() <= MAX_REVISION as f64)
                    && (!integer || v.fract() == 0.0)
                    && min.is_none_or(|m| *v >= m)
                    && max.is_none_or(|m| *v <= m)
            }
            (Self::String { max_length }, SettingsScalar::String(v)) => {
                v.len() <= (*max_length).min(MAX_TEXT) && !v.chars().any(char::is_control)
            }
            _ => false,
        }
    }
}
fn scalar(value: &Value, kind: &SettingsFieldKind) -> Option<SettingsScalar> {
    let v = match value {
        Value::Bool(v) => SettingsScalar::Bool(*v),
        Value::Number(v) => SettingsScalar::Number(v.as_f64()?),
        Value::String(v) => SettingsScalar::String(v.clone()),
        _ => return None,
    };
    kind.accepts(&v).then_some(v)
}

pub(crate) fn project_inventory(value: Value) -> Result<PluginInventorySnapshot> {
    bounded(&value)?;
    let wire: WireInventory = serde_json::from_value(value).map_err(|_| Error::InvalidDto)?;
    if wire.entries.len() > MAX_ROWS {
        return Err(Error::Oversize);
    }
    let mut ids = HashSet::new();
    let entries = wire
        .entries
        .into_iter()
        .map(|e| {
            if !text(&e.entry_id, 512)
                || !text(&e.module_name, 512)
                || !ids.insert(e.entry_id.clone())
            {
                return Err(Error::InvalidDto);
            }
            Ok(PluginEntry {
                entry_id: e.entry_id,
                module_name: e.module_name,
                enabled: e.enabled,
                fiber_phase: e.fiber_phase.unwrap_or(FiberPhase::Absent),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    if entries
        .iter()
        .map(|e| e.entry_id.len() + e.module_name.len() + 64)
        .sum::<usize>()
        > MAX_BYTES
    {
        return Err(Error::Oversize);
    }
    Ok(PluginInventorySnapshot {
        entries,
        management_available: false,
    })
}
pub(crate) fn project_settings(value: Value) -> Result<SettingsDescribeValue> {
    bounded(&value)?;
    let wire: WireSettings = serde_json::from_value(value).map_err(|_| Error::InvalidDto)?;
    if wire.namespaces.len() > MAX_ROWS {
        return Err(Error::Oversize);
    }
    let mut ids = HashSet::new();
    let mut total_fields = 0;
    let mut traversal_budget = MAX_ITEMS;
    let namespaces = wire
        .namespaces
        .into_iter()
        .map(|n| {
            if !ids.insert(n.ns.clone()) {
                return Err(Error::InvalidDto);
            }
            let n = project_wire_namespace(n, &mut traversal_budget)?;
            total_fields += n.fields.len();
            if total_fields > MAX_FIELDS {
                return Err(Error::Oversize);
            }
            Ok(n)
        })
        .collect::<Result<Vec<_>>>()?;
    if namespaces.iter().map(projected_bytes).sum::<usize>() > MAX_BYTES {
        return Err(Error::Oversize);
    }
    Ok(SettingsDescribeValue {
        writable: wire.writable,
        namespaces,
    })
}
pub(crate) fn project_namespace(value: Value) -> Result<SettingsNamespaceView> {
    bounded(&value)?;
    let mut budget = MAX_ITEMS;
    project_wire_namespace(
        serde_json::from_value(value).map_err(|_| Error::InvalidDto)?,
        &mut budget,
    )
}
fn project_wire_namespace(w: WireNamespace, budget: &mut usize) -> Result<SettingsNamespaceView> {
    if !text(&w.ns, 512)
        || w.revision > MAX_REVISION
        || w.applies != "live"
        || w.secrets.len() > MAX_FIELDS
    {
        return Err(Error::InvalidDto);
    }
    for s in &w.secrets {
        if s.path.len() > MAX_PATH || s.path.iter().any(|v| !text(v, 128)) {
            return Err(Error::InvalidDto);
        }
        let _ = s.set;
    }
    let mut out = SettingsNamespaceView {
        ns: w.ns.clone(),
        revision: w.revision,
        auto_generate: w.auto_generate,
        fields: Vec::new(),
        unsupported_fields: 0,
        secret_fields: w.secrets.len(),
    };
    if !w.auto_generate {
        out.unsupported_fields = 1;
        return Ok(out);
    }
    let Some(refs) = w.schema.get("refs").and_then(Value::as_object) else {
        out.unsupported_fields = 1;
        return Ok(out);
    };
    let Some(uid) = w.schema.get("uid").and_then(Value::as_u64) else {
        out.unsupported_fields = 1;
        return Ok(out);
    };
    fn walk(
        w: &WireNamespace,
        refs: &serde_json::Map<String, Value>,
        id: u64,
        path: &mut Vec<String>,
        stack: &mut Vec<u64>,
        out: &mut SettingsNamespaceView,
        budget: &mut usize,
    ) -> Result<()> {
        *budget = budget.checked_sub(1).ok_or(Error::Oversize)?;
        if stack.len() > MAX_PATH || stack.contains(&id) {
            out.unsupported_fields += 1;
            return Ok(());
        }
        let Some(node) = refs.get(&id.to_string()).and_then(Value::as_object) else {
            out.unsupported_fields += 1;
            return Ok(());
        };
        if !node
            .keys()
            .all(|k| ["type", "meta", "uid", "dict"].contains(&k.as_str()))
        {
            out.unsupported_fields += 1;
            return Ok(());
        }
        let Some(meta) = node.get("meta").and_then(Value::as_object) else {
            out.unsupported_fields += 1;
            return Ok(());
        };
        let role = meta.get("role");
        if role == Some(&json!("secret")) {
            if !w.secrets.iter().any(|s| s.path == *path) {
                out.secret_fields += 1;
            }
            return Ok(());
        }
        if role.is_some()
            || meta.get("hidden").is_some_and(|v| v != &Value::Bool(false))
            || meta
                .get("disabled")
                .is_some_and(|v| v != &Value::Bool(false))
            || path.iter().any(|v| sensitive(v))
        {
            out.unsupported_fields += 1;
            return Ok(());
        }
        if !meta.keys().all(|k| {
            [
                "description",
                "comment",
                "default",
                "required",
                "hidden",
                "disabled",
                "collapse",
                "min",
                "max",
                "step",
            ]
            .contains(&k.as_str())
        }) {
            out.unsupported_fields += 1;
            return Ok(());
        }
        if node.get("type").and_then(Value::as_str) == Some("object") {
            if meta.contains_key("min") || meta.contains_key("max") || meta.contains_key("step") {
                out.unsupported_fields += 1;
                return Ok(());
            }
            let Some(dict) = node.get("dict").and_then(Value::as_object) else {
                out.unsupported_fields += 1;
                return Ok(());
            };
            stack.push(id);
            for (key, child) in dict {
                *budget = budget.checked_sub(1).ok_or(Error::Oversize)?;
                if !text(key, 128) || path.len() >= MAX_PATH {
                    out.unsupported_fields += 1;
                    continue;
                }
                path.push(key.clone());
                if let Some(child) = child.as_u64() {
                    walk(w, refs, child, path, stack, out, budget)?;
                } else {
                    out.unsupported_fields += 1;
                }
                path.pop();
            }
            stack.pop();
            return Ok(());
        }
        if !safe_path(path)
            || w.secrets
                .iter()
                .any(|s| path.starts_with(&s.path) || s.path.starts_with(path))
            || [&w.value, &w.base, &w.user]
                .iter()
                .any(|v| expression_on_path(v, path))
        {
            out.unsupported_fields += 1;
            return Ok(());
        }
        let kind = match node.get("type").and_then(Value::as_str) {
            Some("boolean") if !["min", "max", "step"].iter().any(|k| meta.contains_key(*k)) => {
                SettingsFieldKind::Bool
            }
            Some("number") => {
                let bound = |key: &str| -> Option<Option<f64>> {
                    match meta.get(key) {
                        None => Some(None),
                        Some(v) => v.as_f64().filter(|v| v.is_finite()).map(Some),
                    }
                };
                let (Some(min), Some(max)) = (bound("min"), bound("max")) else {
                    out.unsupported_fields += 1;
                    return Ok(());
                };
                let integer = match meta.get("step") {
                    None => false,
                    Some(v) if v.as_f64() == Some(1.0) => true,
                    _ => {
                        out.unsupported_fields += 1;
                        return Ok(());
                    }
                };
                if min.zip(max).is_some_and(|(a, b)| a > b) {
                    out.unsupported_fields += 1;
                    return Ok(());
                }
                SettingsFieldKind::Number { min, max, integer }
            }
            Some("string") if !["min", "max", "step"].iter().any(|k| meta.contains_key(*k)) => {
                SettingsFieldKind::String {
                    max_length: MAX_TEXT,
                }
            }
            _ => {
                out.unsupported_fields += 1;
                return Ok(());
            }
        };
        let value = match at(&w.value, path) {
            Some(v) => match scalar(v, &kind) {
                Some(v) => Some(v),
                None => {
                    out.unsupported_fields += 1;
                    return Ok(());
                }
            },
            None => None,
        };
        if out.fields.len() >= MAX_FIELDS {
            return Err(Error::Oversize);
        }
        out.fields.push(SettingsField {
            path: path.clone(),
            label: path.join("."),
            kind,
            value,
            overridden: at(&w.user, path).is_some(),
        });
        Ok(())
    }
    walk(
        &w,
        refs,
        uid,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut out,
        budget,
    )?;
    if projected_bytes(&out) > MAX_BYTES {
        return Err(Error::Oversize);
    }
    Ok(out)
}
fn projected_bytes(namespace: &SettingsNamespaceView) -> usize {
    namespace.ns.len()
        + 128
        + namespace
            .fields
            .iter()
            .map(|f| {
                f.label.len()
                    + f.path.iter().map(String::len).sum::<usize>()
                    + 256
                    + match &f.value {
                        Some(SettingsScalar::String(v)) => v.len(),
                        _ => 32,
                    }
            })
            .sum::<usize>()
}

/// Validate the exact reviewed primitive against its namespace; no root or arbitrary JSON writes.
pub(crate) fn mutation_args(
    namespace: &SettingsNamespaceView,
    field: &SettingsField,
    value: &SettingsScalar,
) -> Result<Value> {
    if !text(&namespace.ns, 512)
        || !namespace.auto_generate
        || namespace.revision > MAX_REVISION
        || !safe_path(&field.path)
        || !namespace.fields.contains(field)
        || !field
            .value
            .as_ref()
            .is_some_and(|original| field.kind.accepts(original) && original != value)
        || !field.kind.accepts(value)
    {
        return Err(Error::InvalidDto);
    }
    let value = match value {
        SettingsScalar::Bool(v) => json!(v),
        SettingsScalar::Number(v) => json!(v),
        SettingsScalar::String(v) => json!(v),
    };
    Ok(
        json!({"ns":namespace.ns,"ops":[{"op":"set","path":field.path,"value":value}],"expectedRevision":namespace.revision}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wire() -> Value {
        json!({"writable":true,"namespaces":[{"ns":"probe","revision":3,"autoGenerate":true,"applies":"live","schema":{"uid":1,"refs":{"1":{"type":"object","meta":{},"dict":{"enabled":2,"count":3,"title":4,"apiKey":5,"provider":6}},"2":{"type":"boolean","meta":{}},"3":{"type":"number","meta":{"min":1,"max":8,"step":1}},"4":{"type":"string","meta":{}},"5":{"type":"string","meta":{"role":"secret","default":"PRIVATE-SENTINEL"}},"6":{"type":"string","meta":{"role":"credential-ref"}}}},"value":{"enabled":true,"count":4,"title":"public title","apiKey":"PRIVATE-SENTINEL","provider":"PRIVATE-REFERENCE"},"base":{},"user":{"count":4},"secrets":[{"path":["apiKey"],"set":true}]}]})
    }
    #[test]
    fn ref_graph_projects_only_supported_primitives_and_debug_hides_values() {
        let s = project_settings(wire()).unwrap();
        let n = &s.namespaces[0];
        assert_eq!(n.fields.len(), 3);
        assert!(n.fields.iter().all(|f| !f.path.contains(&"apiKey".into()) && !f.path.contains(&"provider".into())));
        let debug = format!("{s:?}");
        assert!(!debug.contains("PRIVATE"));
        assert!(!debug.contains("public title"));
        let f = n.fields.iter().find(|f| f.path == ["count"]).unwrap();
        assert!(f.overridden);
        assert_eq!(
            mutation_args(n, f, &SettingsScalar::Number(5.0)).unwrap(),
            json!({"ns":"probe","expectedRevision":3,"ops":[{"op":"set","path":["count"],"value":5.0}]})
        );
        for v in [
            SettingsScalar::Number(f64::NAN),
            SettingsScalar::Number(2.5),
            SettingsScalar::Number(9.0),
            SettingsScalar::String("5".into()),
        ] {
            assert_eq!(mutation_args(n, f, &v), Err(Error::InvalidDto));
        }
    }
    #[test]
    fn complex_unknown_expression_and_secret_ancestor_fail_closed() {
        for node in [
            json!({"type":"union","meta":{},"list":[2,3]}),
            json!({"type":"any","meta":{}}),
            json!({"type":"boolean","meta":{"pattern":{}}}),
            json!({"type":"boolean","meta":{"hidden":true}}),
        ] {
            let mut w = wire();
            w["namespaces"][0]["schema"]["refs"]["2"] = node;
            assert!(
                !project_settings(w).unwrap().namespaces[0]
                    .fields
                    .iter()
                    .any(|f| f.path == ["enabled"])
            );
        }
        let mut w = wire();
        w["namespaces"][0]["user"] = json!({"__jsExpr":"PRIVATE-SENTINEL"});
        assert!(project_settings(w).unwrap().namespaces[0].fields.is_empty());
        let mut w = wire();
        w["namespaces"][0]["secrets"] = json!([{"path":[],"set":true}]);
        assert!(project_settings(w).unwrap().namespaces[0].fields.is_empty());
        let mut w = wire();
        w["namespaces"][0]["schema"]["refs"]["1"]["dict"]["enabled"] = json!(1);
        let s = project_settings(w).unwrap();
        assert!(!s.namespaces[0].fields.iter().any(|f| f.path == ["enabled"]));
    }
    #[test]
    fn bounds_and_forged_sensitive_or_unreviewed_edits_are_rejected() {
        let s = project_settings(wire()).unwrap();
        let mut n = s.namespaces[0].clone();
        let mut f = n.fields[0].clone();
        f.path = vec!["__proto__".into()];
        n.fields.push(f.clone());
        assert_eq!(
            mutation_args(&n, &f, &SettingsScalar::Bool(false)),
            Err(Error::InvalidDto)
        );
        f.path = vec!["unreviewed".into()];
        assert_eq!(
            mutation_args(&n, &f, &SettingsScalar::Bool(false)),
            Err(Error::InvalidDto)
        );
        let mut w = wire();
        w["namespaces"][0]["revision"] = json!(MAX_REVISION + 1);
        assert_eq!(project_settings(w), Err(Error::InvalidDto));
        let mut w = wire();
        w["namespaces"] = json!(vec![w["namespaces"][0].clone(); MAX_ROWS + 1]);
        assert_eq!(project_settings(w), Err(Error::Oversize));
    }
    #[test]
    fn reused_object_graphs_exhaust_shared_traversal_budget() {
        let mut refs = serde_json::Map::new();
        for uid in 1..=6 {
            let dict: serde_json::Map<String, Value> = (0..16)
                .map(|i| (format!("field{i}"), json!(uid + 1)))
                .collect();
            refs.insert(
                uid.to_string(),
                json!({"type":"object","meta":{},"dict":dict}),
            );
        }
        refs.insert("7".into(), json!({"type":"any","meta":{}}));
        let mut w = wire();
        w["namespaces"][0]["schema"] = json!({"uid":1,"refs":refs});
        assert!(serde_json::to_vec(&w).unwrap().len() < 8192);
        let namespace: WireNamespace = serde_json::from_value(w["namespaces"][0].clone()).unwrap();
        let mut small_budget = 100;
        assert_eq!(
            project_wire_namespace(namespace, &mut small_budget),
            Err(Error::Oversize)
        );
        assert_eq!(small_budget, 0);
        assert_eq!(project_settings(w), Err(Error::Oversize));
    }

    #[test]
    fn missing_invalid_original_values_and_noop_writes_are_not_admitted() {
        let s = project_settings(wire()).unwrap();
        let mut n = s.namespaces[0].clone();
        let index = n.fields.iter().position(|f| f.path == ["count"]).unwrap();
        assert_eq!(
            mutation_args(&n, &n.fields[index], &SettingsScalar::Number(4.0)),
            Err(Error::InvalidDto)
        );
        for original in [
            None,
            Some(SettingsScalar::String("masked".into())),
            Some(SettingsScalar::Number(f64::NAN)),
        ] {
            n.fields[index].value = original;
            assert_eq!(
                mutation_args(&n, &n.fields[index], &SettingsScalar::Number(5.0)),
                Err(Error::InvalidDto)
            );
        }
    }

    #[test]
    fn custom_pages_and_authority_sensitive_paths_are_readonly() {
        let mut w = wire();
        w["namespaces"][0]["ns"] = json!("permission-presets");
        w["namespaces"][0]["schema"]["refs"]["1"]["dict"] = json!({"defaultPreset":4});
        w["namespaces"][0]["value"] = json!({"defaultPreset":"PRIVATE-PERMISSION-PRESET"});
        for auto in [false, true] {
            w["namespaces"][0]["autoGenerate"] = json!(auto);
            let s = project_settings(w.clone()).unwrap();
            assert!(s.namespaces[0].fields.is_empty());
            assert!(s.namespaces[0].unsupported_fields > 0);
            assert!(!format!("{s:?}").contains("PRIVATE-PERMISSION"));
        }
        let mut n = project_settings(wire()).unwrap().namespaces.remove(0);
        n.auto_generate = false;
        let enabled = n.fields.iter().find(|f| f.path == ["enabled"]).unwrap();
        assert_eq!(
            mutation_args(&n, enabled, &SettingsScalar::Bool(false)),
            Err(Error::InvalidDto)
        );
        for key in [
            "defaultPreset",
            "permission",
            "approval",
            "sandbox",
            "policy",
            "guard",
            "restrict",
        ] {
            assert!(sensitive(key));
        }
    }

    #[test]
    fn numeric_projection_and_edits_reject_unsafe_integral_values() {
        let safe = MAX_REVISION as f64;
        for integer in [false, true] {
            let kind = SettingsFieldKind::Number {
                min: None,
                max: None,
                integer,
            };
            for value in [safe, -safe, 0.0] {
                assert!(kind.accepts(&SettingsScalar::Number(value)));
                assert_eq!(
                    scalar(&json!(value), &kind),
                    Some(SettingsScalar::Number(value))
                );
            }
            for value in [safe + 1.0, -(safe + 1.0), 1.0e100, f64::INFINITY, f64::NAN] {
                assert!(!kind.accepts(&SettingsScalar::Number(value)));
            }
            // JSON integer decoding must not publish an as_f64-rounded value.
            for value in [
                json!(MAX_REVISION + 1),
                json!(MAX_REVISION + 2),
                json!(-(MAX_REVISION as i64 + 2)),
            ] {
                assert_eq!(scalar(&value, &kind), None);
            }
            assert_eq!(kind.accepts(&SettingsScalar::Number(1.5)), !integer);
        }
    }

    #[test]
    fn inventory_drops_preset_expressions_and_metadata_and_checks_identity() {
        let s = project_inventory(json!({"entries":[{"entryId":"include:one","moduleName":"probe","enabled":true,"fiberPhase":null,"meta":{"error":"PRIVATE-SENTINEL"}}],"agentPresets":[{"condition":"PRIVATE-SENTINEL"}]})).unwrap();
        assert_eq!(s.entries[0].fiber_phase, FiberPhase::Absent);
        assert!(!format!("{s:?}").contains("PRIVATE"));
        assert!(!s.management_available);
        assert_eq!(
            project_inventory(
                json!({"entries":[{"entryId":"","moduleName":"probe","enabled":true,"fiberPhase":null}]})
            ),
            Err(Error::InvalidDto)
        );
    }
}
