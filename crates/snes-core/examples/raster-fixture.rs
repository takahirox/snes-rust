//! Build the original MIT CPU/DMA raster cartridge. Never a public site asset.
#[path = "../tests/common/raster.rs"]
mod raster;
fn main() {
    let directory = std::env::args()
        .nth(1)
        .expect("Usage: raster-fixture DIRECTORY");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(format!("{directory}/raster.sfc"), raster::guest_rom()).unwrap();
}
