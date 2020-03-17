extern crate crc;

use std::io::{Cursor, Read, Result, Seek, SeekFrom, copy};

use super::gmstream::GmStream;

pub fn decrypt_gm8xx<T: Read>(mut stream: T) -> Result<Cursor<Vec<u8>>> {
    let mut forward_table: [u8; 256] = [0; 256];
    let mut reverse_table: [u8; 256] = [0; 256];

    // Read and construct tables.
    let d1 = stream.next_u32()?;
    let d2 = stream.next_u32()?;
    stream.skip(4 * d1)?;
    stream.read_exact(&mut forward_table)?;
    stream.skip(4 * d2)?;
    for i in 0..256 {
        reverse_table[forward_table[i] as usize] = i as u8;
    }

    // Read data into memory.
    let len = stream.next_u32()? as usize;
    let mut buf = Vec::with_capacity(len);
    stream.take(len as u64).read_to_end(&mut buf)?;

    // Phase 1.
    for i in (1..len).rev() {
        let a = reverse_table[buf[i] as usize] as u64;
        let b = buf[i - 1] as u64;
        let c = a.wrapping_sub(b).wrapping_sub(i as u64);
        buf[i] = c as u8;
    }

    // Phase 2.
    for i in (0..len).rev() {
        let a = forward_table[i & 0xFF] as i64;
        let mut b = (i as i64) - a;
        if b < 0 {
            b = 0;
        }

        let a = buf[i];
        buf[i] = buf[b as usize];
        buf[b as usize] = a;
    }

    Ok(Cursor::new(buf))
}

pub fn decrypt_gm810<T: Read + Seek>(stream: &mut T) -> Result<Cursor<Vec<u8>>> {
    // Generate seeds
    let key = format!("_MJD{}#RWK", stream.next_u32()?);
    let mut key_buffer = Vec::new();
    for b in key.bytes() {
        key_buffer.push(b);
        key_buffer.push(0);
    }
    let mut seed2: u32 = crc::crc32::checksum_ieee(&key_buffer) ^ 0xFFFFFFFF;
    let mut seed1: u32 = stream.next_u32()?;

    // Read version
    let mut version = stream.next_u32()?;
    let mut pos = stream.seek(SeekFrom::Current(0))? as u32;
    pos -= 4 + 0x0039FBC4 + 0x11;
    if pos == 0 {
        pos += 3;
    }
    pos = ((pos as i32) >> 2) as u32;
    version = version ^ pos;
    assert_eq!(version, 810);

    // Decrypt.
    let mut buf: Vec<u8> = Vec::new();
    stream.read_to_end(&mut buf)?;

    let mut pos = ((seed2 & 0xFF) + 6) as usize;
    println!("pos: {}", pos);
    while pos <= buf.len() - 4 {
        let chunk = &mut buf[pos..(pos + 4)];
        let mut input = [0u8; 4];
        input.copy_from_slice(chunk);
        pos += 4;

        let input = u32::from_le_bytes(input);
        seed1 = (0xFFFF & seed1) * 0x9069 + (seed1 >> 16);
        seed2 = (0xFFFF & seed2) * 0x4650 + (seed2 >> 16);
        let mask = (seed1 << 16) + (seed2 & 0xFFFF);
        let output: u32 = input ^ mask;
        chunk.copy_from_slice(&output.to_le_bytes());
    }

    Ok(Cursor::new(buf))
}

pub fn decrypt_gm530<T: Read + Seek>(stream: &mut T, key: u32) -> Result<Cursor<Vec<u8>>> {
    let mut arr0: [u32; 256] = [0; 256];
    let mut arr1: [u32; 256] = [0; 256];

    for i in 0..256 {
        arr0[i] = i as u32;
    }
    for i in 1..10001 {
        let j = ((i * key) % 254 + 1) as usize;
        let k = arr0[j];
        arr0[j] = arr0[j + 1];
        arr0[j + 1] = k;
    }
    for i in 1..256 {
        arr1[arr0[i] as usize] = i as u32;
    }

    // Decrypt.
    let mut buf: Vec<u8> = Vec::new();
    stream.read_to_end(&mut buf)?;

    for i in 0..buf.len() {
        buf[i] = arr1[buf[i] as usize] as u8;
    }

    Ok(Cursor::new(buf))
}

pub fn decrypt_gm700<T: Read + Seek>(stream: &mut T) -> Result<Cursor<Vec<u8>>> {
    // First uncompress, then decrypt.
    let decompressed = stream.next_compressed()?;
    let decrypted = deobfuscate(decompressed, 0, true, true)?;
    Ok(Cursor::new(decrypted))
}

pub fn decrypt_gm600<T: Read + Seek>(stream: &mut T) -> Result<Cursor<Vec<u8>>> {
    // First uncompress, then decrypt.
    let decompressed = stream.next_compressed()?;
    let decrypted = deobfuscate(decompressed, 4, true, false)?;
    Ok(Cursor::new(decrypted))
}

pub fn make_swap_table(seed: u32) -> [u8; 256] {
    let mut table0: [u8; 256] = [0; 256];
    let mut table1: [u8; 256] = [0; 256];

    let a = 6 + (seed % 250);
    let b = seed / 250;
    for i in 0..256 {
        table0[i] = i as u8;
    }
    for i in 1..10001 {
        let j = (1 + ((i * a + b) % 254)) as usize;
        let t = table0[j];
        table0[j] = table0[j + 1];
        table0[j + 1] = t;
    }
    for i in 1..256 {
        table1[table0[i] as usize] = i as u8;
    }
    table1
}

pub fn do_swap(buffer: &mut [u8], table: [u8; 256], use_offset: bool, initial_offset: usize) {
    for i in 0..buffer.len() {
        let t = buffer[i] as usize;
        buffer[i] = if use_offset {
            let val = (table[t] as i64) - ((initial_offset + i) as i64);
            (val & 0xFF) as u8
        } else {
            table[t]
        }
    }
}

fn deobfuscate(mut input: Cursor<Vec<u8>>, initial_unencrypted: u64, has_garbage: bool, use_offset: bool) -> Result<Vec<u8>> {
    let mut output = Cursor::new(Vec::new());
    let start_pos = input.seek(SeekFrom::Current(0))?;

    copy(&mut input.by_ref().take(initial_unencrypted), &mut output)?;
    let swap_seed = if has_garbage {
        let s1 = input.next_u32()?;
        let s2 = input.next_u32()?;
        input.seek(SeekFrom::Current((4 * s1) as i64))?;
        let seed = input.next_u32()?;
        input.seek(SeekFrom::Current((4 * s2) as i64))?;
        seed
    } else {
        input.next_u32()?
    };
    copy(&mut input.by_ref().take(1), &mut output)?;
    let end_pos = input.seek(SeekFrom::Current(0))?;

    let swap_start = output.get_ref().len();
    let swap_length = copy(&mut input, &mut output)? as usize;
    let swap_offset = (end_pos - start_pos) as usize;
    let swap_table = make_swap_table(swap_seed);
    let mut output = output.into_inner();
    do_swap(&mut output[swap_start..(swap_start + swap_length)], swap_table, use_offset, swap_offset);

    Ok(output)
}
