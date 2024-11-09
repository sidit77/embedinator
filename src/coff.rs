use std::iter::repeat_n;
use crate::IconGroupEntry;

#[derive(Default)]
pub struct CoffWriter(Vec<u8>);

impl BinaryWriter for CoffWriter {
    fn pos(&self) -> usize {
        self.0.len()
    }

    fn reserve(&mut self, amount: usize) {
        self.0.extend(repeat_n(0, amount))
    }

    fn write_bytes(&mut self, data: &[u8]) {
        self.0.extend_from_slice(data)
    }

    fn write_bytes_at(&mut self, index: usize, data: &[u8]) {
        self.0[index..(index + data.len())].copy_from_slice(data)
    }
}

impl CoffWriter {


    /// 16 bytes
    pub fn write_directory_table(&mut self, entries: u16) {
        let _ = entries;
    }

    /// 8 bytes
    pub fn write_directory_entry(&mut self, id: u32, offset: u32, leaf: bool) {
        let _ = (id, offset, leaf);
    }

    /// 16 bytes
    pub fn write_data_entry(&mut self, offset: u32, size: u32) {
        let _ = (offset, size);
    }

    pub fn write_directory(&mut self, entries: u16) -> CoffDirectoryWriter {
        CoffDirectoryWriter::allocate(self, entries)
    }

}

pub struct CoffDirectoryWriter {
    current_index: usize,
    final_index: usize
}

impl CoffDirectoryWriter {
    const ENTRY_SIZE: usize = 2 * size_of::<u32>();

    fn allocate(cw: &mut CoffWriter, entries: u16) -> Self {

        cw.write_u32(0); // Characteristics
        cw.write_u32(0); // TimeDateStamp
        cw.write_u16(0); // MajorVersion
        cw.write_u16(0); // MinorVersion
        cw.write_u16(0); // NumberOfNamedEntries
        cw.write_u16(entries); // NumberOfIdEntries

        let current = cw.pos();
        cw.reserve(entries as usize * Self::ENTRY_SIZE);
        Self {
            current_index: current,
            final_index: cw.pos(),
        }
    }

    fn write_entry(&mut self, cw: &mut CoffWriter, id: u32, leaf: bool) {
        const SUB_DIR_BIT: usize = 0x80000000;
        assert!(self.current_index < self.final_index, "Tried to add more entries to a directory than allowed ({} {})", self.current_index, self.final_index);
        let mut offset = cw.pos();
        assert_eq!(offset & SUB_DIR_BIT, 0, "Too much data");

        if !leaf {
            offset |= SUB_DIR_BIT;
        }

        cw.write_u32_at(self.current_index + 0, id);
        cw.write_u32_at(self.current_index + size_of::<u32>(), offset as u32);

        self.current_index += Self::ENTRY_SIZE;
    }
    
    pub fn subdirectory(&mut self, cw: &mut CoffWriter, id: u32, entries: u16) -> CoffDirectoryWriter {
        self.write_entry(cw, id, false);
        Self::allocate(cw, entries)
    }
    
    pub fn data_entry(&mut self, cw: &mut CoffWriter, id: u32) -> CoffDataEntry {
        self.write_entry(cw, id, true);
        CoffDataEntry::allocate(cw)
    }
    
}

impl Drop for CoffDirectoryWriter {
    fn drop(&mut self) {
        assert_eq!(self.current_index, self.final_index, "Not all entries were written")
    }
}

pub struct CoffDataEntry{
    index: usize,
    written: bool
}

impl CoffDataEntry {

    const ENTRY_SIZE: usize = 4 * size_of::<u32>();

    fn allocate(cw: &mut CoffWriter) -> Self {
        let current = cw.pos();
        cw.reserve(Self::ENTRY_SIZE);
        Self {
            index: current,
            written: false,
        }
    }
    
    pub fn write_data<B: BinaryWritable + ?Sized>(&mut self, coff: &mut CoffWriter, data: &B) {
        let start = coff.pos();

        data.write_to(coff);

        let length = coff.pos() - start;
        coff.align_to(8);

        coff.write_u32_at(self.index + 0 * size_of::<u32>(), start as u32);  // OffsetToData
        coff.write_u32_at(self.index + 1 * size_of::<u32>(), length as u32); // Size
        coff.write_u32_at(self.index + 2 * size_of::<u32>(), 0);          // CodePage
        coff.write_u32_at(self.index + 3 * size_of::<u32>(), 0);          // Reserve

        self.written = true;
    }

}

impl Drop for CoffDataEntry {
    fn drop(&mut self) {
        assert!(self.written, "A data entry was never written")
    }
}


pub trait BinaryWriter {

    fn pos(&self) -> usize;
    fn reserve(&mut self, amount: usize);

    fn write_bytes(&mut self, data: &[u8]);
    fn write_bytes_at(&mut self, index: usize, data: &[u8]);

    fn write_u32(&mut self, v: u32) {
        self.write_bytes(&v.to_le_bytes())
    }

    fn write_u32_at(&mut self, index: usize, v: u32) {
        self.write_bytes_at(index, &v.to_le_bytes())
    }

    fn write_u16(&mut self, v: u16) {
        self.write_bytes(&v.to_le_bytes())
    }

    fn write_u8(&mut self, v: u8) {
        self.write_bytes(&v.to_le_bytes())
    }

    fn align_to(&mut self, i: usize) {
        let required_padding = (i - (self.pos() % i));
        self.reserve(required_padding)
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

pub struct IconGroupWriter<'a>(pub &'a [IconGroupEntry]);

impl<'a> BinaryWritable for IconGroupWriter<'a> {
    fn write_to<W: BinaryWriter>(&self, w: &mut W) {
        // it doesn't seems to matter what we write for most of these fields
        w.write_u16(0x0); // idReserved
        w.write_u16(0x1); // idType
        w.write_u16(self.0.len().try_into().expect("Too many icons in group")); // idCount

        for entry in self.0 {
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