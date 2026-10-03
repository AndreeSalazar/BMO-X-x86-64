//! **`bmo-rt` -- LA LIBC DE BMO.** Lo que un `.bex` enlaza.
//!
//! Exporta con nombre C, SOLO en el `.bex`: `crt0` (`_start` -> `main` ->
//! `exit`), el monton (`malloc`/`free`/`calloc`/`realloc`), los ficheros
//! (`fopen`/`fread`/`fseek`/`fgets`/`fwrite`/`fclose`... y `stdout`), las
//! cadenas (`memcpy`, `strlen`, `strcasecmp`, `strtol`...) y el formato
//! (`bmo_printf`, `bmo_fprintf`, `bmo_snprintf`). La lista entera, con su
//! firma, esta en `BMO.toml`.
//!
//! =======================================================================
//! * POR QUE EXISTE, si el compilador ya emite `printf` en linea
//! =======================================================================
//!
//! Esta es LA pregunta, y sin contestarla este crate parece duplicado. En BMO
//! hay **tres sitios** que saben hacer `memcpy`, y cada uno existe por un
//! motivo distinto:
//!
//! ```text
//!   1. bmo_lower::{memoria, console, fmt}     EN LINEA, dentro del .bex
//!      -> el codegen escupe los bytes del bucle en el sitio de la llamada
//!      -> USADO HOY y verificado en el Ryzen
//!      -> cero enlazador, cero relocaciones, cero formato de libreria
//!      -> y CADA llamada paga su copia del codigo
//!
//!   2. toolchain/lang/base/{lib,bmo}/*.c      MODULOS EN C
//!      -> fuente C con su BMO.toml, que el sistema de modulos resuelve
//!      -> para lo que se escribe MEJOR en C que emitiendo bytes a mano
//!
//!   3. ESTE CRATE                              SIMBOLOS ENLAZABLES
//!      -> una implementacion, una vez, a la que se llama con `call`
//! ```
//!
//! **No compiten: escalan distinto.** Emitir en linea es perfecto para las
//! seis funciones que un programa chico usa --y por eso es lo que corre hoy--
//! y deja de serlo en cuanto un programa usa doscientas. DOOM no es un
//! programa que llame a `memcpy`: es un programa que llama a media libc, y
//! meterle una copia de cada funcion en cada sitio de llamada infla la imagen
//! sin darle nada a cambio.
//!
//! La regla que decide, y que hay que aplicar funcion por funcion:
//!
//! > **En linea lo que no tiene semantica de lenguaje y se usa poco. Enlazado
//! > lo que tiene estado, medida, o se llama desde muchos sitios.**
//!
//! `malloc` es el ejemplo claro: tiene **estado** (la lista de libres). Emitirlo
//! en linea significaria un monton por sitio de llamada, que no es un monton.
//!
//! =======================================================================
//! * QUE NO ES -- para no chocar con lo que ya existe
//! =======================================================================
//!
//! - **No es `bmo-userland`.** Aquel (`Ultra_userspace/userland`) es la API en
//!   **Rust** que usa el compositor: `Pantalla`, `Archivo`, `Directorio`,
//!   `Memoria`. Este exporta **simbolos C** para que los enlace un `.bex`
//!   compilado. Los dos envuelven los mismos 2 syscalls y **ninguno sustituye
//!   al otro**: distinto consumidor, distinto idioma, distinta forma.
//! - **No es `bmo-abi`.** Aquel es el CONTRATO --numeros de operacion,
//!   estructuras, disposicion--; este es una IMPLEMENTACION que lo usa. Por eso
//!   este crate ya no vive en `platform/abi/`: tenerlo ahi hacia que esa
//!   carpeta significara dos cosas.
//! - **No es un driver.** Tenia dentro un `input/ps2.rs` de 176 lineas -- un
//!   driver de teclado PS/2 en una biblioteca estandar. Borrado el 2026-08-02:
//!   la entrada de este sistema es **USB HID por xHCI**, y a Ring 3 le llega
//!   como `KIND_INPUT`. Ni el bus era ese ni el camino.
//!
//! =======================================================================
//! * BMO-X Y NADA MAS (03-10)
//! =======================================================================
//!
//! Hasta el 03-10 esto era una libc de GNU con otro nombre, y se notaba en lo
//! que fallaba:
//!
//! ```text
//!    antes                                 ahora
//!    crt0 borraba __bss_start..__bss_end   la region CEROS de BEF2 llega a
//!      -- simbolos de un ELF de GNU que      cero del kernel; nada que borrar,
//!      el link.ld de BMO no define: NO       y nada que no enlace
//!      ENLAZABA
//!    main(0, NULL)                         argc/argv de TASK_OP_ARGUMENTOS
//!    malloc exportado tambien en las       los nombres de C solo en el .bex;
//!      pruebas: pisaba el del anfitrion      el banco pasa (y antes moria:
//!      y el banco moria                      "memory allocation failed")
//!    arenas de 1 MiB, una peticion cada    arenas que doblan (1 -> 64 MiB):
//!      una -- con OCHO por proceso, 8 MiB    ocho peticiones dan ~200 MiB
//!    lo grande, pedido y nunca devuelto    vuelve con MEM_OP_SOLTAR
//!    alineado a 8, sin mirar el Layout     a 16 (C) y a lo que pida Rust
//!    printf a un bufer de 1 KiB, %d %s     ancho, precision (%.8s), %f, %x
//!      y poco mas; "(nil)" de glibc          y sin tope de largo
//!    sin ficheros                          fopen/fread/fseek/fgets/fwrite
//!                                            sobre KIND_ARCHIVO, leyendo
//!                                            DIRECTO al monton (LEER_EN)
//!    BMO.toml prometia printf, sprintf     dice lo que hay
//!      y snprintf, que no existian
//! ```
//!
//! Lo que la forma de BMO-X cambia de una libc, y aqui se dice una vez:
//!
//! - **Un `FILE` es un handle concedido**, no un descriptor: `fopen` empuja la
//!   ruta por el renglon (`TASK_OP_RUTA`) y el kernel da un `KIND_ARCHIVO`.
//!   Lo escrito llega al disco AL CERRAR, y por eso `exit` cierra todo.
//! - **`printf` no es variadico del ABI de C**: los argumentos llegan en un
//!   arreglo de palabras con su cuenta (`bmo_printf(fmt, n, args)`), que es
//!   lo que emite el frontend. Ver `fmt`.
//! - **No hay entorno, ni `stdin` de texto, ni `locale`**: la entrada es
//!   `KIND_INPUT`, y las clases de caracteres son ASCII.
//!
//! =======================================================================
//! * ESTADO HONESTO: probado, y todavia sin un programa que lo enlace
//! =======================================================================
//!
//! Las pruebas del anfitrion cubren el monton, el formato, las cadenas, los
//! argumentos y los ficheros (sobre un disco de mentira con el mismo
//! contrato). Lo que NO se ha visto todavia es un `.bex` de C que enlace estos
//! simbolos en el metal: eso sigue siendo el punto del enlace, que es de la
//! forja (`toolchain/forge/README.md`, caminos A y B), no de esta libc.
//! Mientras no exista, esto esta LISTO, no USADO -- y se dice aqui para que
//! nadie lo cuente como hecho.

#![no_std]
#![no_builtins]
#![allow(static_mut_refs)]

#[cfg(test)]
extern crate std;

pub mod argumentos;
pub mod fichero;
pub mod fmt;
pub mod heap;
pub mod string;
pub mod syscall;

/// `_start`, `exit` y `abort`: FUERA de las pruebas, porque en el anfitrion
/// el arnes de `cargo test` ya trae su arranque y su `exit`; y solo con la
/// funcion `libc` (un programa de Rust trae su `_start`).
#[cfg(all(not(test), feature = "libc"))]
pub mod crt0;

pub mod ffi;
