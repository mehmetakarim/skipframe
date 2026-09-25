use std::process::Command;

fn main() {
    // The build number shown in the About window: how many commits are behind this binary.
    // A source tarball with no git history builds fine and simply has no build number.
    let count = Command::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(count) = count {
        println!("cargo:rustc-env=SKIPFRAME_BUILD={count}");
    }
    // Without this, a new commit would not rebuild and the number would go stale.
    println!("cargo:rerun-if-changed=../.git/HEAD");

    tauri_build::build()
}
