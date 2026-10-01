/* tanda39.c -- el .exe de la TANDA 39 (02-10): VirtualQuery de TODA
 * direccion. Por Cyberpunk: pregunta por una direccion que no es de
 * VirtualAlloc, la casa contestaba 0 (y el aviso no se veia: solo se dicen
 * los 8 primeros), el juego buscaba la region en su tabla, no la encontraba
 * (-1) y leia tabla[-1]: fallo de pagina en Cyberpunk2077.exe+0x24d8b3.
 * Aqui, como en Windows: un dato y el codigo del .exe son MEM_IMAGE con
 * AllocationBase = su HMODULE (el dato escribible, el codigo ejecutable);
 * una variable de la pila y un bloque del monton son MEM_PRIVATE, hechos y
 * escribibles; VirtualAlloc da su base y su medida, y lo solo reservado es
 * MEM_RESERVE sin proteccion. BaseAddress es la pagina y la region la cubre.
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned long long U64;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA HANDLE W GetModuleHandleW(const unsigned short *n);
typedef struct { U64 base, region; DWORD prot0; DWORD particion; U64 tam; DWORD estado, prot, tipo, relleno; } MBI;
IMPORTA U64 W VirtualQuery(const void *d, MBI *m, U64 n);
IMPORTA void *W VirtualAlloc(void *d, U64 n, DWORD tipo, DWORD prot);
IMPORTA int W VirtualFree(void *d, U64 n, DWORD tipo);
IMPORTA HANDLE W GetProcessHeap(void);
IMPORTA void *W HeapAlloc(HANDLE h, DWORD b, U64 n);
IMPORTA int W HeapFree(HANDLE h, DWORD b, void *p);

#define MEM_COMMIT 0x1000
#define MEM_RESERVE 0x2000
#define MEM_RELEASE 0x8000
#define MEM_PRIVATE 0x20000
#define MEM_IMAGE 0x1000000
#define ESCRIBIBLE 0xCC /* RW, WRITECOPY, EXECUTE_RW, EXECUTE_WRITECOPY */
#define EJECUTABLE 0xF0

static unsigned fallos;
static volatile int dato = 1;

static void di(const char *t) {
    DWORD n = 0, k = 0;
    while (t[n])
        n++;
    WriteFile(GetStdHandle((DWORD)-11), t, n, &k, 0);
}

static void mira(int bien, const char *que) {
    if (!bien)
        fallos++;
    di(bien ? "  bien  " : "  MAL   ");
    di(que);
    di("\r\n");
}

/* La pagina y la region cubren `d`. */
static int cubre(const MBI *m, const void *d) {
    U64 x = (U64)d;
    return m->base == (x & ~0xFFFULL) && m->tam >= 0x1000 && m->base + m->tam > x;
}

void inicio(void);

void inicio(void) {
    MBI m;
    U64 yo = (U64)GetModuleHandleW(0), r;
    volatile int local = 2;
    void *bloque, *p, *q;

    r = VirtualQuery((const void *)&dato, &m, sizeof m);
    mira(r == sizeof m, "un dato del .exe: contesta (48 bytes)");
    mira(m.region == yo && m.tipo == MEM_IMAGE && m.estado == MEM_COMMIT && (m.prot & ESCRIBIBLE) && cubre(&m, (const void *)&dato), "y es MEM_IMAGE de su HMODULE, hecho y escribible");
    r = VirtualQuery((const void *)inicio, &m, sizeof m);
    mira(r == sizeof m && m.region == yo && m.tipo == MEM_IMAGE && (m.prot & EJECUTABLE) && cubre(&m, (const void *)inicio), "el codigo del .exe: MEM_IMAGE y ejecutable");
    r = VirtualQuery((const void *)&local, &m, sizeof m);
    mira(r == sizeof m && m.tipo == MEM_PRIVATE && m.estado == MEM_COMMIT && (m.prot & ESCRIBIBLE) && m.region != 0 && m.region <= (U64)&local && cubre(&m, (const void *)&local), "una variable de la pila: MEM_PRIVATE, hecha, escribible");
    bloque = HeapAlloc(GetProcessHeap(), 0, 100);
    r = bloque ? VirtualQuery(bloque, &m, sizeof m) : 0;
    mira(r == sizeof m && m.tipo == MEM_PRIVATE && m.estado == MEM_COMMIT && (m.prot & ESCRIBIBLE) && cubre(&m, bloque), "un bloque del monton: MEM_PRIVATE, hecho, escribible");
    if (bloque)
        HeapFree(GetProcessHeap(), 0, bloque);
    p = VirtualAlloc(0, 0x10000, MEM_RESERVE | MEM_COMMIT, 4);
    r = p ? VirtualQuery((char *)p + 0x1234, &m, sizeof m) : 0;
    mira(r == sizeof m && m.region == (U64)p && m.base == (U64)p + 0x1000 && m.tam == 0xF000 && m.estado == MEM_COMMIT && m.prot == 4 && m.tipo == MEM_PRIVATE, "VirtualAlloc: su base, de esa pagina al final");
    q = VirtualAlloc(0, 0x100000, MEM_RESERVE, 4);
    r = q ? VirtualQuery(q, &m, sizeof m) : 0;
    mira(r == sizeof m && m.region == (U64)q && m.estado == MEM_RESERVE && m.prot == 0 && m.prot0 == 4 && m.tam == 0x100000, "solo reservado: MEM_RESERVE, sin proteccion");
    r = VirtualQuery((const void *)&dato, &m, 47);
    mira(r == 0, "con 47 bytes no cabe: 0");
    if (p)
        VirtualFree(p, 0, MEM_RELEASE);
    if (q)
        VirtualFree(q, 0, MEM_RELEASE);
    di("tanda39.exe: VirtualQuery de toda direccion\r\n");
    ExitProcess(fallos);
}
