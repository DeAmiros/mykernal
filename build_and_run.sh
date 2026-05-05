#!/bin/bash

# Clear old files
rm -f kernel.elf main.o boot.o

# 1. Assemble the bootloader
echo "--- Assembling boot.S ---"
arm-none-eabi-gcc -c boot.S -o boot.o

# 2. Compile the Rust code
echo "--- Compiling main.rs ---"
rustc --target armv7a-none-eabi -C panic=abort -C opt-level=3 --emit=obj main.rs -o main.o

if [ $? -ne 0 ]; then
    echo "ERROR: Compilation failed."
    exit 1
fi

# 3. Link them together
echo "--- Linking ---"
arm-none-eabi-ld -T linker.ld boot.o main.o -o kernel.elf

if [ $? -ne 0 ]; then
    echo "ERROR: Linking failed."
    exit 1
fi

# 4. Run in QEMU
echo "--- Starting QEMU ---"
echo "Press Ctrl+A then X to exit QEMU"
qemu-system-arm -M virt -cpu cortex-a15 -kernel kernel.elf -nographic
