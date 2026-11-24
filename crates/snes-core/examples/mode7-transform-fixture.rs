//! Build four original MIT Mode 7 transform cartridges, without external assets.
#[path = "../tests/common/mode7_transform.rs"]
mod mode7_transform;
fn main() {
    let mut args = std::env::args().skip(1);
    let directory = args
        .next()
        .expect("Usage: mode7-transform-fixture DIRECTORY [--hirom]");
    let hirom = match args.next().as_deref() {
        None => false,
        Some("--hirom") if args.next().is_none() => true,
        _ => panic!("Usage: mode7-transform-fixture DIRECTORY [--hirom]"),
    };
    std::fs::create_dir_all(&directory).unwrap();
    for (name, clipped) in [("fractional", false), ("clipped", true)] {
        for (layer, bg2) in [("bg1", false), ("bg2", true)] {
            let mut rom = mode7_transform::guest_rom(clipped, bg2);
            if hirom {
                // The guest executes from bank zero at $8000 in both mappings.
                rom[0x7fd5] = 0x21;
                rom[0x7fd7] = 6;
                rom[0x7fdc..0x7fe0].fill(0);
                let checksum = rom
                    .iter()
                    .fold(510u16, |sum, &b| sum.wrapping_add(u16::from(b)));
                rom[0x7fdc..0x7fde].copy_from_slice(&(!checksum).to_le_bytes());
                rom[0x7fde..0x7fe0].copy_from_slice(&checksum.to_le_bytes());
                let mut image = vec![0; 32768];
                image.extend(rom);
                rom = image;
            }
            std::fs::write(format!("{directory}/{name}-{layer}.sfc"), rom).unwrap();
        }
    }
}
