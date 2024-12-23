//!  A simple utility for embedding resources in Windows executables.
//!
//!  # Example
//!
//!  ```rust
//! #[cfg(windows)]
//! fn main() {
//!     embedinator::ResourceBuilder::from_env()
//!         .add_manifest(std::fs::read_to_string("assets/app.manifest").unwrap())
//!         .add_icon(32512, Icon::from_png_bytes(std::fs::read("app.png").unwrap()))
//!         .finish();
//!     println!("cargo:rerun-if-changed=app.manifest");
//!     println!("cargo:rerun-if-changed=app.png");
//! }
//!  ```
//!
//!  # Limitations
//!  Currently always sets the language to 0x0409 (English, US) as I don't fully understand how multilingual resource files are supposed to look like.

use std::collections::{BTreeMap, BTreeSet};
use std::env::var;
use std::path::Path;

use crate::coff::CoffWriter;
#[doc(hidden)]
pub use crate::coff::TargetType;
use crate::res::ResWriter;

mod binary;
mod coff;
mod res;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
#[repr(u16)]
pub(crate) enum ResourceType {
    None = 0x0,
    Version = 0x10,
    Icon = 0x3,
    IconGroup = 0xE,
    Manifest = 0x18
}

impl From<ResourceType> for u32 {
    fn from(value: ResourceType) -> Self {
        value as u32
    }
}

impl ResourceType {
    fn flags(self) -> u16 {
        const MOVEABLE: u16 = 0x0010;
        const PURE: u16 = 0x0020;
        #[allow(dead_code)]
        const PRELOAD: u16 = 0x0040;
        const DISCARDABLE: u16 = 0x1000;

        match self {
            ResourceType::None => 0x0,
            ResourceType::Version => MOVEABLE | PURE,
            ResourceType::Icon => DISCARDABLE | MOVEABLE,
            ResourceType::IconGroup => DISCARDABLE | MOVEABLE | PURE,
            ResourceType::Manifest => MOVEABLE | PURE
        }
    }
}

/// The type of the file.
/// The specification defines even more formats, that could be added in the future if needed.
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum FileType {
    #[default]
    Exe = 1,
    Dll = 2
}

/// A version number.
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
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
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
    SpecialBuild = 0x20 //InfoInferred,
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
struct VersionInfo {
    pub file_version: Version,
    pub product_version: Version,
    pub file_type: FileType,
    pub flags: BTreeSet<FileFlag>,
    pub strings: BTreeMap<String, String>
}

/// An Icon resource.
#[derive(Clone, Eq, PartialEq)]
pub struct Icon(Vec<u8>);

impl Icon {
    /// Create an icon from a PNG file. The PNG must contain 32bpp RGBA data.
    /// Other icon format are not currently not supported, but could be added in the future
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

/// A builder for compiling a new resource file in a cargo build script and setting the correct linker flags.
#[derive(Default, Clone)]
pub struct ResourceBuilder {
    version: VersionInfo,
    icon_groups: Vec<(u16, [IconGroupEntry; 1])>,
    icons: Vec<(u16, Icon)>,
    manifest: Option<String>
}

impl ResourceBuilder {
    /// Automatically fills many fields with values from environment variables set by cargo.
    pub fn from_env() -> Self {
        println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION_MAJOR");
        println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION_MINOR");
        println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION_PATCH");
        println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");
        println!("cargo:rerun-if-env-changed=CARGO_PKG_NAME");
        println!("cargo:rerun-if-env-changed=CARGO_PKG_DESCRIPTION");

        let version = Version {
            major: var("CARGO_PKG_VERSION_MAJOR")
                .expect("No CARGO_PKG_VERSION_MAJOR env var")
                .parse()
                .unwrap_or(0),
            minor: var("CARGO_PKG_VERSION_MINOR")
                .expect("No CARGO_PKG_VERSION_MINOR env var")
                .parse()
                .unwrap_or(0),
            patch: var("CARGO_PKG_VERSION_PATCH")
                .expect("No CARGO_PKG_VERSION_PATCH env var")
                .parse()
                .unwrap_or(0),
            build: 0
        };
        Self::default()
            .set_file_version(version)
            .set_product_version(version)
            .add_string("FileVersion", var("CARGO_PKG_VERSION").expect("No CARGO_PKG_VERSION env var"))
            .add_string("ProductVersion", var("CARGO_PKG_VERSION").expect("No CARGO_PKG_VERSION env var"))
            .add_string("ProductName", var("CARGO_PKG_NAME").expect("No CARGO_PKG_NAME env var"))
            .add_string(
                "FileDescription",
                var("CARGO_PKG_DESCRIPTION")
                    .ok()
                    .filter(|d| !d.is_empty())
                    .or_else(|| var("CARGO_PKG_NAME").ok())
                    .expect("No CARGO_PKG_DESCRIPTION or CARGO_PKG_NAME env var")
            )
    }

    pub fn set_file_version(mut self, version: Version) -> Self {
        self.version.file_version = version;
        self
    }

    pub fn set_product_version(mut self, version: Version) -> Self {
        self.version.product_version = version;
        self
    }

    pub fn set_file_type(mut self, file_type: FileType) -> Self {
        self.version.file_type = file_type;
        self
    }

    pub fn add_file_flags(mut self, flags: impl IntoIterator<Item = FileFlag>) -> Self {
        for flag in flags {
            self.version.flags.insert(flag);
        }
        self
    }

    pub fn add_string<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.version.strings.insert(key.into(), value.into());
        self
    }

    pub fn add_manifest<S: Into<String>>(mut self, manifest: S) -> Self {
        assert!(self.manifest.is_none(), "Manifest already set");
        self.manifest = Some(manifest.into());
        self
    }

    pub fn add_icon(mut self, id: u16, icon: Icon) -> Self {
        assert!(!self.icon_groups.iter().any(|(i, _)| *i == id), "Duplicate icon id");
        const ICON_BASE_ID: u16 = 128;
        let icon_id = ICON_BASE_ID + self.icons.len() as u16;
        self.icon_groups.push((
            id,
            [IconGroupEntry {
                icon_id,
                icon_size: icon.0.len()
            }]
        ));
        self.icons.push((icon_id, icon));
        self
    }

    #[doc(hidden)]
    pub fn compile_to_res(&self) -> ResourceFile {
        let mut res = ResWriter::default();

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
        ResourceFile {
            data: res.finish(),
            kind: ResourceFileKind::Res
        }
    }

    #[doc(hidden)]
    pub fn compile_to_coff(&self, target: TargetType) -> ResourceFile {
        let mut writer = CoffWriter::new(target);

        writer.add_resource(ResourceType::Version, 1, &self.version);
        for (id, icon) in &self.icons {
            writer.add_resource(ResourceType::Icon, *id as u32, icon);
        }
        for (id, entries) in &self.icon_groups {
            writer.add_resource(ResourceType::IconGroup, *id as u32, entries.as_slice());
        }
        if let Some(manifest) = &self.manifest {
            writer.add_resource(ResourceType::Manifest, 1, manifest.as_bytes());
        }

        ResourceFile {
            data: writer.finish(),
            kind: ResourceFileKind::Coff
        }
    }

    pub fn finish(self) {
        let target = var("CARGO_CFG_TARGET_ARCH").expect("No CARGO_CFG_TARGET_ARCH env var");
        let target = match target.as_str() {
            "x86_64" => TargetType::X86_64,
            "x86" => TargetType::I386,
            "aarch64" => TargetType::Aarch64,
            _ => panic!("Unsupported target arch")
        };

        let out_dir = var("OUT_DIR").expect("No OUT_DIR env var");
        let out_file = format!("{out_dir}/resources.lib");

        // COFF doesn't seem to work, idk why
        //self.compile_to_res()
        self.compile_to_coff(target)
            .write_to_file(&out_file)
            .expect("Failed to write resource file");

        println!("cargo:rustc-link-arg-bins={}", &out_file);
    }
}

#[doc(hidden)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ResourceFileKind {
    Coff,
    Res
}

#[doc(hidden)]
#[must_use]
#[derive(Clone, Eq, PartialEq)]
pub struct ResourceFile {
    pub data: Vec<u8>,
    pub kind: ResourceFileKind
}

impl ResourceFile {
    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        std::fs::write(path, &self.data)
    }
}
