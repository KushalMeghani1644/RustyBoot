# RustyBoot

**RustyBoot** is a low-level bootloader written in **Rust**, designed to boot from **EXT-based partitions** (ext2/ext3/ext4).  
It currently supports **detecting EXT partitions** by reading and parsing the on-disk **superblock**, with plans to load kernel files in the future.

This project is a part of the [Rusty-Suite](https://github.com/KushalMeghani1644) and is built for learning, experimentation, and low-level OS development.

---

## 🚧 Status

🚀 **EXT superblock reading implemented!**

✅ Reads disk sectors directly  
✅ Detects EXT2/3/4 partitions by parsing the superblock  
✅ Supports loading kernel
✅ Basic memory management.

⚠️ This bootloader is still **under active development**.  
Use it only for educational or experimental purposes.

## Trademark Notice
The names “Rusty-Suite”, “RustyTodos”, “RustyBoot”, and “Rusty-Checker” are part of this project’s identity.  
See [TRADEMARK.md](TRADEMARK.md) for details.

---

## 🛠️ Build & Run

This project uses a **Makefile** to simplify building and running the bootloader.

### 🔧 Requirements

- `rustup` with **Rust nightly**  
- `cargo-xbuild` (for building core/kernel without std)  
- `llvm-objcopy`  
- `qemu`  
- `make`

You also need to install `bootimage`:

```bash
rustup component add llvm-tools-preview
cargo +nightly build -Z build-std=core --target i686-bootloader.json
truncate -s 510 RustyBoot.bin
echo -ne '\x55\xAA' >> RustyBoot.bin
qemu-system-i386 -drive format=raw,file=RustyBoot.bin -nographic
```

# BUILT WITH ❤️ IN RUST
