extern crate encoding_rs;

use flate2::read::ZlibDecoder;
use std::io;
use std::io::{Read, Take};

pub trait GmStream: Sized {
    fn next_u32(&mut self) -> io::Result<u32>;

    fn next_i32(&mut self) -> io::Result<i32>;

    fn next_bool(&mut self) -> io::Result<bool> {
        Ok(GmStream::next_u32(self)? != 0)
    }

    fn next_f64(&mut self) -> io::Result<f64>;

    fn next_string(&mut self) -> io::Result<String>;

    fn skip(&mut self, bytes: u32) -> io::Result<()>;

    fn read_compressed(&mut self) -> io::Result<ZlibDecoder<Take<&mut Self>>>;

    fn skip_section(&mut self) -> io::Result<()>;
}

impl<T: Read> GmStream for T {
    fn next_u32(&mut self) -> io::Result<u32> {
        let mut bytes = [0u8; 4];
        self.read_exact(&mut bytes)?;
        Ok(u32::from_le_bytes(bytes))
    }

    fn next_i32(&mut self) -> io::Result<i32> {
        let mut bytes = [0u8; 4];
        self.read_exact(&mut bytes)?;
        Ok(i32::from_le_bytes(bytes))
    }

    fn next_f64(&mut self) -> io::Result<f64> {
        let mut bytes = [0u8; 8];
        self.read_exact(&mut bytes)?;
        Ok(f64::from_le_bytes(bytes))
    }

    fn next_string(&mut self) -> io::Result<String> {
        let length = self.next_u32()?;
        let mut data = Vec::with_capacity(length as usize);
        self.take(length as u64).read_to_end(&mut data)?;
        let (decoded, _, _) = encoding_rs::WINDOWS_1252.decode(&data);
        let string = decoded.to_string();
        Ok(string)
    }

    fn skip(&mut self, bytes: u32) -> io::Result<()> {
        let mut sub = self.take(bytes as u64);
        match io::copy(&mut sub, &mut io::sink()) {
            Err(e) => Err(e),
            Ok(_) => Ok(()),
        }
    }

    fn read_compressed(&mut self) -> io::Result<ZlibDecoder<Take<&mut T>>> {
        let length = GmStream::next_u32(self)?;
        let substream = self.take(length as u64);
        let decoder = ZlibDecoder::new(substream);

        Ok(decoder)
    }

    fn skip_section(&mut self) -> io::Result<()> {
        let length = GmStream::next_u32(self)?;
        self.skip(length)
    }
}
