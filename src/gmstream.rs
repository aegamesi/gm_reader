use std::io::{Read, Seek};

extern crate byteorder;

use byteorder::{ReadBytesExt, LittleEndian};
use std::io;


pub trait GmStream: Read + Seek {
    fn read_u32(&mut self) -> io::Result<u32>;

    fn read_u16(&mut self) -> io::Result<u16>;

    fn read_u8(&mut self) -> io::Result<u8>;

    fn read_bool(&mut self) -> io::Result<bool>;

    fn read_string(&mut self) -> io::Result<String>;

    fn read_f64(&mut self) -> io::Result<f64>;
}

impl<T> GmStream for T where T: Read + Seek {
    fn read_u32(&mut self) -> io::Result<u32> {
        ReadBytesExt::read_u32::<LittleEndian>(self)
    }

    fn read_u16(&mut self) -> io::Result<u16> {
        ReadBytesExt::read_u16::<LittleEndian>(self)
    }

    fn read_u8(&mut self) -> io::Result<u8> {
        ReadBytesExt::read_u8(self)
    }

    fn read_bool(&mut self) -> io::Result<bool> {
        Ok(GmStream::read_u32(self)? > 0)
    }

    fn read_string(&mut self) -> io::Result<String> {
        let length = GmStream::read_u32(self)?;
        let mut string = String::with_capacity(length as usize);
        self.take(length as u64).read_to_string(&mut string)?;
        Ok(string)
    }

    fn read_f64(&mut self) -> io::Result<f64> {
        ReadBytesExt::read_f64::<LittleEndian>(self)
    }
}