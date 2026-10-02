// espejo.dll -- ESPEJO, paso 0 (la mitad que NO inyecta en nadie).
//
// Se carga EN EL PROPIO proceso (`tanda40.exe` hace LoadLibraryW de esta DLL
// al arrancar). Al cargar, reescribe la IAT del `.exe` principal: cada funcion
// importada pasa por un trampolin suyo que apunta la PRIMERA llamada y salta a
// la de Windows. El registro sale en ESPEJO.TXT, al lado del `.exe`, con EL
// MISMO formato que el DIARIO de PROTON-X (proton-x-casa/src/diario.rs), para
// que `espejo-compara` ponga los dos ficheros lado a lado.
//
// Sin CRT, como las tandas. El trampolin es la misma tecnica que el DIARIO:
// un `mov r11d, <i>; jmp comun` por funcion (11 bytes, generado en RWX) y un
// unico `espejo_comun` en asm que preserva los registros, llama a `primera(i)`
// la primera vez y salta al destino real.
//
// NO engancha COM (eso es el paso 4) ni inyecta en otro proceso (eso es el
// lanzador, un paso aparte): esto solo observa su propia IAT.

typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned int u32;
typedef unsigned long long u64;
typedef long long i64;

#define WINAPI __attribute__((ms_abi))
#define NULLP ((void *)0)

// -- Windows, lo justo (como los .def de las tandas) -------------------------
extern WINAPI void *GetModuleHandleW(const u16 *);
extern WINAPI u32 GetModuleFileNameW(void *, u16 *, u32);
extern WINAPI int VirtualProtect(void *, u64, u32, u32 *);
extern WINAPI void *VirtualAlloc(void *, u64, u32, u32);
extern WINAPI u32 GetCurrentThreadId(void);
extern WINAPI void *CreateFileW(const u16 *, u32, u32, void *, u32, u32, void *);
extern WINAPI int WriteFile(void *, const void *, u32, u32 *, void *);
extern WINAPI int CloseHandle(void *);

#define PAGE_READWRITE 0x04
#define PAGE_EXECUTE_READWRITE 0x40
#define MEM_COMMIT 0x1000
#define MEM_RESERVE 0x2000
#define GENERIC_WRITE 0x40000000
#define CREATE_ALWAYS 2
#define INVALID_HANDLE ((void *)-1)

// -- PE, a mano --------------------------------------------------------------
typedef struct { u16 e_magic; u8 pad[58]; u32 e_lfanew; } DOS;
typedef struct { u32 VA; u32 Size; } DIR;
typedef struct {
    u32 Signature; u8 fh[20];
    u16 Magic; u8 oh_a[110];   // DataDirectory esta a 112 de Magic (PE32+)
    DIR Dir[16];
} NT;
typedef struct {
    u32 OriginalFirstThunk; u32 TimeDateStamp; u32 ForwarderChain;
    u32 Name; u32 FirstThunk;
} IMPDESC;
typedef struct { u16 Hint; char Name[1]; } IMPNAME;

#define IMAGE_ORDINAL_FLAG64 0x8000000000000000ull

// -- Estado ------------------------------------------------------------------
#define PUESTOS 8192
// g_destino y g_visto los lee el trampolin en asm: enlace externo para que
// el ensamblador los resuelva (los demas estados quedan static).
u64 g_destino[PUESTOS];          // la de Windows, por indice
u8  g_visto[PUESTOS];            // 1 si ya se apunto la primera llamada
static const char *g_dll[PUESTOS];
static const char *g_fun[PUESTOS];
static u32 g_n = 0;
static u32 g_vistas = 0;

static u8 g_texto[1 << 20];      // ESPEJO.TXT en memoria (1 MiB basta)
static u32 g_len = 0;
static u16 g_ruta[520];          // ESPEJO.TXT, al lado del .exe
static u32 g_ruta_ok = 0;

// El principal es el hilo 7, como los que da PROTON-X, para que los dos
// ficheros se comparen. Los demas, 8, 9... por orden de aparicion.
static u32 g_hilo_win[64];
static u32 g_hilo_bmo[64];
static u32 g_hilos = 0;
static u32 g_primer_hilo = 0;

// -- Texto sin CRT -----------------------------------------------------------
static void add(const char *s) { while (*s) g_texto[g_len++] = (u8)*s++; }
static void add_u32(u32 v, int ancho) {
    char b[16]; int n = 0;
    if (v == 0) b[n++] = '0';
    while (v) { b[n++] = (char)('0' + v % 10); v /= 10; }
    for (int i = n; i < ancho; i++) g_texto[g_len++] = ' ';
    while (n) g_texto[g_len++] = b[--n];
}

static u32 hilo_bmo(void) {
    u32 w = GetCurrentThreadId();
    for (u32 i = 0; i < g_hilos; i++)
        if (g_hilo_win[i] == w) return g_hilo_bmo[i];
    if (g_hilos < 64) {
        g_hilo_win[g_hilos] = w;
        g_hilo_bmo[g_hilos] = (g_hilos == 0) ? 7 : (7 + g_hilos);
        return g_hilo_bmo[g_hilos++];
    }
    return 0;
}

static void volcar(void) {
    if (!g_ruta_ok) return;
    void *h = CreateFileW(g_ruta, GENERIC_WRITE, 0, NULLP, CREATE_ALWAYS, 0x80, NULLP);
    if (h == INVALID_HANDLE) return;
    u32 esc = 0;
    WriteFile(h, g_texto, g_len, &esc, NULLP);
    CloseHandle(h);
}

// -- La primera llamada de la funcion `i` ------------------------------------
WINAPI void espejo_primera(u32 i) {
    if (i >= g_n || g_visto[i]) return;
    g_visto[i] = 1;
    g_vistas++;
    add_u32(g_vistas, 5); add(" ");
    add_u32(hilo_bmo(), 5); add(" ");
    add(g_dll[i]); add(" ");
    add(g_fun[i]); add("\n");
    volcar();
}

// -- El trampolin comun, en asm (como el del DIARIO) -------------------------
// A la entrada rsp = 8 mod 16. r11 lleva el indice (lo puso el stub por
// funcion). Preserva los enteros de argumento y xmm0-3, llama a espejo_primera
// y salta al destino real.
__asm__(
    ".text\n"
    ".globl espejo_comun\n"
    ".balign 16\n"
    "espejo_comun:\n"
    "    leaq g_visto(%rip), %rax\n"
    "    cmpb $0, (%rax,%r11)\n"
    "    jne 2f\n"
    "    pushq %rcx\n"
    "    pushq %rdx\n"
    "    pushq %r8\n"
    "    pushq %r9\n"
    "    pushq %r11\n"
    "    subq $0x80, %rsp\n"
    "    movdqu %xmm0, 0x20(%rsp)\n"
    "    movdqu %xmm1, 0x30(%rsp)\n"
    "    movdqu %xmm2, 0x40(%rsp)\n"
    "    movdqu %xmm3, 0x50(%rsp)\n"
    "    movl %r11d, %ecx\n"
    "    callq espejo_primera\n"
    "    movdqu 0x20(%rsp), %xmm0\n"
    "    movdqu 0x30(%rsp), %xmm1\n"
    "    movdqu 0x40(%rsp), %xmm2\n"
    "    movdqu 0x50(%rsp), %xmm3\n"
    "    addq $0x80, %rsp\n"
    "    popq %r11\n"
    "    popq %r9\n"
    "    popq %r8\n"
    "    popq %rdx\n"
    "    popq %rcx\n"
    "2:\n"
    "    leaq g_destino(%rip), %rax\n"
    "    jmpq *(%rax,%r11,8)\n"
);
extern void espejo_comun(void);

// Genera en `dst` (RWX) el stub de 18 bytes. Salto ABSOLUTO por rax (volatil,
// sin argumento, y el trampolin lo pisa de todas formas): VirtualAlloc puede
// caer a mas de 2 GiB del modulo, y un `jmp rel32` se pasaria de rango.
//   41 BB ii ii ii ii          mov  r11d, imm32   (0..5)
//   48 B8 <imm64>              mov  rax, espejo_comun  (6..15)
//   FF E0                      jmp  rax           (16..17)
#define STUB_LEN 24
static void stub(u8 *dst, u32 i) {
    u64 d = (u64)espejo_comun;
    dst[0] = 0x41; dst[1] = 0xBB;
    dst[2] = (u8)i; dst[3] = (u8)(i >> 8);
    dst[4] = (u8)(i >> 16); dst[5] = (u8)(i >> 24);
    dst[6] = 0x48; dst[7] = 0xB8;
    for (int k = 0; k < 8; k++) dst[8 + k] = (u8)(d >> (8 * k));
    dst[16] = 0xFF; dst[17] = 0xE0;
}

// -- Enganche de la IAT del .exe principal -----------------------------------
static u8 *g_stubs = NULLP;

static void enganchar(void *base) {
    u8 *b = (u8 *)base;
    DOS *dos = (DOS *)b;
    NT *nt = (NT *)(b + dos->e_lfanew);
    DIR imp = nt->Dir[1];           // IMAGE_DIRECTORY_ENTRY_IMPORT
    if (!imp.VA) return;

    // Cuantas funciones hay, para reservar los stubs de una vez.
    u32 total = 0;
    for (IMPDESC *d = (IMPDESC *)(b + imp.VA); d->Name; d++) {
        u64 *iat = (u64 *)(b + d->FirstThunk);
        for (u32 k = 0; iat[k]; k++) total++;
    }
    if (total > PUESTOS) total = PUESTOS;
    g_stubs = (u8 *)VirtualAlloc(NULLP, (u64)total * STUB_LEN, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE);
    if (!g_stubs) return;

    for (IMPDESC *d = (IMPDESC *)(b + imp.VA); d->Name; d++) {
        const char *dll = (const char *)(b + d->Name);
        u64 *orig = d->OriginalFirstThunk ? (u64 *)(b + d->OriginalFirstThunk) : (u64 *)(b + d->FirstThunk);
        u64 *iat = (u64 *)(b + d->FirstThunk);
        for (u32 k = 0; iat[k]; k++) {
            if (g_n >= PUESTOS) return;
            const char *fun;
            if (orig[k] & IMAGE_ORDINAL_FLAG64) {
                fun = "#ordinal";       // por ordinal: sin nombre
            } else {
                IMPNAME *nm = (IMPNAME *)(b + orig[k]);
                fun = nm->Name;
            }
            u32 i = g_n++;
            g_dll[i] = dll;
            g_fun[i] = fun;
            g_destino[i] = iat[k];
            u8 *s = g_stubs + (u64)i * STUB_LEN;
            stub(s, i);
            u32 viejo;
            if (VirtualProtect(&iat[k], 8, PAGE_READWRITE, &viejo)) {
                iat[k] = (u64)s;
                VirtualProtect(&iat[k], 8, viejo, &viejo);
            }
        }
    }
}

// ESPEJO.TXT, al lado del .exe (misma carpeta, nombre cambiado).
static void ruta_del_exe(void) {
    u32 n = GetModuleFileNameW(NULLP, g_ruta, 512);
    if (!n || n >= 512) return;
    while (n && g_ruta[n - 1] != '\\' && g_ruta[n - 1] != '/') n--;
    const char *nombre = "ESPEJO.TXT";
    for (u32 i = 0; nombre[i]; i++) g_ruta[n++] = (u16)nombre[i];
    g_ruta[n] = 0;
    g_ruta_ok = 1;
}

static const char *CAB =
    "# ESPEJO de Windows: cada funcion, la PRIMERA vez que el .exe la llama\n"
    "# orden hilo dll funcion\n";

int WINAPI DllMain(void *inst, u32 razon, void *reservado) {
    (void)inst; (void)reservado;
    if (razon == 1) {               // DLL_PROCESS_ATTACH
        g_primer_hilo = GetCurrentThreadId();
        add(CAB);
        ruta_del_exe();
        enganchar(GetModuleHandleW(NULLP));
        volcar();
    }
    return 1;
}
