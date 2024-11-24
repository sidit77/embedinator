use std::iter::repeat_n;
use std::time::SystemTime;
use crate::binary::{BinaryWritable, BinaryWriter};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TargetType {
    Aarch64,
    I386,
    X86_64
}

impl TargetType {
    fn id(self) -> u16 {
        match self {
            TargetType::Aarch64 => 0xaa64,
            TargetType::I386 => 0x014c,
            TargetType::X86_64 => 0x8664,
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RelocationType {
    Rva32
}

impl RelocationType {

    pub fn id(self, target: TargetType) -> u16 {
        const IMAGE_REL_AMD64_ADDR32NB: u16 = 0x0003;
        const IMAGE_REL_ARM64_ADDR32NB: u16 = 0x0002;
        const IMAGE_REL_I386_DIR32NB: u16 = 0x0007;
        match self {
            RelocationType::Rva32 => match target {
                TargetType::Aarch64 => IMAGE_REL_ARM64_ADDR32NB,
                TargetType::I386 => IMAGE_REL_I386_DIR32NB,
                TargetType::X86_64 => IMAGE_REL_AMD64_ADDR32NB
            }
        }
    }

}

pub struct CoffWriter {
    data: Vec<u8>,
    relocations: u16,
    data_start: usize,
    relocation_start: usize,
    target: TargetType
}

impl CoffWriter {

    const HEADER_SIZE: usize = 60;

    pub fn new(target: TargetType) -> Self {
        Self {
            data: vec![0u8; Self::HEADER_SIZE],
            relocations: 0,
            data_start: Self::HEADER_SIZE,
            relocation_start: 0,
            target,
        }
    }

    pub fn write_directory(&mut self, entries: u16) -> CoffDirectoryWriter {
        CoffDirectoryWriter::allocate(self, entries)
    }

    pub fn write_relocation(&mut self, address: u32, ty: RelocationType) {
        self.relocations += 1;
        self.write_u32(address);
        self.write_bytes(&[0, 0, 0, 0]);
        self.write_u16(ty.id(self.target));
    }

    pub fn start_relocations(&mut self) {
        self.relocation_start = self.pos();
    }

    pub fn finish(mut self) -> Vec<u8> {
        let target = self.target;
        let relocations = self.relocations;
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as u32);

        let pointer_to_symbol_table = self.pos();
        let relocation_start = self.relocation_start;
        let data_size = relocation_start - self.data_start;

        // Write the symbols and auxiliary data for the section.
        self.write_bytes(b".rsrc\0\0\0"); // name
        self.write_bytes(&[0, 0, 0, 0]); // address
        self.write_bytes(&[1, 0]); // section number (1-based)
        self.write_bytes(&[0, 0, 3, 1]); // type = 0, class = static, aux symbols = 1
        self.write_u32(data_size as u32);
        self.write_u16(self.relocations);
        self.write_bytes(&[0; 12]);

        // Write the empty string table.
        self.write_bytes(&[0; 4]);

        {
            let mut h = self.slice(0, Self::HEADER_SIZE);

            h.write_u16(target.id());
            h.write_bytes(&[1, 0]); // number of sections
            h.write_u32(timestamp);
            h.write_u32(pointer_to_symbol_table as u32);
            h.write_bytes(&[2, 0, 0, 0]); // number of symbol table entries
            h.write_bytes(&[0; 4]); // optional header size = 0, characteristics = 0

            // Write the section header.
            h.write_bytes(b".rsrc\0\0\0");
            h.write_u32(0); // physical address
            h.write_u32(0); // virtual address
            h.write_u32(data_size as u32);
            h.write_bytes(&[60, 0, 0, 0]); // pointer to raw data
            h.write_u32(relocation_start as u32); // pointer to relocations
            h.write_bytes(&[0; 4]); // pointer to line numbers
            h.write_u16(relocations);
            h.write_bytes(&[0; 2]); // number of line numbers
            h.write_bytes(&[0x40, 0, 0x30, 0xc0]); // characteristics

        }


        self.data
    }

}

impl BinaryWriter for CoffWriter {
    fn pos(&self) -> usize {
        self.data.len()
    }

    fn reserve(&mut self, amount: usize) {
        self.data.extend(repeat_n(0, amount))
    }

    fn write_bytes(&mut self, data: &[u8]) {
        self.data.extend_from_slice(data)
    }

    fn write_bytes_at(&mut self, index: usize, data: &[u8]) {
        self.data[index..(index + data.len())].copy_from_slice(data)
    }

    fn align_to(&mut self, i: usize) {
        let required_padding = i - ((self.pos() - Self::HEADER_SIZE) % i);
        self.reserve(required_padding)
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
        let mut offset = cw.pos() - CoffWriter::HEADER_SIZE;
        assert_eq!(offset & SUB_DIR_BIT, 0, "Too much data");

        if !leaf {
            offset |= SUB_DIR_BIT;
        }

        let mut slice = cw.slice(self.current_index, Self::ENTRY_SIZE);
        slice.write_u32(id);
        slice.write_u32(offset as u32);

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
        coff.align_to(8);
        let start = coff.pos();

        data.write_to(coff);

        let length = coff.pos() - start;
        coff.align_to(8);

        let offset = start - CoffWriter::HEADER_SIZE;
        let mut slice = coff.slice(self.index, Self::ENTRY_SIZE);
        slice.write_u32(offset as u32);  // OffsetToData
        slice.write_u32(length as u32); // Size
        slice.write_u32(0);          // CodePage
        slice.write_u32(0);          // Reserve

        self.written = true;
    }

    pub fn write_relocation(&self, coff: &mut CoffWriter) {
        coff.write_relocation((self.index - coff.data_start) as u32, RelocationType::Rva32)
    }

}

impl Drop for CoffDataEntry {
    fn drop(&mut self) {
        assert!(self.written, "A data entry was never written")
    }
}


