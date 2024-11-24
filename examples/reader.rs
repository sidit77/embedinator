use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let mut data = Reader::new(std::fs::read("cvtres.lib")?);
    println!("File:");
    println!("    Architecture: {:#X}", data.read_u16());
    let number_of_sections = data.read_u16();
    println!("    Sections: {}", number_of_sections);
    println!("    Timestamp: {:#X}", data.read_u32());
    let symbol_table_pointer = data.read_u32() as usize;
    println!("    Symbol table pointer: {:#X}", symbol_table_pointer);
    let number_of_symbols = data.read_u32() as usize;
    println!("    Number of symbols: {}", number_of_symbols);
    let optional_header_size = data.read_u16();
    println!("    Optional header size: {}", optional_header_size);
    println!("    Flags: {:#X}", data.read_u16()); // IMAGE_FILE_32BIT_MACHINE = 0x100

    println!();
    let mut sections = Vec::new();
    for i in 0..number_of_sections {
        println!("Section {} // Position: {:#X}", i, data.pos());
        let name = String::from_utf8_lossy(&data.read::<8>()).to_string();
        println!("    Name: {:?}", name);
        println!("    Physical Address: {:#X}", data.read_u32());
        println!("    Virtual Address: {:#X}", data.read_u32());
        let section_size = data.read_u32() as usize;
        println!("    Section size: {}", section_size);
        let pointer_to_raw_data = data.read_u32() as usize;
        println!("    Pointer to raw data: {:#X}", pointer_to_raw_data);
        let pointer_to_relocations = data.read_u32() as usize;
        println!("    Pointer to relocations: {:#X}", pointer_to_relocations);
        println!("    Pointer to line numbers: {:#X}", data.read_u32());
        let number_of_relocations = data.read_u16() as usize;
        println!("    Number of relocations: {}", number_of_relocations);
        println!("    Number of line numbers: {}", data.read_u16());

        // IMAGE_SCN_CNT_INITIALIZED_DATA  0x00000040  The section contains initialized data
        // IMAGE_SCN_MEM_READ              0x40000000  The section can be read.
        // IMAGE_SCN_MEM_WRITE             0x80000000  The section can be written to.
        println!("    Characteristics: {:#X}", data.read_u32());

        sections.push(Section {
            name,
            pointer_to_raw_data,
            size_of_raw_data: section_size,
            relocations: if number_of_relocations > 0 {
                assert_ne!(pointer_to_relocations, 0);
                Some(Relocations {
                    position: pointer_to_relocations,
                    count: number_of_relocations,
                })
            } else {
                None
            },
        });
    }
    println!("//End of header: {:#X}", data.pos());

    println!();
    {
        println!("Section 0 data:");
        let section = &sections[0];
        data.set_pos(section.pointer_to_raw_data);
        parse_res_table(&mut data, 1, section.pointer_to_raw_data);
    }




    println!();
    data.set_pos(symbol_table_pointer);
    for i in 0..number_of_symbols {
        println!("Symbol {}", i);
        let name = String::from_utf8_lossy(&data.read::<8>()).to_string();
        println!("    Name: {:?}", name);
        println!("    Value: {:#X}", data.read_u32());
        println!("    Section Number: {}", parse_section_number(data.read_u16()));
        println!("    Type: {:#X}", data.read_u16());
        println!("    Storage Class: {}", parse_section_storage_class(data.read_u8()));
        println!("    Number of Auxiliary Symbols: {:#X}", data.read_u8());
    }



    for section in &sections {
        if let Some(relocations) = &section.relocations {
            println!("Relocations for section {:?}", section.name);
            data.set_pos(relocations.position);
            for i in 0..relocations.count {
                println!("    Relocation {}", i);
                println!("        Virtual Address: {:#X}", data.read_u32());
                println!("        Symbol Table Index: {:#X}", data.read_u32());
                println!("        Type: {:#X}", data.read_u16());
            }
        }
    }


    Ok(())
}

fn parse_res_table(data: &mut Reader, level: u32, table_base: usize) {
    const SUB_DIR_BIT: usize = 0x80000000;

    let indent = "   ".repeat(level as usize);
    println!("{indent}Characteristics: {}", data.read_u32());
    println!("{indent}TimeDateStamp: {:#X}", data.read_u32());
    println!("{indent}MajorVersion: {}", data.read_u16());
    println!("{indent}MinorVersion: {}", data.read_u16());
    assert_eq!(data.read_u16(), 0); // NumberOfNamedEntries
    let number_of_id_entries = data.read_u16() as usize;
    let base = data.pos();
    let indent = "   ".repeat(level as usize + 1);
    for i in 0..number_of_id_entries {
        data.set_pos(base + i * 8);
        println!("{indent}Entry {:#X}:", data.read_u32());
        let offset = data.read_u32() as usize;
        data.set_pos(table_base + (offset & !SUB_DIR_BIT));
        if offset & SUB_DIR_BIT != 0 {
            parse_res_table(data, level + 1, table_base);
        } else {
            println!("{indent}//Offset: {:#X}", data.pos() - table_base);
            println!("{indent}DataRVA: {:#X}", data.read_u32());
            println!("{indent}DataSize: {}", data.read_u32());
            println!("{indent}CodePage: {}", data.read_u32());
            assert_eq!(data.read_u32(), 0); // Reserved
        }

    }

}

fn parse_section_number(section_number: u16) -> String {
    match section_number as i16 {
        0 => "UNDEFINED".to_string(),
        -1 => "ABSOLUTE".to_string(),
        -2 => "DEBUG".to_string(),
        _ => format!("Section {}", section_number),
    }
}

fn parse_section_storage_class(storage_class: u8) -> &'static str {
    match storage_class {
        0x0 => "NULL",
        0x3 => "STATIC",
        _ => unimplemented!()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct Section {
    name: String,
    pointer_to_raw_data: usize,
    size_of_raw_data: usize,
    relocations: Option<Relocations>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct Relocations {
    position: usize,
    count: usize,
}

struct Reader {
    data: Vec<u8>,
    offset: usize,
}

impl Reader {

    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            offset: 0,
        }
    }

    pub fn set_pos(&mut self, pos: usize) {
        self.offset = pos;
    }

    pub fn pos(&self) -> usize {
        self.offset
    }

    pub fn read<const N: usize>(&mut self) -> [u8; N] {
        let mut buf = [0u8; N];
        buf.copy_from_slice(&self.data[self.offset..self.offset + N]);
        self.offset += N;
        buf
    }

    pub fn read_u8(&mut self) -> u8 {
        u8::from_le_bytes(self.read())
    }

    pub fn read_u16(&mut self) -> u16 {
        u16::from_le_bytes(self.read())
    }

    pub fn read_u32(&mut self) -> u32 {
        u32::from_le_bytes(self.read())
    }

}
