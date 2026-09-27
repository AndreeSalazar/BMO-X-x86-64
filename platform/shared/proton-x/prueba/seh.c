/* seh.c -- el .exe de P4c de PROTON-X: las excepciones estructuradas (SEH) de
 * Windows x64, como las usan el CRT y los juegos.
 *
 *    __try / __except       el filtro ve el codigo y los parametros de
 *                           RaiseException; el bloque, GetExceptionCode()
 *    __try / __finally      el __finally de una funcion de en medio corre AL
 *                           DESENROLLAR, antes que el __except de arriba, y
 *                           AbnormalTermination() dice que si
 *    EXCEPTION_CONTINUE_SEARCH   el __except de dentro pasa; lo coge el de fuera
 *    EXCEPTION_CONTINUE_EXECUTION  RaiseException vuelve, y se sigue
 *    tres marcos de profundidad: se desenrollan y el de arriba sigue con SUS
 *        registros (rbx, rsi, rdi, r12..r15 que el compilador guardo)
 *    RtlCaptureContext + RtlLookupFunctionEntry + RtlVirtualUnwind: el marco
 *        de quien llamo, con su direccion de vuelta
 *    una excepcion dentro de un HILO, cogida en ese hilo
 *    y al final, SetUnhandledExceptionFilter: una que nadie coge llega a el
 *
 * Sin CRT: `__C_specific_handler` (el manejador de `__try` de C) lo exporta
 * ntdll.dll. Sale con el numero de fallos -- desde el filtro de las no
 * manejadas, que es el ultimo que habla. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned long long U64;
typedef long LONG;

typedef struct EXCEPTION_RECORD {
    DWORD ExceptionCode, ExceptionFlags;
    struct EXCEPTION_RECORD *ExceptionRecord;
    void *ExceptionAddress;
    DWORD NumberParameters;
    U64 ExceptionInformation[15];
} EXCEPTION_RECORD;
typedef struct { EXCEPTION_RECORD *ExceptionRecord; void *ContextRecord; } EXCEPTION_POINTERS;
typedef struct { DWORD BeginAddress, EndAddress, UnwindData; } RUNTIME_FUNCTION;
typedef LONG(__stdcall *FILTRO)(EXCEPTION_POINTERS *);
typedef DWORD(__stdcall *PROC)(void *);

#define WINAPI __stdcall
#define IMPORTA __declspec(dllimport)
IMPORTA HANDLE WINAPI GetStdHandle(DWORD n);
IMPORTA int WINAPI WriteFile(HANDLE h, const void *b, DWORD n, DWORD *escritos, void *ov);
IMPORTA void WINAPI ExitProcess(unsigned int codigo);
IMPORTA void WINAPI RaiseException(DWORD codigo, DWORD banderas, DWORD n, const U64 *args);
IMPORTA FILTRO WINAPI SetUnhandledExceptionFilter(FILTRO f);
IMPORTA void WINAPI RtlCaptureContext(void *ctx);
IMPORTA RUNTIME_FUNCTION *WINAPI RtlLookupFunctionEntry(U64 pc, U64 *base, void *historia);
IMPORTA void *WINAPI RtlVirtualUnwind(DWORD tipo, U64 base, U64 pc, RUNTIME_FUNCTION *f, void *ctx, void **datos, U64 *marco, void *punteros);
IMPORTA HANDLE WINAPI CreateThread(void *attr, U64 pila, PROC f, void *arg, DWORD banderas, DWORD *id);
IMPORTA DWORD WINAPI WaitForSingleObject(HANDLE h, DWORD ms);
IMPORTA int WINAPI GetExitCodeThread(HANDLE h, DWORD *codigo);

/* Lo de excpt.h: los intrinsecos que el compilador da dentro de __except. */
unsigned long __cdecl _exception_code(void);
void *__cdecl _exception_info(void);
#define GetExceptionCode _exception_code
#define GetExceptionInformation() ((EXCEPTION_POINTERS *)_exception_info())

#define EXCEPTION_EXECUTE_HANDLER 1
#define EXCEPTION_CONTINUE_SEARCH 0
#define EXCEPTION_CONTINUE_EXECUTION (-1)
#define INFINITE 0xFFFFFFFF
/* Rip y Rsp dentro del CONTEXT de x64. */
#define CTX_RSP 0x98
#define CTX_RIP 0xF8

static HANDLE salida;
static unsigned fallos;

static void di(const char *s) {
    DWORD n = 0, e;
    while (s[n]) n++;
    WriteFile(salida, s, n, &e, 0);
}

static void hex(U64 v) {
    char b[19];
    int i;
    b[0] = '0';
    b[1] = 'x';
    for (i = 0; i < 16; i++) {
        unsigned d = (unsigned)(v >> (60 - 4 * i)) & 15;
        b[2 + i] = (char)(d < 10 ? '0' + d : 'a' + d - 10);
    }
    b[18] = 0;
    di(b);
}

static void mira(int bien, const char *que, U64 valor) {
    di(bien ? "  bien  " : "  MAL   ");
    di(que);
    di(" ");
    hex(valor);
    di("\r\n");
    if (!bien) fallos++;
}

/* El orden en que pasan las cosas. */
static int orden[16], n_orden;
static void marca(int x) {
    if (n_orden < 16) orden[n_orden++] = x;
}

static volatile DWORD visto_codigo;
static volatile U64 visto_param;

static int filtro(EXCEPTION_POINTERS *p, int respuesta) {
    visto_codigo = p->ExceptionRecord->ExceptionCode;
    visto_param = p->ExceptionRecord->NumberParameters ? p->ExceptionRecord->ExceptionInformation[0] : 0;
    return respuesta;
}

__declspec(noinline) static void lanza(DWORD codigo, U64 param) {
    RaiseException(codigo, 0, 1, &param);
}

/* Un __finally en medio: tiene que correr al desenrollar. */
__declspec(noinline) static void con_finally(void) {
    __try {
        marca(1);
        lanza(0xE0000002, 0);
        marca(99);
    } __finally {
        marca(_abnormal_termination() ? 2 : 98);
    }
}

/* Tres marcos que guardan registros no volatiles: al coger arriba, el de
 * arriba sigue con los suyos. */
static volatile U64 sumidero;
__declspec(noinline) static void hondo(int n) {
    volatile U64 a = 0x1111 * n, b = 0x2222 * n;
    if (n == 0) {
        lanza(0xE0000005, 5);
        return;
    }
    hondo(n - 1);
    sumidero = a + b;
}

/* RtlVirtualUnwind: el marco de quien me llamo. */
static U64 contexto[0x4D0 / 8 + 2];
__declspec(noinline) static U64 quien_me_llamo(U64 *vuelta) {
    U64 base = 0, marco = 0;
    void *datos = 0;
    RUNTIME_FUNCTION *f;
    U64 *ctx = (U64 *)(((U64)contexto + 15) & ~(U64)15);
    RtlCaptureContext(ctx);
    f = RtlLookupFunctionEntry(ctx[CTX_RIP / 8], &base, 0);
    if (!f) return 0;
    RtlVirtualUnwind(0, base, ctx[CTX_RIP / 8], f, ctx, &datos, &marco, 0);
    *vuelta = (U64)__builtin_return_address(0);
    return ctx[CTX_RIP / 8];
}

static DWORD WINAPI en_un_hilo(void *arg) {
    (void)arg;
    __try {
        lanza(0xE0000007, 7);
    } __except (filtro(GetExceptionInformation(), EXCEPTION_EXECUTE_HANDLER)) {
        return GetExceptionCode() & 0xFF;
    }
    return 0;
}

static LONG WINAPI sin_manejar(EXCEPTION_POINTERS *p) {
    mira(p->ExceptionRecord->ExceptionCode == 0xE00000FF, "SetUnhandledExceptionFilter: la que nadie coge llega al filtro", p->ExceptionRecord->ExceptionCode);
    di(fallos ? "seh.exe: ALGO NO es como en Windows\r\n" : "seh.exe: las excepciones son las de Windows\r\n");
    ExitProcess(fallos);
    return EXCEPTION_EXECUTE_HANDLER;
}

void inicio(void) {
    volatile DWORD en_except = 0;
    volatile int siguio = 0, fuera = 0, dentro = 0;
    U64 vuelta = 0, rip = 0;
    HANDLE h;
    DWORD codigo = 0, id;
    salida = GetStdHandle((DWORD)-11);
    di("seh.exe: excepciones estructuradas de Windows x64\r\n");

    __try {
        lanza(0xE0000001, 0x1234);
    } __except (filtro(GetExceptionInformation(), EXCEPTION_EXECUTE_HANDLER)) {
        en_except = GetExceptionCode();
    }
    mira(visto_codigo == 0xE0000001 && visto_param == 0x1234, "el filtro de __except ve el codigo y el parametro", visto_param);
    mira(en_except == 0xE0000001, "y el bloque __except, GetExceptionCode()", en_except);

    __try {
        con_finally();
    } __except (EXCEPTION_EXECUTE_HANDLER) {
        marca(3);
    }
    mira(n_orden == 3 && orden[0] == 1 && orden[1] == 2 && orden[2] == 3, "__finally corre al desenrollar, antes que el __except de arriba", (U64)n_orden);

    __try {
        __try {
            lanza(0xE0000003, 0);
        } __except (filtro(GetExceptionInformation(), EXCEPTION_CONTINUE_SEARCH)) {
            dentro = 1;
        }
    } __except (EXCEPTION_EXECUTE_HANDLER) {
        fuera = 1;
    }
    mira(!dentro && fuera, "CONTINUE_SEARCH: el de dentro pasa, lo coge el de fuera", (U64)(dentro * 2 + fuera));

    __try {
        lanza(0xE0000004, 0);
        siguio = 1;
    } __except (filtro(GetExceptionInformation(), EXCEPTION_CONTINUE_EXECUTION)) {
        siguio = 2;
    }
    mira(siguio == 1, "CONTINUE_EXECUTION: RaiseException vuelve y se sigue", (U64)siguio);

    {
        volatile U64 x = 0xABCDEF, y = 0x123456;
        __try {
            hondo(3);
        } __except (filtro(GetExceptionInformation(), EXCEPTION_EXECUTE_HANDLER)) {
            x += 1;
        }
        mira(x == 0xABCDF0 && y == 0x123456 && visto_codigo == 0xE0000005 && visto_param == 5, "tres marcos desenrollados: el de arriba sigue con lo suyo", x);
    }

    rip = quien_me_llamo(&vuelta);
    mira(rip != 0 && rip == vuelta, "RtlVirtualUnwind da la vuelta a quien llamo (y RtlLookupFunctionEntry su funcion)", rip - vuelta);

    h = CreateThread(0, 0, en_un_hilo, 0, 0, &id);
    WaitForSingleObject(h, INFINITE);
    GetExitCodeThread(h, &codigo);
    mira(codigo == 7, "una excepcion en un hilo, cogida en ese hilo", codigo);

    SetUnhandledExceptionFilter(sin_manejar);
    lanza(0xE00000FF, 0);
    mira(0, "una excepcion sin manejar NO puede volver aqui", 0);
    ExitProcess(0xBAD);
}
