use std::collections::{HashMap, HashSet};
use crate::coff::{CoffWriter, TargetType};

mod res;
mod coff;
mod binary;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u16)]
pub(crate) enum ResourceType {
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

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum FileType {
    #[default]
    Exe = 1,
    Dll = 2
}

#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
    pub build: u16
}

impl Version {
    pub fn new(major: u16, minor: u16, patch: u16, build: u16) -> Self {
        Self { major, minor, patch, build }
    }
}

/// Flags that indicate the file's status.
/// See https://learn.microsoft.com/en-us/windows/win32/api/verrsrc/ns-verrsrc-vs_fixedfileinfo
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[repr(u8)]
pub enum FileFlag {
    /// The file contains debugging information or is compiled with debugging features enabled.
    Debug = 0x01,
    /// The file has been modified and is not identical to the original shipping file of the same version number.
    Patched = 0x04,
    /// The file is a development version, not a commercially released product.
    Prerelease = 0x02,
    /// The file was not built using standard release procedures.
    /// If this flag is set, the `VersionInfo` structure should contain a *PrivateBuild* entry.
    PrivateBuild = 0x08,
    /// The file was built by the original company using standard release procedures but is a variation of the normal file of the same version number.
    /// If this flag is set, the `VersionInfo` structure should contain a *SpecialBuild* entry.
    SpecialBuild = 0x20

    //InfoInferred,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VersionInfo {
    pub file_version: Version,
    pub product_version: Version,
    pub file_type: FileType,
    pub flags: HashSet<FileFlag>,
    pub strings: HashMap<String, String>,
}

impl Default for VersionInfo {
    fn default() -> Self {
        Self {
            file_version: Version::new(0, 1, 0, 0),
            product_version: Version::new(0, 1, 0, 0),
            file_type: FileType::Exe,
            flags: HashSet::new(),
            strings: HashMap::from([
                (String::from("ProductVersion"), String::from("0.1.0")),
                (String::from("FileVersion"), String::from("0.1.0")),
                (String::from("ProductName"), String::from("rusty-twinkle-tray")),
                (String::from("FileDescription"), String::from("rusty-twinkle-tray"))
            ]),
        }
    }
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
    version: VersionInfo,
    icon_groups: Vec<(u16, [IconGroupEntry; 1])>,
    icons: Vec<(u16, Icon)>,
    manifest: Option<String>
}

impl ResourceBuilder {

    pub fn add_manifest<S: Into<String>>(mut self, manifest: S) -> Self {
        assert!(self.manifest.is_none(), "Manifest already set");
        self.manifest = Some(manifest.into());
        self
    }

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

        res.write_resource(ResourceType::None, 0, &()); // Files seem to start with an empty resource
        res.write_resource(ResourceType::Version, 1, &self.version);
        for (id, icon) in &self.icons {
            res.write_resource(ResourceType::Icon, *id, icon);
        }
        for (id, entries) in &self.icon_groups {
            res.write_resource(ResourceType::IconGroup, *id, entries.as_slice());
        }
        if let Some(manifest) = &self.manifest {
            res.write_resource(ResourceType::Manifest, 1, manifest.as_bytes());
        }
        res
    }

    pub fn compile_to_coff(self) -> ResourceFile {
        //https://gitlab.com/careyevans/embed-manifest/-/blob/main/src/embed/mod.rs?ref_type=heads

        let mut coff = CoffWriter::new(TargetType::X86_64);


        const LANG_US: u32 = 0x0409;

        let number_of_resource_types =// 1 +
            u16::from(self.manifest.is_some()) +
            u16::from(self.icons.len() > 0) +
            u16::from(self.icon_groups.len() > 0);

        let mut data_entries = Vec::new();
        let mut res_dir = coff.write_directory(number_of_resource_types);
        //{
        //    let entry = res_dir
        //        .subdirectory(&mut coff, ResourceType::Version as u32, 1)
        //        .subdirectory(&mut coff, 1, 1)
        //        .data_entry(&mut coff, LANG_US);
        //    data_entries.push(entry);
        //}
        if self.manifest.is_some() {
            let entry = res_dir
                .subdirectory(&mut coff, ResourceType::Manifest as u32, 1)
                .subdirectory(&mut coff, 1, 1)
                .data_entry(&mut coff, LANG_US);
            data_entries.push(entry);
        }
        if self.icons.len() > 0 {
            let mut icon_dir = res_dir
                .subdirectory(&mut coff, ResourceType::Icon as u32, self.icons.len() as u16);
            for (id, _) in &self.icons {
                let entry = icon_dir
                    .subdirectory(&mut coff, *id as u32, 1)
                    .data_entry(&mut coff, LANG_US);
                data_entries.push(entry);
            }
        }
        if self.icon_groups.len() > 0 {
            let mut icon_dir = res_dir
                .subdirectory(&mut coff, ResourceType::IconGroup as u32, self.icon_groups.len() as u16);
            for (id, _) in &self.icon_groups {
                let entry = icon_dir
                    .subdirectory(&mut coff, *id as u32, 1)
                    .data_entry(&mut coff, LANG_US);
                data_entries.push(entry);
            }
        }

        {
            let mut next_entry = data_entries.iter_mut();
            let mut next_entry = move || next_entry.next().expect("not enough data entries");

            //next_entry().write_data(&mut coff, (&[234u8]).as_slice());
            if let Some(manifest) = &self.manifest {
                next_entry().write_data(&mut coff, manifest.as_bytes());
            }
            for (_, icon) in &self.icons {
                next_entry().write_data(&mut coff, icon);
            }
            for (_, group) in &self.icon_groups {
                next_entry().write_data(&mut coff, group.as_slice());
            }
        }

        {
            coff.start_relocations();
            for e in data_entries {
                e.write_relocation(&mut coff);
            }
        }

        ResourceFile(coff.finish())

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