use embedinator::{ResourceBuilder, TargetType, Version};

fn main() {
    ResourceBuilder::default()
        .set_file_version(Version::new(1, 0, 0, 0))
        .set_product_version(Version::new(1, 0, 0, 0))
        .add_string("ProductVersion", "1.0.0")
        .add_string("FileVersion", "1.0.0")
        .add_string("ProductName", "Example")
        .add_string("FileDescription", "An example application")
        //.add_manifest(std::fs::read_to_string("app.manifest").unwrap())
        //.add_icon(4, Icon::from_png_bytes(std::fs::read("app.png").unwrap()))
        .compile_to_coff(TargetType::X86_64)
        .write_to_file("test.lib")
        .unwrap()
}