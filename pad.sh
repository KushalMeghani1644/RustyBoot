#!/bin/bash

# Step 1: Truncate or pad the file to exactly 510 bytes
truncate -s 510 bootloader.bin

# Step 2: Add the magic boot signature 0x55AA
echo -ne '\x55\xAA' >> bootloader.bin

