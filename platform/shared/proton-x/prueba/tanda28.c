/* tanda28.c -- el .exe de la TANDA 28 de Cyberpunk (30-09):
 * RtlUnwindEx con STATUS_UNWIND_CONSOLIDATE. En el metal REDGalaxy64.dll
 * lanzo una excepcion de C++ dentro de un try (sin red, ioctlsocket falla,
 * como en Windows sin red) y su CRT de MSVC, enlazado DENTRO de la DLL, la
 * cogio con su __CxxFrameHandler4: este llama a RtlUnwindEx con
 * STATUS_UNWIND_CONSOLIDATE y ExceptionInformation[0] = la funcion que corre
 * el catch. La casa volvia a TargetIp con rax = ReturnValue en vez de
 * llamarla, y la DLL leyo un puntero nulo.
 *
 * Aqui se hace lo mismo a mano: `medio` llama a `lanzar`, que busca el
 * marco de `medio` (RtlCaptureContext, RtlLookupFunctionEntry,
 * RtlVirtualUnwind) y desenrolla hasta el CONSOLIDANDO. Windows llama a
 * `consolidar(registro)` en la pila de debajo y sigue donde devuelva
 * (`tras`); `medio` no vuelve nunca por su camino.
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
IMPORTA void W RtlCaptureContext(void *ctx);
IMPORTA void *W RtlLookupFunctionEntry(U64 pc, U64 *base, void *historia);
IMPORTA void *W RtlVirtualUnwind(DWORD tipo, U64 base, U64 pc, void *f, void *ctx, void **datos, U64 *marco, void *punteros);
IMPORTA void W RtlUnwindEx(void *marco, void *ip, void *registro, void *valor, void *ctx, void *historia);

#define CONSOLIDAR 0x80000029u /* STATUS_UNWIND_CONSOLIDATE */
#define CTX_RIP 0xF8
#define CTX_RSP 0x98

typedef struct {
    DWORD codigo, banderas;
    U64 anidado, direccion;
    DWORD n, relleno;
    U64 info[15];
} Registro;

static unsigned fallos;

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

static __declspec(align(16)) unsigned char ctx[1232];
static __declspec(align(16)) unsigned char arriba[1232];
static Registro registro;
static volatile U64 marco_medio, pila_consolidar;
static volatile void *visto;
static volatile unsigned llamadas, volvio, pasos;

/* `tras`: donde sigue `medio` segun `consolidar`. Llega con la pila del marco
 * de `medio` (alineada a 16): se ajusta como una entrada de funcion. */
void seguir(void);
__asm__(".globl tras\n"
        "tras:\n"
        "  subq $8, %rsp\n"
        "  jmp seguir\n");
extern char tras[];

static void *W consolidar(Registro *r) {
    volatile int aqui = 0;
    llamadas++;
    visto = r;
    pila_consolidar = (U64)&aqui;
    return tras;
}

static void copiar(unsigned char *a, const unsigned char *b) {
    int i;
    for (i = 0; i < 1232; i++)
        a[i] = b[i];
}

__declspec(noinline) static void lanzar(void) {
    U64 base = 0, marco = 0, pc;
    void *f, *datos = 0;
    RtlCaptureContext(ctx);
    copiar(arriba, ctx);
    /* Subir dos marcos: el de lanzar y el de medio. */
    pc = *(U64 *)(arriba + CTX_RIP);
    f = RtlLookupFunctionEntry(pc, &base, 0);
    RtlVirtualUnwind(0, base, pc, f, arriba, &datos, &marco, 0);
    pc = *(U64 *)(arriba + CTX_RIP);
    f = RtlLookupFunctionEntry(pc, &base, 0);
    RtlVirtualUnwind(0, base, pc, f, arriba, &datos, &marco, 0);
    marco_medio = marco;
    pasos = f != 0;
    registro.codigo = CONSOLIDAR;
    registro.banderas = 1; /* EXCEPTION_NONCONTINUABLE, como el CRT */
    registro.n = 1;
    registro.info[0] = (U64)consolidar;
    RtlUnwindEx((void *)marco, (void *)pc, &registro, (void *)0x1234, ctx, 0);
}

__declspec(noinline) static int medio(void) {
    volatile int x = 3;
    lanzar();
    volvio = 1; /* Windows no pasa nunca por aqui */
    return x;
}

void seguir(void) {
    volatile int aqui = 0;
    mira(pasos, "RtlLookupFunctionEntry da el marco de medio");
    mira(llamadas == 1, "RtlUnwindEx llama UNA vez a ExceptionInformation[0]");
    mira(visto == &registro, "y le pasa el EXCEPTION_RECORD");
    mira(pila_consolidar < marco_medio, "la llamada corre en la pila de debajo del marco destino");
    mira(volvio == 0, "medio no vuelve por TargetIp");
    mira((U64)&aqui < marco_medio + 0x1000 && (U64)&aqui + 0x1000 > marco_medio, "se sigue en la pila del marco de medio");
    di("tanda28.exe: RtlUnwindEx CONSOLIDA como Windows (el catch de C++)\r\n");
    ExitProcess(fallos);
}

void inicio(void);

void inicio(void) {
    medio();
    mira(0, "medio volvio a inicio: RtlUnwindEx no consolido");
    ExitProcess(fallos + 5);
}
