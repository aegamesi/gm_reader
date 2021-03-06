extern crate encoding_rs;

use flate2::read::ZlibDecoder;
use std::io;
use std::io::Read;

pub trait GmStream: Sized {
    fn next_u32(&mut self) -> io::Result<u32>;

    fn next_i32(&mut self) -> io::Result<i32>;

    fn next_bool(&mut self) -> io::Result<bool> {
        Ok(GmStream::next_i32(self)? > 0)
    }

    fn next_f64(&mut self) -> io::Result<f64>;

    fn next_string(&mut self) -> io::Result<String>;

    fn next_blob(&mut self) -> io::Result<Vec<u8>>;

    fn skip(&mut self, bytes: u32) -> io::Result<()>;

    fn next_compressed(&mut self) -> io::Result<io::Cursor<Vec<u8>>>;

    fn skip_blob(&mut self) -> io::Result<u32>;
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
        let data = self.next_blob()?;
        Ok(decode_string(&data))
    }

    fn next_blob(&mut self) -> io::Result<Vec<u8>> {
        let length = self.next_u32()?;
        let mut data = vec![0; length as usize];
        self.take(length as u64).read_exact(&mut data)?;
        Ok(data)
    }

    fn skip(&mut self, bytes: u32) -> io::Result<()> {
        let mut sub = self.take(bytes as u64);
        match io::copy(&mut sub, &mut io::sink()) {
            Err(e) => Err(e),
            Ok(_) => Ok(()),
        }
    }

    fn next_compressed(&mut self) -> io::Result<io::Cursor<Vec<u8>>> {
        let length = GmStream::next_u32(self)?;
        let substream = self.take(length as u64);
        let mut decoder = ZlibDecoder::new(substream);
        let mut buf = Vec::new();
        decoder.read_to_end(&mut buf)?;
        let cursor = io::Cursor::new(buf);
        Ok(cursor)
    }

    fn skip_blob(&mut self) -> io::Result<u32> {
        let length = GmStream::next_u32(self)?;
        self.skip(length)?;
        Ok(length)
    }
}

pub fn decode_string(data: &[u8]) -> String {
    let (decoded, _, _) = encoding_rs::WINDOWS_1252.decode(data);
    decoded.to_string()
}
