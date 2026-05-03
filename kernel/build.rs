use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=src/arch/x86_64/ap_trampoline.asm");
    println!("cargo:rerun-if-env-changed=YASM");

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let source = manifest_dir.join("src/arch/x86_64/ap_trampoline.asm");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("out dir"));
    let output = out_dir.join("ap_trampoline.bin");
    let assembler = env::var_os("YASM").unwrap_or_else(|| "yasm".into());

    let status = Command::new(&assembler)
        .arg("-f")
        .arg("bin")
        .arg("-o")
        .arg(&output)
        .arg(&source)
        .status()
        .unwrap_or_else(|error| {
            panic!(
                "failed to launch assembler {:?} for {}: {}",
                assembler,
                source.display(),
                error
            )
        });

    assert!(
        status.success(),
        "assembler {:?} failed for {}",
        assembler,
        source.display()
    );
}
