/* bmo-kernel linker script — Ring 0 base */
ENTRY(_start)

SECTIONS
{
    . = 0x400000;

    .text ALIGN(4K) : {
        /* ** LOS DOS LIMITES, para que la pantalla azul pueda SITUAR un rip.
         *
         * Sin esto la azul imprime una direccion absoluta y quien la lee tiene
         * que saberse de memoria donde se carga el kernel. Con esto dice el
         * desplazamiento, que es lo que `simbolo.py` come. Y sobre todo dice
         * cuando el rip **no cae en codigo del kernel**, que es la pista que
         * el 11-09 costo media sesion. */
        __text_start = .;
        KEEP(*(.text._start))
        *(.text .text.*)
        __text_end = .;
    }

    .rodata ALIGN(4K) : {
        /* ** LOS DOS LIMITES DE LAS CONSTANTES, y existen por lo mismo que los
         * de `.text`: para poder preguntar "esto es mio?" ANTES de leerlo.
         *
         * El 2026-09-20 la maquina murio en `cabina::ring::record_fmt` leyendo
         * el ultimo byte de `Location::caller().file()` -- o sea que no murio
         * haciendo el trabajo: murio APUNTANDOLO. Ese `&'static str` vive aqui
         * dentro, y sin estos dos numeros no hay forma de que el que graba
         * compruebe el puntero que le dan antes de dereferenciarlo. */
        __rodata_start = .;
        *(.rodata .rodata.*)
        __rodata_end = .;
    }

    .data ALIGN(4K) : {
        *(.data .data.*)
    }

    .bss ALIGN(4K) : {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(16);
        . = . + 65536;  /* KERNEL_STACK (64 KiB kernel stack) */
        KERNEL_STACK_END_MARKER = .;
        __bss_end = .;
    }
}
