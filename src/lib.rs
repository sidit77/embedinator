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


// https://learn.microsoft.com/en-us/windows/win32/api/verrsrc/ns-verrsrc-vs_fixedfileinfo
pub struct FixedVersionInfo {
    file_version: [u16; 4],
    product_version: [u16; 4],
    file_flags: FileFlags
}

#[derive(Default, Debug, Clone)]
pub struct ResourceBuilder;

impl ResourceBuilder {

    pub fn compile(self) -> ResourceFile {
        let mut res = ResourceFile(Vec::new());

        res.write_empty(); // Files seem to start with an empty resource
        res.write_version(FixedVersionInfo {
            file_version: [0, 1, 0, 0],
            product_version: [0, 1, 0, 0],
            file_flags: FileFlags::NONE,
        });
        res
    }

}

#[must_use]
#[derive(Clone, Eq, PartialEq)]
pub struct ResourceFile(Vec<u8>);

impl ResourceFile {

    pub fn write_to_file(self) -> std::io::Result<()> {
        std::fs::write("test.res", self.0)
    }

}