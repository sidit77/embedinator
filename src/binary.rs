use crate::binary::version::{FieldType, FieldValue};
use crate::{FixedVersionInfo, Icon, IconGroupEntry};

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

impl BinaryWritable for FixedVersionInfo {
    fn write_to<W: BinaryWriter>(&self, writer: &mut W) {
        let mut w = version::VersionWriter(writer);
        // https://learn.microsoft.com/en-us/windows/win32/menurc/vs-versioninfo
        w.write_field(FieldType::Binary, "VS_VERSION_INFO", FieldValue::header(|w | {
            // https://learn.microsoft.com/en-us/windows/win32/api/verrsrc/ns-verrsrc-vs_fixedfileinfo
            w.write_u32(0xFEEF04BD); //magic number
            w.write_u32(1 << 16); // struct version

            w.write_u16(self.file_version[1]);
            w.write_u16(self.file_version[0]);
            w.write_u16(self.file_version[3]);
            w.write_u16(self.file_version[2]);

            w.write_u16(self.product_version[1]);
            w.write_u16(self.product_version[0]);
            w.write_u16(self.product_version[3]);
            w.write_u16(self.product_version[2]);

            w.write_u32(0x3f); // fileflagsmask
            w.write_u32(self.file_flags.0);
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
        w.align_to(4);
    }
}

mod version {
    use crate::binary::BinaryWriter;

    #[derive(Debug, Copy, Clone, Eq, PartialEq)]
    #[repr(u16)]
    pub enum FieldType {
        Binary = 0x0,
        Text = 0x1
    }

    pub enum FieldValue<F> {
        None,
        Header(F),
        Other(u16)
    }

    impl<F: FnOnce(&mut VersionWriter<'_>)> FieldValue<F> {
        pub fn header(writer: F) -> Self {
            Self::Header(writer)
        }
    }

    impl FieldValue<fn(&mut VersionWriter<'_>)> {
        pub fn none() -> Self {
            Self::None
        }

        pub fn other(v: u16) -> Self {
            Self::Other(v)
        }
    }

    pub struct VersionWriter<'a>(pub &'a mut dyn BinaryWriter);

    impl<'a> VersionWriter<'a> {

        fn reserve_u16(&mut self) -> usize {
            let pos = self.pos();
            self.write_u16(0);
            pos
        }

        fn update_u16(&mut self, location: usize, v: u16) {
            self.write_bytes_at(location, &v.to_le_bytes())
        }

        pub fn write_utf16(&mut self, text: &str) {
            for c in text.encode_utf16() {
                self.write_u16(c);
            }
            self.write_u16(0x0);
        }

        pub fn write_field<F: FnOnce(&mut Self), B: FnOnce(&mut Self)>(&mut self, field_type: FieldType, key: &str, value: FieldValue<F>, body: B) {
            self.align_to(4);
            let field_start = self.pos();
            let field_length_pos = self.reserve_u16();
            let header_length_pos = self.reserve_u16();
            self.write_u16(field_type as u16);
            self.write_utf16(key);
            self.align_to(4);

            match value {
                FieldValue::None => {}
                FieldValue::Header(f) => {
                    let header_start = self.pos();
                    f(self);
                    let header_length = self.pos() - header_start;
                    self.update_u16(header_length_pos, header_length.try_into().expect("header too long"));
                    self.align_to(4);
                }
                FieldValue::Other(i) => self.update_u16(header_length_pos, i)
            }

            body(self);
            let field_length = self.pos() - field_start;
            self.update_u16(field_length_pos, field_length.try_into().expect("field is too long"));
        }
    }

    impl<'a> BinaryWriter for VersionWriter<'a> {
        fn pos(&self) -> usize {
            self.0.pos()
        }

        fn reserve(&mut self, amount: usize) {
            self.0.reserve(amount)
        }

        fn write_bytes(&mut self, data: &[u8]) {
            self.0.write_bytes(data)
        }

        fn write_bytes_at(&mut self, index: usize, data: &[u8]) {
            self.0.write_bytes_at(index, data)
        }
    }

}