//! Package stored thumbnails; never render or update references during a GUI build.
use omalux::preset::{PresetDocument, catalog::BUILTIN_PRESETS};
use std::{error::Error, fs, path::PathBuf};

fn write_if_changed(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    // Qt registers these generated files as Cargo inputs. Keep their mtimes
    // stable so they cannot trigger a rebuild on every subsequent invocation.
    if fs::read(path).ok().as_deref() != Some(bytes) {
        fs::write(path, bytes)?;
    }
    Ok(())
}

fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn package() -> Result<PathBuf, Box<dyn Error>> {
    println!("cargo::rerun-if-changed=build/preset_previews.rs");
    let directory = PathBuf::from(std::env::var_os("OUT_DIR").ok_or("missing OUT_DIR")?)
        .join("preset-previews");
    fs::create_dir_all(&directory)?;
    let mut resources = String::from("<RCC><qresource prefix=\"/preset-previews\">\n");
    for source in BUILTIN_PRESETS {
        let preset = PresetDocument::from_json(source.json)?;
        let output = directory.join(format!("{}.jpg", preset.id));
        write_if_changed(&output, source.thumbnail)?;
        resources.push_str(&format!(
            "<file alias=\"{}.jpg\">{}</file>\n",
            xml(&preset.id),
            xml(&output.to_string_lossy()),
        ));
    }
    resources.push_str("</qresource></RCC>\n");
    let qrc = directory.join("preset_previews.qrc");
    write_if_changed(&qrc, resources.as_bytes())?;
    Ok(qrc)
}
