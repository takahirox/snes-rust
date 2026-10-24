//! Build four original MIT Mode 7 transform cartridges, without external assets.
#[path = "../tests/common/mode7_transform.rs"]
mod mode7_transform;
fn main() {
    let directory = std::env::args()
        .nth(1)
        .expect("Usage: mode7-transform-fixture DIRECTORY");
    std::fs::create_dir_all(&directory).unwrap();
    for (name, clipped) in [("fractional", false), ("clipped", true)] {
        for (layer, bg2) in [("bg1", false), ("bg2", true)] {
            std::fs::write(
                format!("{directory}/{name}-{layer}.sfc"),
                mode7_transform::guest_rom(clipped, bg2),
            )
            .unwrap();
        }
    }
}
