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

  ldr     x30, =LD_STACK_PTR0
	mov     sp, x30
  bl      kernel_main

.equ PSCI_SYSTEM_OFF, 0x84000008
.globl system_off
system_off:
    ldr     x0, =PSCI_SYSTEM_OFF
    hvc     #0

start_core_1:
	/* Enable NEON/SIMD instructions */
	mov x30, #(0x3 << 20)
	msr cpacr_el1, x30
	isb
	/* -- */

  ldr     x30, [x0, #8]
	add     x30, x30, #8192
  mov     sp, x30
	ldr     x3, [x0, #0]
  br      x3
spin:
  bl spin

.equ PSCI_0_2_FN64_CPU_ON, 0xc4000003
.globl cpu_on
cpu_on:
    mov x3, x1         /* context ID: put into target CPU's x0 */
    mov x1, x0 
    ldr x0, =PSCI_0_2_FN64_CPU_ON
    ldr x2, =start_core_1   /* entry point */
    hvc #0
		ret

.equ PSCI_CPU_OFF, 0x84000002
.globl cpu_off
cpu_off:
    mov x1, x0 
    ldr     x0, =PSCI_CPU_OFF
    hvc     #0
		ret

spin_lock:
	mov	w2, #1
	sevl
l1:	wfe
l2:	ldaxr	w1, [x0]
	cbnz	w1, l1
	stxr	w1, w2, [x0]
	cbnz	w1, l2
	ret

