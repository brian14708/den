fn main() {
    // Ensure web/out exists so rust-embed compiles even without a frontend build.
    let out = std::path::Path::new("web/out");
    if !out.exists() {
        std::fs::create_dir_all(out).ok();
    }
}
