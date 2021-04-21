.globl _start
.extern LD_STACK_PTR

.section ".text.boot"
_start:
	/* Get device tree. It's either passed in x0, or at the beginning of memory
	 */
	adr x1, .
	mrs x2, TTBR0_EL1
	cmp x0,  #0
	mov x0, #0x40000000

	/* Enable NEON/SIMD instructions */
	mov x30, #(0x3 << 20)
	msr cpacr_el1, x30
	isb
	/* -- */

  ldr     x30, =LD_STACK_PTR
  mov     sp, x30
  bl      kernel_main

.equ PSCI_SYSTEM_OFF, 0x84000008
.globl system_off
system_off:
    ldr     x0, =PSCI_SYSTEM_OFF
    hvc     #0
