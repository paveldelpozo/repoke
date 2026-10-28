// The target triple names the release asset `self-update` downloads.
fn main() {
    println!(
        "cargo:rustc-env=REPOKE_TARGET={}",
        std::env::var("TARGET").expect("cargo sets TARGET")
    );
}
