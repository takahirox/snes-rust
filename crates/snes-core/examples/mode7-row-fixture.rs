//! Build four original MIT Mode 7 screen-row cartridges without external assets.
#[path = "../tests/common/mode7_row.rs"]
mod mode7_row;
fn main() {
    let directory = std::env::args()
        .nth(1)
        .expect("Usage: mode7-row-fixture DIRECTORY");
    std::fs::create_dir_all(&directory).unwrap();
    for (name, flipped) in [("normal", false), ("flipped", true)] {
        for (layer, bg2) in [("bg1", false), ("bg2", true)] {
            std::fs::write(
                format!("{directory}/{name}-{layer}.sfc"),
                mode7_row::guest_rom(flipped, bg2),
            )
            .unwrap();
        }
    }
}
