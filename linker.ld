ENTRY(_start)
SECTIONS
{
    . = 0x10000;
    .text:{
        *(.text)
    }
    .ALIGN(4)
    .data : {
        *(.data)
    }
    .ALIGN(4)
    .bss : {
        *(.bss)
    }
}