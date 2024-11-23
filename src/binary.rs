use crate::{Icon, IconGroupEntry};

pub trait BinaryWriter {

    fn pos(&self) -> usize;
    fn reserve(&mut self, amount: usize);

    fn write_bytes(&mut self, data: &[u8]);
    fn write_bytes_at(&mut self, index: usize, data: &[u8]);

    fn write_u32(&mut self, v: u32) {
        self.write_bytes(&v.to_le_bytes())
    }

    fn write_u16(&mut self, v: u16) {
        self.write_bytes(&v.to_le_bytes())
    }

    fn write_u8(&mut self, v: u8) {
        self.write_bytes(&v.to_le_bytes())
    }

    fn slice(&mut self, index: usize, length: usize) -> BinarySlice<'_> where Self: Sized {
        BinarySlice {
            inner: self,
            start: index,
            length,
            pos: 0,
        }
    }

    fn align_to(&mut self, i: usize) {
        let required_padding = (i - (self.pos() % i)) % i;
        self.reserve(required_padding)
    }

}

pub struct BinarySlice<'a> {
    inner: &'a mut dyn BinaryWriter,
    start: usize,
    length: usize,
    pos: usize
}

impl<'a> BinaryWriter for BinarySlice<'a> {
    fn pos(&self) -> usize {
        self.pos
    }

    fn reserve(&mut self, _: usize) {
        unimplemented!()
    }

    fn write_bytes(&mut self, data: &[u8]) {
        assert!(self.pos + data.len() <= self.length);
        self.inner.write_bytes_at(self.start + self.pos, data);
        self.pos += data.len();
    }

    fn write_bytes_at(&mut self, _: usize, _: &[u8]) {
        unimplemented!()
    }
}

impl<'a> Drop for BinarySlice<'a> {
    fn drop(&mut self) {
        assert_eq!(self.pos, self.length)
    }
}

pub trait BinaryWritable {
    fn write_to<W: BinaryWriter>(&self, writer: &mut W);
}

impl BinaryWritable for Vec<u8> {
    fn write_to<W: BinaryWriter>(&self, writer: &mut W) {
        writer.write_bytes(self)
    }
}

impl BinaryWritable for String {
    fn write_to<W: BinaryWriter>(&self, writer: &mut W) {
        writer.write_bytes(self.as_bytes())
    }
}

impl BinaryWritable for [u8] {
    fn write_to<W: BinaryWriter>(&self, writer: &mut W) {
        writer.write_bytes(self)
    }
}

impl BinaryWritable for [IconGroupEntry] {
    fn write_to<W: BinaryWriter>(&self, w: &mut W) {
        // it doesn't seems to matter what we write for most of these fields
        w.write_u16(0x0); // idReserved
        w.write_u16(0x1); // idType
        w.write_u16(self.len().try_into().expect("Too many icons in group")); // idCount

        for entry in self {
            w.write_u8(0x0); // bWidth
            w.write_u8(0x0); // bHeight
            w.write_u8(0x0); // bColorCount
            w.write_u8(0x0); // bReserved
            w.write_u16(0x1); // wPlanes
            w.write_u16(32); // wBitCount
            w.write_u32(entry.icon_size.try_into().expect("icon file too large")); // dwBytesInRes
            w.write_u16(entry.icon_id);
        }
    }
}

impl BinaryWritable for Icon {
    fn write_to<W: BinaryWriter>(&self, w: &mut W) {
        w.write_bytes(&self.0)
    }
}

impl BinaryWritable for () {
    fn write_to<W: BinaryWriter>(&self, _: &mut W) {
        // do nothing
    }
}