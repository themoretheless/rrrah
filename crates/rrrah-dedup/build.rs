mod build_context;

use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn files(path: &Path, output: &mut Vec<PathBuf>) {
    println!("cargo:rerun-if-changed={}", path.display());
    if path.is_dir() {
        for entry in fs::read_dir(path).expect("read recipe source directory") {
            let child = entry.expect("read recipe source entry").path();
            if child.file_name().is_some_and(|n| n == "target" || n == ".git") {
                continue;
            }
            files(&child, output);
        }
    } else {
        output.push(path.to_path_buf());
    }
}

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let workspace = manifest.parent().and_then(Path::parent);
    let complete_workspace = workspace.is_some_and(|root| {
        ["rrrah-dedup", "rrrah-decode", "rrrah-core", "rrrah-memory"]
            .iter()
            .all(|name| root.join("crates").join(name).join("Cargo.toml").is_file())
    });
    let root = if complete_workspace {
        workspace.expect("checked workspace")
    } else {
        manifest.as_path()
    };
    let mut paths = Vec::new();
    if complete_workspace {
        for name in ["rrrah-dedup", "rrrah-decode", "rrrah-core", "rrrah-memory"] {
            files(&root.join("crates").join(name), &mut paths);
        }
        for name in ["Cargo.toml", "Cargo.lock"] {
            files(&root.join(name), &mut paths);
        }
    } else {
        assert!(
            env::var_os("CARGO_FEATURE_DECODE").is_none(),
            "decode recipe identity currently requires the complete rrrah source workspace"
        );
        files(root, &mut paths);
    }
    paths.sort_unstable();
    let mut hash = blake3::Hasher::new();
    hash.update(b"rrrah-built-source-recipe-v1");
    hash.update(&[u8::from(complete_workspace)]);
    let compiler = std::process::Command::new(env::var_os("RUSTC").expect("Rust compiler"))
        .arg("-vV")
        .output()
        .expect("read Rust compiler identity");
    assert!(compiler.status.success(), "Rust compiler identity failed");
    hash.update(&(compiler.stdout.len() as u64).to_le_bytes());
    hash.update(&compiler.stdout);
    for path in paths {
        let relative = path
            .strip_prefix(root)
            .expect("recipe source inside workspace")
            .to_string_lossy();
        let bytes = fs::read(&path).expect("read recipe source");
        hash.update(&(relative.len() as u64).to_le_bytes());
        hash.update(relative.as_bytes());
        hash.update(&(bytes.len() as u64).to_le_bytes());
        hash.update(&bytes);
    }
    let target = env::var("TARGET").expect("build target");
    for key in build_context::watched_keys(&target) {
        println!("cargo:rerun-if-env-changed={key}");
    }
    let context = build_context::context(&target, env::vars());
    for (key, value) in context {
        hash.update(&(key.len() as u64).to_le_bytes());
        hash.update(key.as_bytes());
        hash.update(&(value.len() as u64).to_le_bytes());
        hash.update(value.as_bytes());
    }
    println!(
        "cargo:rustc-env=RRRAH_BUILT_RECIPE_ID={}",
        hash.finalize().to_hex()
    );
}
