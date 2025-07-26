# RustyBoot

**RustyBoot** is a simple experimental bootloader written in Rust.  
It's an early-stage project that aims to eventually boot into `ext` partitions.

---

## 🚧 Status

> ⚠️ This bootloader is still under development and may not work as expected.  
> Use it for learning or experimentation only!

---

## 🛠️ Build & Run

This project uses a `Makefile` for easy building and testing.

### Requirements

- Rust (nightly)
- `cargo`
- `llvm-objcopy`
- `qemu`
- `make`

### Commands

```bash
make bootloader   # Builds the bootloader binary
make run          # Runs it using QEMU
```
### BUILT WITH ❤️ IN RUST
