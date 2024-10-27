use windows_resource_compiler::{Icon, ResourceBuilder};

fn main() {
    ResourceBuilder::default()
        .add_icon_group(2, Icon::from_png_bytes(std::fs::read("app.png").unwrap()))
        .compile()
        .write_to_file()
        .unwrap()
}