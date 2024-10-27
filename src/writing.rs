use std::iter::repeat_n;
use crate::{FixedVersionInfo, ResourceFile};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u16)]
pub enum ResourceType {
    None = 0x0,
    Version = 0x10,
    Icon = 0x3,
    IconGroup = 0xE,
    Manifest = 0x18
}

impl ResourceType {

    fn flags(self) -> u16 {
        const MOVEABLE: u16 = 0x0010;
        const PURE : u16 = 0x0020;
        #[allow(dead_code)]
        const PRELOAD : u16 = 0x0040;
        const DISCARDABLE : u16 = 0x1000;

        match self {
            ResourceType::None => 0x0,
            ResourceType::Version => MOVEABLE | PURE,
            ResourceType::Icon => DISCARDABLE | MOVEABLE,
            ResourceType::IconGroup => DISCARDABLE | MOVEABLE | PURE,
            ResourceType::Manifest => MOVEABLE | PURE
        }
    }

}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u16)]
enum FieldType {
    Binary = 0x0,
    Text = 0x1
}

enum FieldValue<F> {
    None,
    Header(F),
    Other(u16)
}

impl<F: FnOnce(&mut ResourceFile)> FieldValue<F> {
    fn header(writer: F) -> Self {
        Self::Header(writer)
    }
}

impl FieldValue<fn(&mut ResourceFile)> {
    fn none() -> Self {
        Self::None
    }

    fn other(v: u16) -> Self {
        Self::Other(v)
    }
}

impl ResourceFile {

    fn write_u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes())
    }

    fn write_u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes())
    }

    fn realign(&mut self) {
        let required_padding = (4 - (self.pos() & 0b11)) & 0b11;
        self.0.extend(repeat_n(0, required_padding))
    }

    fn reserve_u32(&mut self) -> usize {
        let pos = self.pos();
        self.write_u32(0);
        pos
    }

    fn update_u32(&mut self, location: usize, v: u32) {
        self.0[location..(location + size_of::<u32>())].copy_from_slice(&v.to_le_bytes())
    }

    fn reserve_u16(&mut self) -> usize {
        let pos = self.pos();
        self.write_u16(0);
        pos
    }

    fn update_u16(&mut self, location: usize, v: u16) {
        self.0[location..(location + size_of::<u16>())].copy_from_slice(&v.to_le_bytes())
    }

    fn write_ident(&mut self, id: u16) {
        self.write_u16(0xffff);
        self.write_u16(id);
    }

    fn write_utf16(&mut self, text: &str) {
        for c in text.encode_utf16() {
            self.write_u16(c);
        }
        self.write_u16(0x0);
    }

    fn pos(&self) -> usize {
        self.0.len()
    }

    fn write_resource<F: FnOnce(&mut Self)>(&mut self, ty: ResourceType, name: u16, writer: F) {
        let header_start = self.pos();
        let data_size_loc = self.reserve_u32();
        let header_size_loc = self.reserve_u32();
        self.write_ident(ty as u16);
        self.write_ident(name);
        self.realign();
        self.write_u32(0); // format version
        self.write_u16(ty.flags());
        self.write_u16(match ty {
            ResourceType::None => 0x0,
            _ => 0x0409 // en-US
        });
        self.write_u32(0); // data version
        self.write_u32(0); // characteristics

        let header_len = self.pos() - header_start;
        self.update_u32(header_size_loc, header_len as u32);
        let data_start = self.pos();
        writer(self);
        let data_len = self.pos() - data_start;
        self.update_u32(data_size_loc, data_len as u32);
        self.realign();
    }

    pub(crate) fn write_empty(&mut self) {
        self.write_resource(ResourceType::None, 0, |_| {})
    }

    fn write_field<F: FnOnce(&mut Self), B: FnOnce(&mut Self)>(&mut self, field_type: FieldType, key: &str, value: FieldValue<F>, body: B) {
        self.realign();
        let field_start = self.pos();
        let field_length_pos = self.reserve_u16();
        let header_length_pos = self.reserve_u16();
        self.write_u16(field_type as u16);
        self.write_utf16(key);
        self.realign();

        match value {
            FieldValue::None => {}
            FieldValue::Header(f) => {
                let header_start = self.pos();
                f(self);
                let header_length = self.pos() - header_start;
                self.update_u16(header_length_pos, header_length.try_into().expect("header too long"));
                self.realign();
            }
            FieldValue::Other(i) => self.update_u16(header_length_pos, i)
        }

        body(self);
        let field_length = self.pos() - field_start;
        self.update_u16(field_length_pos, field_length.try_into().expect("field is too long"));
    }

    pub(crate) fn write_version(&mut self, fixed: FixedVersionInfo) {
        self.write_resource(ResourceType::Version, 1, |w| {
            // https://learn.microsoft.com/en-us/windows/win32/menurc/vs-versioninfo
            w.write_field(FieldType::Binary, "VS_VERSION_INFO",
            FieldValue::header(|w | {
                // https://learn.microsoft.com/en-us/windows/win32/api/verrsrc/ns-verrsrc-vs_fixedfileinfo
                w.write_u32(0xFEEF04BD); //magic number
                w.write_u32(1 << 16); // struct version

                w.write_u16(fixed.file_version[1]);
                w.write_u16(fixed.file_version[0]);
                w.write_u16(fixed.file_version[3]);
                w.write_u16(fixed.file_version[2]);

                w.write_u16(fixed.product_version[1]);
                w.write_u16(fixed.product_version[0]);
                w.write_u16(fixed.product_version[3]);
                w.write_u16(fixed.product_version[2]);

                w.write_u32(0x3f); // fileflagsmask
                w.write_u32(fixed.file_flags.0);
                w.write_u32(0x00040004); // VOS_NT_WINDOWS32
                w.write_u32(0x00000001); // VFT_APP
                w.write_u32(0x0);

                w.write_u32(0x0); //Timestamp
                w.write_u32(0x0);
            }),
            |w| {
                // https://learn.microsoft.com/en-us/windows/win32/menurc/stringfileinfo
                w.write_field(FieldType::Text, "StringFileInfo", FieldValue::none(), |w| {
                    // https://learn.microsoft.com/en-us/windows/win32/menurc/stringtable
                    w.write_field(FieldType::Text, "000004b0", FieldValue::none(), |w| {
                        let fields = [
                            ("ProductVersion", "0.1.0"),
                            ("FileVersion", "0.1.0"),
                            ("ProductName", "rusty-twinkle-tray"),
                            ("FileDescription", "rusty-twinkle-tray")
                        ];
                        for (k, v) in fields {
                            let l = u16::try_from(v.encode_utf16().count() + 1).expect("Key too long");
                            // https://learn.microsoft.com/en-us/windows/win32/menurc/string-str
                            w.write_field(FieldType::Text, k, FieldValue::other(l), |w| w.write_utf16(v));
                        }
                    });
                });
                // https://learn.microsoft.com/en-us/windows/win32/menurc/varfileinfo
                w.write_field(FieldType::Text, "VarFileInfo", FieldValue::none(), |w| {
                    w.write_field(FieldType::Binary, "Translation", FieldValue::header(|w| {
                        w.write_u32(0x04b00000);
                    }), |_| {})
                })
            });
        });
        self.realign();
    }

}
