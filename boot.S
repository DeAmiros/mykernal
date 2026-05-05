.global start
start:
    ldr sp, =0x40200000
    mov r0, r2
    bl kernel_main
hang:
    b hang
