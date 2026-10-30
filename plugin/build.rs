// OMSI looks the plugin functions up by their plain names (GetProcAddress), but a 32-bit stdcall function is exported decorated
// (`_PluginStart@4`). The .def file exports them under the plain names.
fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo:rustc-cdylib-link-arg=/DEF:{dir}\\plugin.def");
    println!("cargo:rerun-if-changed=plugin.def");
}
