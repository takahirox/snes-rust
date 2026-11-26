//! Machine snapshot wire encoding. All fields are explicit and little-endian.
use miniz_oxide::{
    deflate::compress_to_vec_zlib,
    inflate::{core::*, TINFLStatus},
};
use sha2::{Digest, Sha256};

const MAGIC: &[u8; 8] = b"SNESRUST";
const VERSION: u16 = 1;
const HEADER: usize = 56;
const DIGEST: usize = 32;
const MAX_BODY: usize = 1024 * 1024;
pub const MAX_SNAPSHOT_BYTES: usize = MAX_BODY + 1024 + HEADER + DIGEST;

pub(crate) type Result<T> = std::result::Result<T, String>;

pub(crate) fn invalid() -> String {
    "Save state is corrupt or has invalid machine data".into()
}

pub(crate) fn pack(rom: [u8; 32], mapping: u8, battery: bool, sram: usize, body: &[u8]) -> Vec<u8> {
    assert!(body.len() <= MAX_BODY);
    let compressed = compress_to_vec_zlib(body, 1);
    let mut file = Vec::with_capacity(HEADER + DIGEST + compressed.len());
    file.extend_from_slice(MAGIC);
    VERSION.write(&mut file);
    rom.write(&mut file);
    mapping.write(&mut file);
    battery.write(&mut file);
    sram.write(&mut file);
    body.len().write(&mut file);
    compressed.len().write(&mut file);
    let digest = Sha256::new()
        .chain_update(&file)
        .chain_update(&compressed)
        .finalize();
    file.extend_from_slice(&digest);
    file.extend_from_slice(&compressed);
    file
}

pub(crate) fn unpack(
    file: &[u8],
    rom: [u8; 32],
    mapping: u8,
    battery: bool,
    sram: usize,
) -> Result<Vec<u8>> {
    if file.len() > MAX_SNAPSHOT_BYTES {
        return Err("Save state exceeds the supported size".into());
    }
    let mut header = Reader(file.get(..HEADER).ok_or_else(invalid)?);
    if header.take(8)? != MAGIC {
        return Err("Choose a snes-rust save state file".into());
    }
    let version = u16::read(&mut header)?;
    if version != VERSION {
        return Err(format!(
            "Save state version {version} is unsupported. Use a snes-rust build that supports it (this build supports {VERSION})."
        ));
    }
    if <[u8; 32]>::read(&mut header)? != rom {
        return Err(
            "Save state belongs to a different ROM revision. Insert the original ROM".into(),
        );
    }
    if u8::read(&mut header)? != mapping
        || bool::read(&mut header)? != battery
        || usize::read(&mut header)? != sram
    {
        return Err(invalid());
    }
    let len = usize::read(&mut header)?;
    let input_len = usize::read(&mut header)?;
    let compressed = file.get(HEADER + DIGEST..).ok_or_else(invalid)?;
    if !(1..=MAX_BODY).contains(&len) || compressed.len() != input_len {
        return Err(invalid());
    }
    let digest = Sha256::new()
        .chain_update(&file[..HEADER])
        .chain_update(compressed)
        .finalize();
    if digest[..] != file[HEADER..HEADER + DIGEST] {
        return Err(invalid());
    }
    let mut body = vec![0; len];
    let (status, consumed, written) = decompress(
        &mut DecompressorOxide::new(),
        compressed,
        &mut body,
        0,
        inflate_flags::TINFL_FLAG_PARSE_ZLIB_HEADER
            | inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF,
    );
    if status != TINFLStatus::Done || consumed != compressed.len() || written != len {
        return Err(invalid());
    }
    Ok(body)
}

pub(crate) struct Reader<'a>(pub(crate) &'a [u8]);

impl<'a> Reader<'a> {
    pub(crate) fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let (value, rest) = self.0.split_at_checked(len).ok_or_else(invalid)?;
        self.0 = rest;
        Ok(value)
    }
}

pub(crate) trait Value: Sized {
    fn write(&self, out: &mut Vec<u8>);
    fn read(input: &mut Reader<'_>) -> Result<Self>;
}

macro_rules! integer {
    ($($ty:ty),+) => {
        $(impl Value for $ty {
            fn write(&self, out: &mut Vec<u8>) {
                out.extend_from_slice(&self.to_le_bytes());
            }
            fn read(input: &mut Reader<'_>) -> Result<Self> {
                Ok(Self::from_le_bytes(
                    input.take(size_of::<Self>())?.try_into().unwrap(),
                ))
            }
        })+
    };
}
integer!(u8, u16, i16, u32, i32, u64, i64, u128);

impl Value for usize {
    fn write(&self, out: &mut Vec<u8>) {
        u32::try_from(*self)
            .expect("snapshot field fits in u32")
            .write(out);
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        Ok(u32::read(input)? as Self)
    }
}

impl Value for bool {
    fn write(&self, out: &mut Vec<u8>) {
        u8::from(*self).write(out);
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        match u8::read(input)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(invalid()),
        }
    }
}

impl<T: Value + Copy + Default, const N: usize> Value for [T; N] {
    fn write(&self, out: &mut Vec<u8>) {
        for value in self {
            value.write(out);
        }
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        let mut result = [T::default(); N];
        for value in &mut result {
            *value = T::read(input)?;
        }
        Ok(result)
    }
}

impl<T: Value> Value for Option<T> {
    fn write(&self, out: &mut Vec<u8>) {
        self.is_some().write(out);
        if let Some(value) = self {
            value.write(out);
        }
    }
    fn read(input: &mut Reader<'_>) -> Result<Self> {
        if bool::read(input)? {
            Ok(Some(T::read(input)?))
        } else {
            Ok(None)
        }
    }
}

macro_rules! tuple {
    ($($ty:ident : $index:tt),+) => {
        impl<$($ty: Value),+> Value for ($($ty,)+) {
            fn write(&self, out: &mut Vec<u8>) {
                $(self.$index.write(out);)+
            }
            fn read(input: &mut Reader<'_>) -> Result<Self> {
                Ok(($($ty::read(input)?,)+))
            }
        }
    };
}
tuple!(A: 0, B: 1);
tuple!(A: 0, B: 1, C: 2);

macro_rules! snapshot_fields {
    ($ty:ty, $($field:ident),+ $(,)?) => {
        impl $crate::snapshot::Value for $ty {
            fn write(&self, out: &mut Vec<u8>) {
                let Self { $($field),+ } = self;
                $($field.write(out);)+
            }
            fn read(input: &mut $crate::snapshot::Reader<'_>) -> $crate::snapshot::Result<Self> {
                Ok(Self { $($field: $crate::snapshot::Value::read(input)?),+ })
            }
        }
    };
}
pub(crate) use snapshot_fields;

macro_rules! snapshot_members {
    ($ty:ty, $($field:ident),+ $(,)?) => {
        impl $ty {
            pub(crate) fn write_members(&self, out: &mut Vec<u8>) {
                $(self.$field.write(out);)+
            }
            pub(crate) fn read_members(&mut self, input: &mut $crate::snapshot::Reader<'_>) -> $crate::snapshot::Result<()> {
                $(self.$field = $crate::snapshot::Value::read(input)?;)+
                Ok(())
            }
        }
    };
}
pub(crate) use snapshot_members;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Machine;

    fn refresh_digest(file: &mut [u8]) {
        let digest = Sha256::new()
            .chain_update(&file[..HEADER])
            .chain_update(&file[HEADER + DIGEST..])
            .finalize();
        file[HEADER..HEADER + DIGEST].copy_from_slice(&digest);
    }

    #[test]
    fn packed_stream_must_match_both_lengths_even_with_valid_integrity_data() {
        let rom = [12; 32];
        let saved = pack(rom, 0, true, 2048, &[0; 16_384]);
        for len in [1u32, 16_383, 16_385, u32::MAX] {
            let mut file = saved.clone();
            file[48..52].copy_from_slice(&len.to_le_bytes());
            refresh_digest(&mut file);
            assert!(unpack(&file, rom, 0, true, 2048).is_err());
        }
        let mut trailing = saved;
        trailing.push(0);
        let len = (trailing.len() - HEADER - DIGEST) as u32;
        trailing[52..56].copy_from_slice(&len.to_le_bytes());
        refresh_digest(&mut trailing);
        assert!(unpack(&trailing, rom, 0, true, 2048).is_err());
    }

    #[test]
    fn checksummed_invalid_machine_fields_cannot_replace_the_inserted_guest() {
        let mut machine = Machine::new(include_bytes!("../../../tests/roms/smoke.bin")).unwrap();
        machine.run_frame().unwrap();
        let saved = machine.save_snapshot().unwrap();
        let cart = &machine.bus.cart;
        let (hash, mapping, battery, size) = (
            cart.rom_hash(),
            cart.mapping.snapshot_id(),
            cart.battery,
            cart.sram.len(),
        );
        let mut body = unpack(&saved, hash, mapping, battery, size).unwrap();
        // CPU registers occupy 15 bytes, followed by emulation mode and WAI.
        body[16] = 2;
        let invalid = pack(hash, mapping, battery, size, &body);
        assert!(machine.load_snapshot(&invalid).is_err());
        assert_eq!(machine.save_snapshot().unwrap(), saved);
    }

    #[test]
    fn checksummed_invalid_cpu_modes_cannot_replace_the_inserted_guest() {
        use crate::cpu::{M, X};

        let mut machine = Machine::new(include_bytes!("../../../tests/roms/smoke.bin")).unwrap();
        let reset = machine.save_snapshot().unwrap();
        machine.run_frame().unwrap();
        let current = machine.save_snapshot().unwrap();
        let cart = &machine.bus.cart;
        let (hash, mapping, battery, size) = (
            cart.rom_hash(),
            cart.mapping.snapshot_id(),
            cart.battery,
            cart.sram.len(),
        );
        let body = unpack(&reset, hash, mapping, battery, size).unwrap();
        assert_eq!(body[15], 1); // E
        assert_eq!(body[14] & (M | X), M | X); // P
        for (offset, value) in [
            (14, body[14] & !(M | X)),
            (14, body[14] & !M),
            (14, body[14] & !X),
            (7, 0x02), // S high byte
            (3, 0x01), // X high byte
            (5, 0x01), // Y high byte
        ] {
            let mut invalid = body.clone();
            invalid[offset] = value;
            let file = pack(hash, mapping, battery, size, &invalid);
            assert_eq!(machine.load_snapshot(&file).unwrap_err(), super::invalid());
            assert_eq!(machine.save_snapshot().unwrap(), current);
        }
        machine.load_snapshot(&reset).unwrap();
        assert_eq!(machine.save_snapshot().unwrap(), reset);
    }
}
