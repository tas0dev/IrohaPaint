use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn collect_svg_files(root: &Path, directory: &Path, files: &mut Vec<(String, PathBuf)>) {
    for entry in fs::read_dir(directory).expect("failed to read icon directory") {
        let entry = entry.expect("failed to read icon entry");
        let path = entry.path();

        if path.is_dir() {
            collect_svg_files(root, &path, files);
            continue;
        }

        if path.extension().and_then(|extension| extension.to_str()) != Some("svg") {
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .expect("icon is outside icon directory");

        let name = relative
            .with_extension("")
            .to_string_lossy()
            .replace('\\', "/");

        files.push((name, path));
    }
}

fn main() {
    let icon_root = PathBuf::from("resources/icons");
    let mut files = Vec::new();

    collect_svg_files(&icon_root, &icon_root, &mut files);
    files.sort_by(|left, right| left.0.cmp(&right.0));

    let mut generated = String::from(
        "pub fn generated_icon_bytes(name: &str) -> Option<&'static [u8]> {\n\
		\tmatch name {\n",
    );

    for (name, path) in files {
        let absolute = fs::canonicalize(&path).expect("failed to canonicalize icon path");

        generated.push_str(&format!(
            "\t\t{0:?} => Some(include_bytes!({1:?})),\n",
            name,
            absolute.to_string_lossy(),
        ));

        println!("cargo:rerun-if-changed={}", path.display());
    }

    generated.push_str(
        "\t\t_ => None,\n\
		\t}\n\
		}\n",
    );

    let output = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is not set")).join("icons.rs");

    fs::write(output, generated).expect("failed to generate icon table");

    println!("cargo:rerun-if-changed=resources/icons");
}
