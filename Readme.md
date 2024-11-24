# embedinator

A simple utility to embed resources such as icons or manifests into a Windows executable from a cargo build script. 

The advantage of crate over others such as `windres` is that this crate directly outputs a linkable library file instead of relying on, possibly missing, platform tools such as `rc.exe` and `cvtres.exe`.

Additionally, this crate has no other dependencies.

## Example
```rust
#[cfg(windows)]
fn main() {
    embedinator::ResourceBuilder::from_env()
        .add_manifest(std::fs::read_to_string("assets/app.manifest").unwrap())
        .add_icon(32512, Icon::from_png_bytes(std::fs::read("app.png").unwrap()))
        .finish();
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rerun-if-changed=app.png");
}
```
