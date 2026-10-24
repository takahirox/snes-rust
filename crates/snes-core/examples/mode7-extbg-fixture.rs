//! Build the original MIT Mode 7 EXTBG cartridge without external assets.
#[path = "../tests/common/mode7_extbg.rs"]
mod mode7_extbg;
fn main() {
    let directory = std::env::args()
        .nth(1)
        .expect("Usage: mode7-extbg-fixture DIRECTORY");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        format!("{directory}/mode7-extbg.sfc"),
        mode7_extbg::guest_rom(),
    )
    .unwrap();
}
