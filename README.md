# 🧪 Rust Bootloader (Bare Metal)

This is a minimal 512-byte bootloader written in **pure Rust**, built from scratch without `std`, that boots via BIOS and prints a message over the serial port.

## 🚀 What it does

- Compiles a Rust program with no OS or standard library.
- Builds a 512-byte bootable binary with a valid `0x55AA` signature.
- Boots using BIOS (SeaBIOS in QEMU).
- Outputs `Hello from bootloader!` over serial (COM1).

## 🛠 Requirements

- Rust Nightly (`rustup install nightly`)
- LLVM tools (`llvm-objcopy`)
- QEMU
- `cargo-binutils` (optional)

## 🧪 How to Build

```bash
cargo +nightly build -Z build-std=core --release
llvm-objcopy -O binary target/x86_64-boot/release/bootloader bootloader.bin
truncate -s 510 bootloader.bin
echo -ne '\x55\xAA' >> bootloader.bin
dd if=/dev/zero of=disk.img bs=512 count=2880
dd if=bootloader.bin of=disk.img conv=notrunc
