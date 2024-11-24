mod writer;

use std::collections::BTreeMap;
use std::io::Write;
use std::time::SystemTime;
use crate::binary::{BinaryWritable, BinaryWriter};
use crate::coff2::writer::FileWriter;
use crate::{ResourceType, TargetType};

struct Section {
    name: [u8; 8],
    pointer_to_raw_data: usize,
    size_of_raw_data: usize,
    pointer_to_relocations: usize,
    number_of_relocations: usize,
}

#[derive(Default, Copy, Clone)]
enum Symbol {
    #[default]
    Placeholder,
    Section {
        name: [u8; 8],
        section_number: u16,
    },
    SectionAux {
        length: u32,
        number_of_relocations: u16
    },
    Resource {
        name: [u8; 8],
        offset: u32,
        section_number: u16,
    }
}

pub struct CoffWriter2 {
    target_type: TargetType,
    table: BTreeMap<ResourceType, BTreeMap<ResourceId, BTreeMap<LanguageId, ResourceLocation>>>,
    data: FileWriter,
    symbols: Vec<Symbol>,
}

impl CoffWriter2 {
    const TABLE_SYMBOL: usize = 0;
    const DATA_SYMBOL: usize = 2;

    pub fn new(target_type: TargetType) -> Self {
        Self {
            target_type,
            table: Default::default(),
            data: Default::default(),
            symbols: vec![Symbol::default(); 4],
        }
    }

    pub fn add_resource<W: BinaryWritable + ?Sized>(&mut self, ty: ResourceType, id: u32, data: &W) {
        let (offset, size) = {
            let offset = self.data.pos();
            data.write_to(&mut self.data);
            (offset, self.data.pos() - offset)
        };
        self.data.align_to(8);
        let mut name = [0u8; 8];
        write!(name.as_mut_slice(), "$R{:06X}", offset)
            .expect("Failed to generate symbol name");
        let symbol_id = self.symbols.len();
        self.symbols.push(Symbol::Resource { name, offset: offset as u32, section_number: 2 });

        self.table
            .entry(ty)
            .or_default()
            .entry(ResourceId(id))
            .or_default()
            .insert(LanguageId::LANG_US, ResourceLocation { offset, size, symbol_id });
    }


    fn write_symbol_table(&mut self, file: &mut FileWriter) -> (usize, usize) {
        file.align_to(4);
        let symbol_table_pointer = file.pos();

        for symbol in &self.symbols {
            match *symbol {
                Symbol::Placeholder => panic!("Placeholder symbol not replaced"),
                Symbol::Section { name, section_number } => {
                    file.write_bytes(&name); // Name
                    file.write_u32(0); // Value
                    file.write_u16(section_number); // Section number
                    file.write_u16(0); // Type
                    file.write_u8(IMAGE_SYM_CLASS_STATIC); // Storage class
                    file.write_u8(1); // Number of auxiliary symbols
                }
                Symbol::SectionAux { length, number_of_relocations } => {
                    file.write_u32(length); // Length
                    file.write_u16(number_of_relocations); // Number of relocations
                    file.write_u16(0); // Number of lines
                    file.reserve(10); // Checksum, Number, Selection, Unused
                }
                Symbol::Resource { name, section_number, offset } => {
                    file.write_bytes(&name); // Name
                    file.write_u32(offset); // Value
                    file.write_u16(section_number); // Section number
                    file.write_u16(0); // Type
                    file.write_u8(IMAGE_SYM_CLASS_STATIC); // Storage class
                    file.write_u8(0); // Number of auxiliary symbols
                }
            }
        }
        let number_of_symbols = self.symbols.len();

        (symbol_table_pointer, number_of_symbols)
    }

    fn write_table_section(&mut self, file: &mut FileWriter) -> Section {
        file.mark_section_start();
        let pointer_to_raw_data = file.pos();


        let mut relocations = Vec::new();

        file.write_table(&self.table, |file, entry| {
            file.write_table(entry, |file, entry| {
                file.write_table(entry, |file, entry| {
                    relocations.push((file.current_offset(), entry.symbol_id));
                    file.write_u32(0); // Data RVA
                    file.write_u32(entry.size as u32); // Size
                    file.write_u32(0); // Code page
                    file.write_u32(entry.size as u32); // Reserved
                    false
                });
                true
            });
            true
        });
        file.align_to(4);
        let size_of_raw_data = file.pos() - pointer_to_raw_data;
        let pointer_to_relocations = file.pos();
        let number_of_relocations = relocations.len();
        relocations.sort_by_key(|&(_, symbol_id)| symbol_id);
        for (rva, symbol_id) in relocations {
            file.write_u32(rva as u32);
            file.write_u32(symbol_id as u32);
            file.write_u16(RelocationType::Rva32.id(self.target_type));
        }
        file.align_to(4);

        Section {
            name: *b".rsrc$01",
            pointer_to_raw_data,
            size_of_raw_data,
            pointer_to_relocations,
            number_of_relocations,
        }
    }

    fn write_data_section(&mut self, file: &mut FileWriter) -> Section {
        file.mark_section_start();
        let pointer_to_raw_data = file.pos();
        file.write_bytes(&self.data.data);
        file.align_to(4);
        let size_of_raw_data = file.pos() - pointer_to_raw_data;
        Section {
            name: *b".rsrc$02",
            pointer_to_raw_data,
            size_of_raw_data,
            pointer_to_relocations: 0,
            number_of_relocations: 0,
        }
    }

    fn write_sections(&mut self, file: &mut FileWriter) -> [Section; 2] {
        let table_section = self.write_table_section(file);
        let data_section = self.write_data_section(file);

        self.symbols[Self::TABLE_SYMBOL] = Symbol::Section {
            name: table_section.name,
            section_number: 1,
        };
        self.symbols[Self::TABLE_SYMBOL + 1] = Symbol::SectionAux {
            length: table_section.size_of_raw_data as u32,
            number_of_relocations: table_section.number_of_relocations as u16,
        };
        self.symbols[Self::DATA_SYMBOL] = Symbol::Section {
            name: data_section.name,
            section_number: 2,
        };
        self.symbols[Self::DATA_SYMBOL + 1] = Symbol::SectionAux {
            length: data_section.size_of_raw_data as u32,
            number_of_relocations: data_section.number_of_relocations as u16,
        };
        [table_section, data_section]
    }

    pub fn finish(mut self) -> Vec<u8> {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as u32);

        let mut file = FileWriter::default();

        file.set_pos(FILE_HEADER_SIZE + SECTION_HEADER_SIZE * 2);
        let sections = self.write_sections(&mut file);
        let (symbol_table_pointer, symbol_numer) = self.write_symbol_table(&mut file);
        file.write_bytes(&[0; 4]); // String table

        file.set_pos(0);
        file.write_u16(self.target_type.id());
        file.write_u16(2); // number of sections
        file.write_u32(timestamp);
        file.write_u32(symbol_table_pointer as u32);
        file.write_u32(symbol_numer as u32);
        file.write_u16(0); // optional header size
        file.write_u16(IMAGE_FILE_32BIT_MACHINE); // flags
        assert_eq!(file.pos(), FILE_HEADER_SIZE);

        for section in sections {
            file.write_bytes(&section.name);
            file.write_u32(0); // physical address
            file.write_u32(0); // virtual address
            file.write_u32(section.size_of_raw_data as u32);
            file.write_u32(section.pointer_to_raw_data as u32);
            file.write_u32(section.pointer_to_relocations as u32);
            file.write_u32(0); // pointer to line numbers
            file.write_u16(section.number_of_relocations as u16);
            file.write_u16(0); // number of line numbers
            file.write_u32(IMAGE_SCN_CNT_INITIALIZED_DATA | IMAGE_SCN_MEM_READ | IMAGE_SCN_MEM_WRITE);
        }
        assert_eq!(file.pos(), FILE_HEADER_SIZE + 2 * SECTION_HEADER_SIZE);

        file.data
    }
}

impl FileWriter {

    pub fn write_table<K, V, F>(&mut self, table: &BTreeMap<K, V>, mut write_entry: F)
        where K: Copy + Into<u32>, F: FnMut(&mut Self, &V) -> bool
    {
        self.write_u32(0); // Characteristics
        self.write_u32(0); // TimeDateStamp
        self.write_u16(0); // MajorVersion
        self.write_u16(0); // MinorVersion
        self.write_u16(0); // NumberOfNamedEntries
        self.write_u16(table.len() as u16); // NumberOfIdEntries
        let table_base = self.pos();
        let mut frontier = table_base + table.len() * RESOURCE_TABLE_ENTRY_SIZE;
        for (i, (ty, entry)) in table.iter().enumerate() {
            self.set_pos(frontier);
            let offset = self.current_offset();
            let subdir = write_entry(self, entry);
            frontier = self.pos();
            self.set_pos(table_base + i * RESOURCE_TABLE_ENTRY_SIZE);
            self.write_u32((*ty).into());
            self.write_u32(offset as u32 | (subdir as u32) << 31);
        }
        self.set_pos(frontier);
    }

}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
struct ResourceId(u32);

impl From<ResourceId> for u32 {
    fn from(id: ResourceId) -> u32 {
        id.0
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
struct LanguageId(u32);

impl LanguageId {
    const LANG_US: Self = Self(0x0409);
}

impl From<LanguageId> for u32 {
    fn from(id: LanguageId) -> u32 {
        id.0
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct ResourceLocation {
    offset: usize,
    size: usize,
    symbol_id: usize
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum RelocationType {
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


const FILE_HEADER_SIZE: usize = 20;
const SECTION_HEADER_SIZE: usize = 40;

const RESOURCE_TABLE_ENTRY_SIZE: usize = 8;

const IMAGE_SYM_CLASS_STATIC: u8 = 0x03;

const IMAGE_FILE_32BIT_MACHINE: u16 = 0x0100;

const IMAGE_SCN_CNT_INITIALIZED_DATA: u32 = 0x00000040;
const IMAGE_SCN_MEM_READ: u32 = 0x40000000;
const IMAGE_SCN_MEM_WRITE: u32 = 0x80000000;
