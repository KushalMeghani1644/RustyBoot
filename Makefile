RUST_TARGET = i686-unknown-linux-gnu
BUILD_DIR = target/$(RUST_TARGET)/release
BINARY_NAME = RustyBoot

.PHONY: all clean bootloader disk run debug install-deps

all: bootloader

install-deps:
	rustup target add $(RUST_TARGET)
	sudo zypper refresh
	sudo zypper install -y qemu-x86 gcc binutils cross-i686-linux-gnu-binutils cross-i686-linux-gnu-gcc

bootloader:
	cargo build --release --target $(RUST_TARGET)
	# Find the actual binary name and copy it
	@if [ -f "$(BUILD_DIR)/RustyBoot" ]; then \
		objcopy -O binary $(BUILD_DIR)/RustyBoot $(BUILD_DIR)/bootloader.bin; \
	elif [ -f "$(BUILD_DIR)/rustyboot" ]; then \
		objcopy -O binary $(BUILD_DIR)/RustyBoot $(BUILD_DIR)/bootloader.bin; \
	else \
		echo "Looking for binary in deps folder..."; \
		BINARY_FILE=$(find $(BUILD_DIR)/deps/ -name "$(BINARY_NAME)-*" -o -name "RustyBoot-*" | head -1); \
		if [ -n "$BINARY_FILE" ]; then \
			objcopy -O binary "$BINARY_FILE" $(BUILD_DIR)/bootloader.bin; \
		else \
			echo "Error: Could not find compiled binary"; \
			ls -la $(BUILD_DIR)/; \
			ls -la $(BUILD_DIR)/deps/; \
			exit 1; \
		fi \
	fi
	# Make sure the bootloader is exactly 512 bytes for MBR
	truncate -s 512 $(BUILD_DIR)/bootloader.bin

disk: bootloader
	dd if=/dev/zero of=disk.img bs=1M count=10
	dd if=$(BUILD_DIR)/bootloader.bin of=disk.img conv=notrunc

run: disk
	qemu-system-i386 -drive format=raw,file=disk.img -monitor stdio

debug: disk
	qemu-system-i386 -drive format=raw,file=disk.img -s -S &
	echo "Connect with: gdb -ex 'target remote localhost:1234'"

clean:
	cargo clean
	rm -f disk.img

# Create a test kernel for testing
test-kernel:
	echo -e '\x7fELF\x01\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00' > test_kernel
	echo "Test kernel created"
