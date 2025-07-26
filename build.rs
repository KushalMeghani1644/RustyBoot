// === build.rs ===
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("bootloader.ld");

    let linker_script = r#"
ENTRY(_start)

SECTIONS
{
    . = 0x8000;

    .text : {
        *(.text)
        *(.text.*)
    }

    .rodata : {
        *(.rodata)
        *(.rodata.*)
    }

    .data : {
        *(.data)
        *(.data.*)
    }

    .bss : {
        *(.bss)
        *(.bss.*)
    }
}
"#;

    fs::write(&dest_path, linker_script).unwrap();
    println!("cargo:rustc-link-search=native={}", out_dir);
    println!("cargo:rustc-link-arg=-Tbootloader.ld");
    println!("cargo:rerun-if-changed=build.rs");
}
