OUTPUT_FORMAT("elf64-x86-64")
ENTRY(_start)

SECTIONS {
    . = 0x7C00;

    .text : {
        *(.text*)
    }

    .rodata : {
        *(.rodata*)
    }

    .data : {
        *(.data*)
    }

    .bss : {
        *(.bss*)
        *(COMMON)
    }
    /DISCARD/ : { *(.eh_frame) }
}
