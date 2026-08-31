fn main() {
    // Tauri embeds `frontendDist` into the binary at COMPILE time
    // (`generate_context!`), and nothing else tells cargo the built frontend
    // changed — so `npm run build` + `cargo run` would silently keep serving the
    // previous UI. Re-running this script marks the crate dirty, which re-reads
    // the assets. Vite rewrites `assets/` on every build, so its mtime moves.
    println!("cargo:rerun-if-changed=../../frontend/dist");
    println!("cargo:rerun-if-changed=../../frontend/dist/index.html");
    println!("cargo:rerun-if-changed=../../frontend/dist/assets");
    tauri_build::build();
}
