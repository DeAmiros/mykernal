_start:
    ldr sp, =0x20000
    bl kernel_main
hang:
    b hang
