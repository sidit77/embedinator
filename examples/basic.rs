use windows_resource_compiler::ResourceBuilder;

fn main() {
    ResourceBuilder::default()
        .compile()
        .write_to_file()
        .unwrap()
}