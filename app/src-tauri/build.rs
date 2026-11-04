use std::{env, fs, path::PathBuf};

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn main() {
    println!("cargo:rerun-if-changed=features.toml");

    let raw = fs::read_to_string("features.toml").expect("读取 features.toml 失败");
    let doc: toml::Value = raw.parse().expect("features.toml 解析失败");
    let items = doc
        .get("feature")
        .and_then(|v| v.as_array())
        .expect("features.toml 缺少 [[feature]] 段");

    let mut code = String::from("// 由 build.rs 自动生成，请勿手改。源: features.toml\n");
    code.push_str("pub static FEATURES: &[FeatureMeta] = &[\n");

    for f in items {
        let g = |k: &str| f.get(k).and_then(|v| v.as_str()).unwrap_or("");
        let id = g("id");
        let name = g("name");
        let module = g("module");
        let milestone = g("milestone");
        let placeholder = g("placeholder");

        let status = match g("status") {
            "committed" => "Status::Committed",
            "conditional" => "Status::Conditional",
            "deferred" => "Status::Deferred",
            _ => "Status::Out",
        };
        let ms = if milestone.is_empty() {
            "None".to_string()
        } else {
            format!("Some(\"{}\")", esc(milestone))
        };
        let ph = if placeholder.is_empty() {
            "None".to_string()
        } else {
            format!("Some(\"{}\")", esc(placeholder))
        };
        let blocked = f
            .get("blocked_by")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str())
                    .map(|s| format!("\"{}\"", esc(s)))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();

        let delivered = f
            .get("delivered")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        code.push_str(&format!(
            "    FeatureMeta {{ id: \"{}\", name: \"{}\", module: \"{}\", status: {}, milestone: {}, placeholder: {}, blocked_by: &[{}], delivered: {} }},\n",
            esc(id), esc(name), esc(module), status, ms, ph, blocked, delivered
        ));
    }

    code.push_str("];\n");

    let dest = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR 未设置"))
        .join("features_generated.rs");
    fs::write(dest, code).expect("写出 features_generated.rs 失败");

    tauri_build::build()
}
