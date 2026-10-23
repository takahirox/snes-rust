//! Build original MIT SETINI fixtures without downloaded code or an assembler.
#[path = "../tests/common/pseudo_hires.rs"]
mod pseudo_hires;
fn main() {
    let directory = std::env::args()
        .nth(1)
        .expect("Usage: pseudo-hires-fixture DIRECTORY");
    std::fs::create_dir_all(&directory).unwrap();
    for (name, enabled, interactive) in [
        ("off", false, false),
        ("on", true, false),
        ("switch", false, true),
    ] {
        std::fs::write(
            format!("{directory}/pseudo-hires-{name}.sfc"),
            pseudo_hires::guest_rom(enabled, interactive),
        )
        .unwrap();
    }
}
