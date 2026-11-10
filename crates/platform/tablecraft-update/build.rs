use std::{env, fs, path::PathBuf};

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest path"));
    let key = manifest.join("../../../packaging/tablecraft-update-key.pub");
    println!("cargo:rerun-if-changed={}", key.display());
    println!("cargo:rerun-if-changed=build.rs");

    let contents = match fs::read_to_string(&key) {
        Ok(contents) => format!("Some({contents:?})"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => "None".into(),
        Err(error) => panic!("Cannot read the release public key: {error}"),
    };
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output path"));
    fs::write(
        output.join("update_trust.rs"),
        format!("const RELEASE_PUBLIC_KEY: Option<&str> = {contents};\n"),
    )
    .expect("Write compiled update trust");
}
