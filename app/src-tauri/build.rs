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

    // 构建信息（PRD F8-7）：构建时间与依赖清单直接进二进制，关于页无需联网即可展示
    println!("cargo:rustc-env=BUILD_TIME={}", build_time());
    println!("cargo:rerun-if-changed=Cargo.toml");

    let mut deps_code =
        String::from("// 由 build.rs 自动生成，请勿手改。源: Cargo.toml [dependencies]\n");
    deps_code.push_str("pub static DEPS: &[(&str, &str)] = &[\n");
    if let Ok(raw) = fs::read_to_string("Cargo.toml") {
        if let Ok(doc) = raw.parse::<toml::Value>() {
            if let Some(table) = doc.get("dependencies").and_then(|v| v.as_table()) {
                let mut names: Vec<&String> = table.keys().collect();
                names.sort();
                for name in names {
                    let ver = match &table[name] {
                        toml::Value::String(s) => s.clone(),
                        v => v
                            .get("version")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                    };
                    deps_code.push_str(&format!("    (\"{}\", \"{}\"),\n", esc(name), esc(&ver)));
                }
            }
        }
    }
    deps_code.push_str("];\n");
    let deps_dest = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR 未设置"))
        .join("deps_generated.rs");
    fs::write(deps_dest, deps_code).expect("写出 deps_generated.rs 失败");

    tauri_build::build()
}

/// 构建时间用系统本地时间，不额外引入时间库。
fn build_time() -> String {
    let out = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-Date -Format 'yyyy-MM-dd HH:mm'",
        ])
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "未知".to_string(),
    }
}
