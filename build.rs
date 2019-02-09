use std::{env::var, path::Path};
use wayland_scanner::{generate_c_code, generate_c_interfaces, generate_rust_code, Side};

fn main() {
    let protocol = "protocol/wlr-data-control-unstable-v1.xml";

    let out_dir_str = var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir_str);

    let generate_code = if var("CARGO_FEATURE_NATIVE_LIB").ok().is_some() {
        generate_c_code
    } else {
        generate_rust_code
    };

    generate_code(protocol,
                  out_dir.join("wlr_data_control_api.rs"),
                  Side::Client);

    generate_c_interfaces(protocol, out_dir.join("wlr_data_control_interfaces.rs"));
}