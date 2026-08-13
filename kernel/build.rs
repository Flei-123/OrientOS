//! Baubegleiter: schleust den Kernel- und den OS-Namen als Umgebungsvariablen
//! ins Binary, damit `kcore::branding` die EINZIGE Stelle im Quelltext ist, an
//! der ein Produktname vorkommt.
//!
//! Quellen, in dieser Reihenfolge:
//! 1. Umgebungsvariable `OS_NAME_OVERRIDE` (fuer Experimente ohne Dateiaenderung),
//! 2. `[package.metadata.branding] os-name = "..."` in `kernel/Cargo.toml`,
//! 3. Ableitung aus dem Cargo-Paketnamen (`karst` -> `Karstos`).
//!
//! Bewusst ohne TOML-Crate: eine Bauabhaengigkeit fuer drei Zeilen Textsuche
//! waere genau der Ballast, den dieses Projekt vermeiden will.

use std::fs;

fn metadata_value(manifest: &str, key: &str) -> Option<String> {
    let mut in_section = false;
    for line in manifest.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_section = t == "[package.metadata.branding]";
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((k, v)) = t.split_once('=') else { continue };
        if k.trim() == key {
            return Some(v.trim().trim_matches('"').to_string());
        }
    }
    None
}

fn derive_os_name(kernel: &str) -> String {
    let mut c = kernel.chars();
    let head = c.next().map(|f| f.to_uppercase().to_string()).unwrap_or_default();
    format!("{head}{}os", c.as_str())
}

fn main() {
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-env-changed=OS_NAME_OVERRIDE");

    let manifest = fs::read_to_string("Cargo.toml").unwrap_or_default();
    let kernel = std::env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "kernel".into());
    let os = std::env::var("OS_NAME_OVERRIDE")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| metadata_value(&manifest, "os-name"))
        .unwrap_or_else(|| derive_os_name(&kernel));

    println!("cargo:rustc-env=BRANDING_KERNEL_NAME={kernel}");
    println!("cargo:rustc-env=BRANDING_OS_NAME={os}");
}
