//! Baubegleiter: schleust die Markenwerte als Umgebungsvariablen ins Binary,
//! damit `kcore::branding` die EINZIGE Stelle im Quelltext ist, an der ein
//! Produktname vorkommt.
//!
//! Quellen, in dieser Reihenfolge (die erste, die etwas liefert, gewinnt):
//! 1. Einzelne Umgebungsvariablen `OS_NAME_OVERRIDE`, `OS_SLUG_OVERRIDE`,
//!    `OS_PUBLISHER_OVERRIDE`, `OS_WEB_OVERRIDE`, `OS_FEED_OVERRIDE`,
//!    `KERNEL_NAME_OVERRIDE` — fuer Experimente ohne Dateiaenderung,
//! 2. `brands/<BRAND>.toml`, wenn die Umgebungsvariable `BRAND` gesetzt ist
//!    (das ist der Weg fuer Zweitmarken: ein Quellbaum, zwei Builds),
//! 3. `[package.metadata.branding]` in `kernel/Cargo.toml`,
//! 4. Ableitung aus dem Cargo-Paketnamen (`osum` -> `Osumos`).
//!
//! Der Kernelname bleibt normalerweise der Cargo-Paketname: eine Zweitmarke
//! aendert das Produkt, nicht den Kernel — genau wie NT unter jeder
//! Windows-Ausgabe NT heisst.
//!
//! Bewusst ohne TOML-Crate: eine Bauabhaengigkeit fuer ein paar Zeilen
//! Textsuche waere genau der Ballast, den dieses Projekt vermeiden will.

use std::fs;
use std::path::PathBuf;

/// Bindet das aus Firn erzeugte Objekt in den Kernel ein.
///
/// Gebaut wird es von `build.sh` (Schritt „Firn-Module"), nicht hier: der
/// Uebersetzer ist auf einen Commit festgenagelt und wird von
/// `vendor/firn/hole-firnc.sh` besorgt. `build.rs` reicht das fertige Objekt
/// nur an den Linker weiter — und sagt deutlich Bescheid, wenn es fehlt,
/// statt den Kernel still ohne serielle Ausgabe zu bauen.
fn link_firn_objects() {
    let wurzel = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .expect("kernel/ hat immer ein Elternverzeichnis")
        .to_path_buf();

    for name in ["serial", "bitmap", "elf"] {
        let quelle = wurzel.join(format!("kernel/firn/{name}.fi"));
        let objekt = wurzel.join(format!("build/firn/{name}.o"));
        println!("cargo:rerun-if-changed={}", quelle.display());
        println!("cargo:rerun-if-changed={}", objekt.display());
        if !objekt.exists() {
            panic!(
                "Firn-Objekt fehlt: {}\n\
                 Es wird von ./build.sh erzeugt. Ein blosses `cargo build` reicht \
                 nicht, weil der Firn-Uebersetzer festgenagelt ist \
                 (vendor/firn/COMMIT). Bau mit ./build.sh.",
                objekt.display()
            );
        }
        println!("cargo:rustc-link-arg={}", objekt.display());
    }
}

/// Liest `schluessel = "wert"` aus einem Abschnitt (oder aus dem Dateikopf,
/// wenn `section` leer ist). Kein TOML-Parser, absichtlich: Markendateien sind
/// flache Listen aus Schluessel und Zeichenkette.
fn value_from(text: &str, section: &str, key: &str) -> Option<String> {
    let mut in_section = section.is_empty();
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('#') || t.is_empty() {
            continue;
        }
        if t.starts_with('[') {
            in_section = t == section;
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((k, v)) = t.split_once('=') else { continue };
        if k.trim() == key {
            let v = v.split('#').next().unwrap_or(v);
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
    link_firn_objects();

    for var in [
        "BRAND",
        "OS_NAME_OVERRIDE",
        "OS_SLUG_OVERRIDE",
        "OS_PUBLISHER_OVERRIDE",
        "OS_WEB_OVERRIDE",
        "OS_FEED_OVERRIDE",
        "KERNEL_NAME_OVERRIDE",
    ] {
        println!("cargo:rerun-if-env-changed={var}");
    }

    let manifest = fs::read_to_string("Cargo.toml").unwrap_or_default();

    // Markendatei, falls BRAND gesetzt ist. Ein Tippfehler im Markennamen soll
    // NICHT still zur Standardmarke zurueckfallen — sonst baut man stundenlang
    // das falsche Produkt, ohne es zu merken.
    let brand = std::env::var("BRAND").ok().filter(|s| !s.is_empty());
    let brand_file = brand.as_ref().map(|b| {
        let pfad = PathBuf::from("../brands").join(format!("{b}.toml"));
        println!("cargo:rerun-if-changed={}", pfad.display());
        fs::read_to_string(&pfad).unwrap_or_else(|e| {
            panic!("Marke \"{b}\" nicht lesbar ({}): {e}\nVorhandene Marken: ls brands/", pfad.display())
        })
    });

    // env-Override > Markendatei > Cargo-Metadaten > Vorgabe
    let feld = |env_name: &str, key: &str, vorgabe: Option<String>| -> Option<String> {
        std::env::var(env_name)
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| brand_file.as_ref().and_then(|t| value_from(t, "", key)))
            .or_else(|| value_from(&manifest, "[package.metadata.branding]", key))
            .or(vorgabe)
    };

    let paket = std::env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "kernel".into());
    let kernel = feld("KERNEL_NAME_OVERRIDE", "kernel-name", Some(paket.clone()))
        .unwrap_or(paket);
    let os = feld("OS_NAME_OVERRIDE", "os-name", None)
        .unwrap_or_else(|| derive_os_name(&kernel));
    let slug = feld("OS_SLUG_OVERRIDE", "slug", None)
        .unwrap_or_else(|| os.to_lowercase());
    let publisher = feld("OS_PUBLISHER_OVERRIDE", "publisher", Some(String::new())).unwrap();
    let web = feld("OS_WEB_OVERRIDE", "web", Some(String::new())).unwrap();
    let feed = feld("OS_FEED_OVERRIDE", "feed", Some(String::new())).unwrap();

    println!("cargo:rustc-env=BRANDING_KERNEL_NAME={kernel}");
    println!("cargo:rustc-env=BRANDING_OS_NAME={os}");
    println!("cargo:rustc-env=BRANDING_SLUG={slug}");
    println!("cargo:rustc-env=BRANDING_PUBLISHER={publisher}");
    println!("cargo:rustc-env=BRANDING_WEB={web}");
    println!("cargo:rustc-env=BRANDING_FEED={feed}");
}
