/* autopsia_C.bex -- un programa que FALLA A PROPOSITO (B2 de EL_FOCO).
 *
 * Lee `p[100]` de un puntero malo, 0xDEAD0000: el kernel lo para con un
 * fallo de pagina y la autopsia de Ring 3 lo escribe en `datos/fallos.txt`
 * (y en el capitulo 7 de `save`). Lo que se mira en el Ryzen es el renglon
 * `instr`, justo detras del `rip`: la instruccion que fallo, leida por la
 * lente, y el registro de su direccion con lo que valia --
 *
 *     instr     movsxd rax, dword [rax]  rax=0xdead0190
 *
 * (la forma exacta la dice el emisor; lo que no puede faltar es una lectura
 * cuyo registro y desplazamiento den 0xDEAD0190). Es el mismo programa que
 * `toolchain/lang/c/emisor-x86_64/src/tests/autopsia.rs` corre en el
 * anfitrion contra la trampa del emulador.
 *
 * Si la consola dice "no fallo", algo mapeo esa direccion: es un NO.
 */
int leer(int *p) { return p[100]; }

int main(void) {
    printf("autopsia: leo de 0xDEAD0000 a proposito\n");
    int v = leer((int *)0xDEAD0000);
    printf("no fallo: %d\n", v);
    return 1;
}
