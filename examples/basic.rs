use windows_resource_compiler::{Icon, ResourceBuilder};

fn main() {
    ResourceBuilder::default()
        .add_icon(2, Icon::from_png_bytes(std::fs::read("app.png").unwrap()))
        //.add_icon(4, Icon::from_png_bytes(std::fs::read("app.png").unwrap()))
        .compile_to_coff()
        //.compile()
        .write_to_file()
        .unwrap()
}