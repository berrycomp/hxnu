// TCOL / HPL (HXNU Public License)
// This file is strictly governed by the HXNU Public License (HPL).
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=src/arch/x86_64/ap_trampoline.asm");
    println!("cargo:rerun-if-changed=src/arch/x86_64/ap_trampoline.bin");
    println!("cargo:rerun-if-env-changed=YASM");

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let source = manifest_dir.join("src/arch/x86_64/ap_trampoline.asm");
    let prebuilt = manifest_dir.join("src/arch/x86_64/ap_trampoline.bin");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("out dir"));
    let output = out_dir.join("ap_trampoline.bin");
    let assembler = env::var_os("YASM").unwrap_or_else(|| "yasm".into());

    let status = Command::new(&assembler)
        .arg("-f")
        .arg("bin")
        .arg("-o")
        .arg(&output)
        .arg(&source)
        .status();

    match status {
        Ok(s) if s.success() => {},
        _ => {
            if prebuilt.exists() {
                fs::copy(&prebuilt, &output).expect("failed to copy prebuilt ap_trampoline.bin");
            } else {
                panic!("failed to assemble or copy ap_trampoline.bin");
            }
        }
    }
}
