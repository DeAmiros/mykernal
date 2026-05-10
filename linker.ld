ENTRY(start)
SECTIONS
{
    . = 0x40100000;
    .text : {
        *(.text)
    }
    . = ALIGN(4);
    .data : {
        *(.data)
    }
    . = ALIGN(4);
    .bss : {
        __bss_start = .;
        *(.bss)
        __bss_end = .;
    }
}