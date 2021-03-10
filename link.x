ENTRY(_start)
SECTIONS
{
    . = 0x41080000;
    .text.boot : { *(.text.boot) }
    .text : { *(.text) }
    .data : { *(.data) }
    .rodata : { *(.rodata) }
    .bss : { *(.bss) }

    . = ALIGN(8);
    . = . + 0x40000;
    LD_STACK_PTR = .;
}
