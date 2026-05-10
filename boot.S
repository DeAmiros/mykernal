.global start
start:
    ldr sp, =0x42000000
    
    ldr r0, =__bss_start
    ldr r1, =__bss_end
    mov r3, #0
1:
    cmp r0, r1
    bge 2f
    str r3, [r0], #4
    b 1b
2:
    
    mov r0, r2
    bl kernel_main
hang:
    b hang
