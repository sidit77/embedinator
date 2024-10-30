use std::ops::BitOr;

mod writing;

pub struct FileFlags(u32);

impl FileFlags {
    pub const NONE: Self = Self(0x0);

    /// The file contains debugging information or is compiled with debugging features enabled.
    pub const DEBUG: Self = Self(0x01);

    /// The file's version structure was created dynamically; therefore, some of the members in this structure may be empty or incorrect.
    /// This flag should never be set in a file's VS_VERSIONINFO data.
    pub const INFOINFERRED: Self = Self(0x10);

    /// The file has been modified and is not identical to the original shipping file of the same version number.
    pub const PATCHED: Self = Self(0x04);

    /// The file is a development version, not a commercially released product.
    pub const PRERELEASE: Self = Self(0x02);

    /// The file was not built using standard release procedures.
    /// If this flag is set, the StringFileInfo structure should contain a PrivateBuild entry.
    pub const PRIVATEBUILD: Self = Self(0x08);

    /// The file was built by the original company using standard release procedures but is a variation of the normal file of the same version number.
    /// If this flag is set, the StringFileInfo structure should contain a SpecialBuild entry.
    pub const SPECIALBUILD: Self = Self(0x20);
}

impl BitOr for FileFlags {
    type Output = FileFlags;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

pub struct FixedVersionInfo {
    file_version: [u16; 4],
    product_version: [u16; 4],
    file_flags: FileFlags
}

#[derive(Clone, Eq, PartialEq)]
pub struct Icon(Vec<u8>);

impl Icon {

    pub fn from_png_bytes(data: Vec<u8>) -> Self {
        assert_eq!(&data[..8], &[137, 80, 78, 71, 13, 10, 26, 10], "Invalid PNG file");
        assert_eq!(&data[12..16], b"IHDR", "Invalid PNG file");
        // let width = u32::from_be_bytes((&data[16..20]).try_into().unwrap());
        // let height = u32::from_be_bytes((&data[20..24]).try_into().unwrap());
        let bit_depth = data[24];
        let color_type = data[25];
        assert_eq!((color_type, bit_depth), (6, 8), "The png must contain 32bpp RGBA data");
        Self(data)
    }

}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct IconGroupEntry {
    icon_id: u16,
    icon_size: usize
}

#[derive(Default, Clone)]
pub struct ResourceBuilder {
    icon_groups: Vec<(u16, [IconGroupEntry; 1])>,
    icons: Vec<(u16, Icon)>
}

impl ResourceBuilder {

    pub fn add_icon(mut self, id: u16, icon: Icon) -> Self {
        assert!(!self.icon_groups.iter().any(|(i, _ )| *i == id), "Duplicate icon id");
        const ICON_BASE_ID: u16 = 128;
        let icon_id = ICON_BASE_ID + self.icons.len() as u16;
        self.icon_groups.push((id, [IconGroupEntry {
            icon_id,
            icon_size: icon.0.len(),
        }]));
        self.icons.push((icon_id, icon));
        self
    }

    pub fn compile(&self) -> ResourceFile {
        let mut res = ResourceFile(Vec::new());

        res.write_empty(); // Files seem to start with an empty resource
        res.write_version(FixedVersionInfo {
            file_version: [0, 1, 0, 0],
            product_version: [0, 1, 0, 0],
            file_flags: FileFlags::NONE,
        });
        for (id, icon) in &self.icons {
            res.write_icon(*id, icon);
        }
        for (id, entries) in &self.icon_groups {
            res.write_icon_group(*id, entries)
        }
        res
    }

}

#[must_use]
#[derive(Clone, Eq, PartialEq)]
pub struct ResourceFile(Vec<u8>);

impl ResourceFile {

    pub fn write_to_file(&self) -> std::io::Result<()> {
        std::fs::write("test.res", &self.0)
    }

    pub fn save_and_link(&self) -> std::io::Result<()> {
        let out_dir = std::env::var("OUT_DIR")
            .expect("No OUT_DIR env var");
        let out_file = format!("{out_dir}/resources.res");
        std::fs::write(&out_file, &self.0)?;
        println!("cargo:rustc-link-arg-bins={}", &out_file);
        Ok(())
    }

}