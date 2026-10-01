use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=assets/icons");
    let directory = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("assets/icons");
    let mut paths: Vec<_> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "svg"))
        .collect();
    paths.sort();
    let mut source = String::from("const ICON_ASSETS: &[(&str, &[u8])] = &[\n");
    for path in paths {
        let name = format!("icons/{}", path.file_name().unwrap().to_str().unwrap());
        source.push_str(&format!(
            "({name:?}, include_bytes!({:?}).as_slice()),\n",
            path.to_str().unwrap()
        ));
    }
    source.push_str("];\n");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("icons.rs");
    fs::write(output, source).unwrap();
}
