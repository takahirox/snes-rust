//! Build original MIT fixtures; no downloaded ROM or assembler is needed.
#[path = "../tests/common/hires.rs"]
mod hires;
fn main() {
    let directory = std::env::args()
        .nth(1)
        .expect("Usage: hires-fixture DIRECTORY");
    std::fs::create_dir_all(&directory).unwrap();
    for mode in [1, 5, 6] {
        for interactive in [false, true] {
            let name = format!(
                "{directory}/hires-{mode}{}.sfc",
                if interactive { "-switch" } else { "" }
            );
            std::fs::write(name, hires::guest_rom(mode, interactive)).unwrap();
        }
    }
}
