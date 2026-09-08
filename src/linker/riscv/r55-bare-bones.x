/* Linker script for r55, whose interpreter provides 1GB of RAM starting at
   0x80000000. Every section has to be placed explicitly and end up inside a
   PT_LOAD segment: r55 sizes the emulator's DRAM from the program headers, so
   an orphaned section would simply not exist at runtime. */

MEMORY
{
  CALL_DATA : ORIGIN = 0x80000000, LENGTH = 1M
  STACK     : ORIGIN = 0x80100000, LENGTH = 2M
  REST_OF_RAM : ORIGIN = 0x80300000, LENGTH = 1021M
}

SECTIONS
{
  . = 0x80300000;

  .text : {
    /* The entry stub, kept first so it lands at the start of the image. */
    *(.text.start)
    *(.text .text.*)
  } > REST_OF_RAM

  .rodata : {
    *(.rodata .rodata.*)
    *(.srodata .srodata.*)
  } > REST_OF_RAM

  .data : {
    *(.data .data.*)
    . = ALIGN(8);
    PROVIDE( __global_pointer$ = . + 0x800 );
    *(.sdata .sdata.*)
  } > REST_OF_RAM

  .bss (NOLOAD) : {
    *(.sbss .sbss.*)
    *(.bss .bss.*)
    *(COMMON)
  } > REST_OF_RAM

  /* The stack grows down from the top of the STACK region, stopping just
     below where .text begins. */
  _stack_top = ORIGIN(STACK) + LENGTH(STACK);

  /DISCARD/ : {
    *(.eh_frame*)
    *(.comment)
    *(.riscv.attributes)
  }
}

ENTRY(_start)
