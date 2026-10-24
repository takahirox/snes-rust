//! Build the original MIT Mode 7 scroll cartridge, without external assets.
#[path = "../tests/common/mode7_scroll.rs"]
mod mode7_scroll;
fn main() {
    let directory = std::env::args()
        .nth(1)
        .expect("Usage: mode7-scroll-fixture DIRECTORY");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        format!("{directory}/mode7-scroll.sfc"),
        mode7_scroll::guest_rom(),
    )
    .unwrap();
}
