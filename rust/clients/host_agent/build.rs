use std::process::Command;

fn main() {
    let output = Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .output()
        .unwrap();
    let hash = String::from_utf8(output.stdout).unwrap();
    println!("cargo:rustc-env=GIT_HASH={}", hash);

    let status = Command::new("git")
        .args(&["diff-index", "--quiet", "HEAD"])
        .status()
        .unwrap();
    if status.success() {
        println!("cargo:rustc-env=GIT_STATUS=clean");
    } else {
        println!("cargo:rustc-env=GIT_STATUS=dirty");
    }
}
