use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/soundshare.ico");
    println!("cargo:rerun-if-changed=assets/soundshare.manifest");
    println!("cargo:rerun-if-changed=assets/soundshare.rc");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let output = out_dir.join("soundshare.res");
    let resource_compiler =
        find_resource_compiler().expect("Windows SDK resource compiler (rc.exe) was not found");

    let status = Command::new(resource_compiler)
        .current_dir(&manifest_dir)
        .args(["/nologo", "/fo"])
        .arg(&output)
        .arg("assets\\soundshare.rc")
        .status()
        .expect("failed to start rc.exe");

    if !status.success() {
        panic!("rc.exe failed with status {status}");
    }

    println!(
        "cargo:rustc-link-arg-bin=soundshare-gui={}",
        output.display()
    );
}

fn find_resource_compiler() -> Option<PathBuf> {
    if let Some(path) = env::var_os("RC") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }

    let program_files = env::var_os("ProgramFiles(x86)")?;
    let bin_root = Path::new(&program_files).join("Windows Kits\\10\\bin");
    let mut versions = fs::read_dir(bin_root)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    versions.sort_by(|left, right| right.file_name().cmp(&left.file_name()));

    versions
        .into_iter()
        .map(|version| version.join("x64\\rc.exe"))
        .find(|candidate| candidate.is_file())
}
