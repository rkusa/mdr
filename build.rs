use std::path::Path;
use std::{env, fs};

use parcel_css::stylesheet::{ParserOptions, PrinterOptions, StyleSheet};
use sha2::{Digest, Sha256};

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();

    // normalize.css
    println!("cargo:rerun-if-changed=src/theme/normalize.css");
    let normalize = include_str!("src/theme/normalize.css");
    let file_name = format!("normalize-{}.css", hash(normalize));
    let normalize = StyleSheet::parse(
        "normalize.css".to_string(),
        normalize,
        ParserOptions {
            nesting: true,
            css_modules: false,
        },
    )
    .unwrap();
    let normalize = normalize
        .to_css(PrinterOptions {
            minify: true,
            source_map: false,
            targets: None,
            analyze_dependencies: true,
            pseudo_classes: None,
        })
        .unwrap();
    assert!(
        normalize.dependencies.unwrap().is_empty(),
        "CSS dependencies are not supported yet"
    );
    fs::write(Path::new(&out_dir).join(&file_name), normalize.code).unwrap();
    println!("cargo:rustc-env=NORMALIZE_CSS={}", file_name);

    // style.css
    println!("cargo:rerun-if-changed=src/theme/style.css");
    let style = include_str!("src/theme/style.css");
    let file_name = format!("style-{}.css", hash(style));
    let style = StyleSheet::parse(
        "style.css".to_string(),
        style,
        ParserOptions {
            nesting: true,
            css_modules: false,
        },
    )
    .unwrap();
    let style = style
        .to_css(PrinterOptions {
            minify: true,
            source_map: false,
            targets: None,
            analyze_dependencies: true,
            pseudo_classes: None,
        })
        .unwrap();
    assert!(
        style.dependencies.unwrap().is_empty(),
        "CSS dependencies are not supported yet"
    );
    fs::write(Path::new(&out_dir).join(&file_name), style.code).unwrap();
    println!("cargo:rustc-env=STYLE_CSS={}", file_name);
}

fn hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    let hash = hasher.finalize();
    base64::encode_config(&hash[..16], base64::URL_SAFE_NO_PAD)
}
