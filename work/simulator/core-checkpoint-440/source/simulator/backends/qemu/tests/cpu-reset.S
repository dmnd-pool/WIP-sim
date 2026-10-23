/* Project-owned Cortex-A9 device regression. This is not miner firmware. */
.syntax unified
.arm
.text
.macro call target
    adr lr, 99f
    b \target
99:
.endm
.global _start
_start:
    mrc p15, 0, r0, c0, c0, 5
    and r0, r0, #3
    cmp r0, #0
    bne secondary
    ldr r1, =0xe0001000
    mov r3, #124
    str r3, [r1, #0x18]
    mov r3, #6
    str r3, [r1, #0x34]
    mov r3, #0x20
    str r3, [r1, #4]
    mov r3, #0x14
    str r3, [r1]
    ldr r2, =0xf8000000
    ldr r4, =0x10000
    ldr r3, =0xdf0d
    str r3, [r2, #8]
    ldr r3, [r4, #32]
    cmp r3, #1
    beq after_reset
    mov r3, #0
    str r3, [r4]
    str r3, [r4, #4]
    mov r0, #'A'
    str r0, [r1, #0x30]
    call release_secondary
    mov r9, #1
    call wait_boot
    mov r0, #'B'
    str r0, [r1, #0x30]
    mov r3, #0x20
    str r3, [r2, #0x244]
    call delay
    ldr r6, [r4, #4]
    call delay
    ldr r7, [r4, #4]
    cmp r6, r7
    bne fail
    ldr r7, [r4]
    cmp r7, #1
    bne fail
    mov r0, #'C'
    str r0, [r1, #0x30]
    mov r3, #0
    str r3, [r2, #0x244]
    call delay
    ldr r7, [r4, #4]
    cmp r7, r6
    bls fail
    ldr r7, [r4]
    cmp r7, #1
    bne fail
    mov r0, #'D'
    str r0, [r1, #0x30]
    call release_secondary
    mov r9, #2
    call wait_boot
    mov r0, #'E'
    str r0, [r1, #0x30]
    /* Reset the machine while the secondary clock gate is asserted. */
    mov r3, #0x20
    str r3, [r2, #0x244]
    call delay
    mov r3, #1
    str r3, [r4, #32]
    str r3, [r2, #0x200]
    b done
after_reset:
    mov r0, #'R'
    str r0, [r1, #0x30]
    ldr r6, [r4, #4]
    call delay
    ldr r7, [r4, #4]
    cmp r6, r7
    bne fail
    call release_secondary
    mov r9, #3
    call wait_boot
    mov r3, #0x20
    str r3, [r2, #0x244]
    call delay
    mov r0, #'P'
    str r0, [r1, #0x30]
    mov r0, #'\n'
    str r0, [r1, #0x30]
done:
    wfi
    b done
release_secondary:
    mov r3, #0x22
    str r3, [r2, #0x244]
    mov r3, #0x20
    str r3, [r2, #0x244]
    mov r3, #0
    str r3, [r2, #0x244]
    dsb sy
    bx lr
wait_boot:
    ldr r10, =0x1000000
1:  ldr r7, [r4]
    cmp r7, r9
    bxeq lr
    subs r10, r10, #1
    bne 1b
    b fail
delay:
    ldr r10, =0x100000
1:  subs r10, r10, #1
    bne 1b
    bx lr
secondary:
    ldr r4, =0x10000
    ldr r7, [r4]
    add r7, r7, #1
    str r7, [r4]
    ldr r8, =0x12345670
1:  add r8, r8, #1
    str r8, [r4, #4]
    b 1b
fail:
    mov r0, #'F'
    str r0, [r1, #0x30]
    b done
.ltorg
