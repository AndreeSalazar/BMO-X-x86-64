/* tanda22d.c -- la DLL de la TANDA 22 de Cyberpunk (30-09): una DLL con su
 * PROPIO TLS (__declspec(thread)) y un callback de TLS, como libxess_fg.dll
 * del juego: su DllMain leyo 0+0x8 en el metal porque la casa solo le daba
 * TLS al .exe. Sin CRT: el directorio de TLS (_tls_used) va escrito aqui,
 * como lo pone tlssup.obj del CRT de MSVC. */
typedef unsigned long long U64;
typedef unsigned long DWORD;
typedef void *HANDLE;
#define W __stdcall
typedef void(W *CB)(void *, DWORD, void *);

#pragma section(".tls", read, write)
#pragma section(".tls$ZZZ", read, write)
#pragma section(".CRT$XLA", read)
#pragma section(".CRT$XLB", read)
#pragma section(".CRT$XLZ", read)
__declspec(allocate(".tls")) char _tls_start = 0;
__declspec(allocate(".tls$ZZZ")) char _tls_end = 0;
DWORD _tls_index = 0;

static int cb_proceso, cb_hilo;
static void W al_tls(void *h, DWORD motivo, void *r) {
    (void)h;
    (void)r;
    if (motivo == 1)
        cb_proceso++;
    if (motivo == 2)
        cb_hilo++;
}
__declspec(allocate(".CRT$XLA")) CB __xl_a = 0;
__declspec(allocate(".CRT$XLB")) CB __xl_b = al_tls;
__declspec(allocate(".CRT$XLZ")) CB __xl_z = 0;

typedef struct {
    U64 ini, fin, indice, callbacks;
    DWORD ceros, caracteristicas;
} DirTls;
const DirTls _tls_used = {(U64)&_tls_start, (U64)&_tls_end, (U64)&_tls_index, (U64)(&__xl_a + 1), 0, 0};

static __declspec(thread) int por_hilo = 42;
static int visto_en_main = -1, callbacks_antes_de_main = -1;

int W entrada(HANDLE h, DWORD motivo, void *r) {
    (void)h;
    (void)r;
    if (motivo == 1) {
        visto_en_main = por_hilo;
        callbacks_antes_de_main = cb_proceso;
        por_hilo = 7;
    }
    return 1;
}

int leer(void) { return por_hilo; }
void poner(int v) { por_hilo = v; }
int en_main(void) { return visto_en_main; }
int antes_de_main(void) { return callbacks_antes_de_main; }
int callbacks_hilo(void) { return cb_hilo; }
/* Para el diagnostico: su indice y su bloque del hilo que llama. */
unsigned indice(void) { return (unsigned)_tls_index; }
U64 su_bloque(void) {
    U64 t;
    __asm__("movq %%gs:0x58, %0" : "=r"(t));
    return t ? ((U64 *)t)[_tls_index] : 0;
}
