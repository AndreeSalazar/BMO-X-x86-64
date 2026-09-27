/* teb.c -- el .exe de P1d de PROTON-X. Sin CRT, pero lee `gs:` como lo lee
 * el CRT de Microsoft al arrancar:
 *
 *    gs:[0x30]  el TEB, que se apunta a si mismo      (NtCurrentTeb)
 *    gs:[0x60]  el PEB, el mismo que dice el TEB
 *    PEB+0x10   la base de la imagen                  (GetModuleHandle(NULL))
 *    TEB+0x08   StackBase y TEB+0x10 StackLimit: la pila de verdad
 *    gs:[0x68]  LastErrorValue: SetLastError escribe ahi, GetLastError lee
 *    TEB+0x40   el proceso y TEB+0x48 el hilo: los de GetCurrent*Id
 *
 * Dice cada comprobacion con su valor y sale con el numero de las que
 * fallaron: 0 es que el TEB y el PEB son los que un .exe espera. Las mismas
 * siete funciones de kernel32 corren igual en Windows. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned long long U64;
__declspec(dllimport) HANDLE __stdcall GetStdHandle(DWORD n);
__declspec(dllimport) int __stdcall WriteFile(HANDLE h, const void *b, DWORD n, DWORD *escritos, void *ov);
__declspec(dllimport) void __stdcall ExitProcess(unsigned int codigo);
__declspec(dllimport) void __stdcall SetLastError(DWORD e);
__declspec(dllimport) DWORD __stdcall GetLastError(void);
__declspec(dllimport) DWORD __stdcall GetCurrentProcessId(void);
__declspec(dllimport) DWORD __stdcall GetCurrentThreadId(void);

/* La base de la imagen: el enlazador la da con este nombre. */
extern char __ImageBase;

static U64 gs64(unsigned long o) {
    U64 v;
    __asm__ volatile("movq %%gs:(%1), %0" : "=r"(v) : "r"((U64)o));
    return v;
}

static DWORD gs32(unsigned long o) {
    DWORD v;
    __asm__ volatile("movl %%gs:(%1), %0" : "=r"(v) : "r"((U64)o));
    return v;
}

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

void inicio(void) {
    U64 teb, peb, base, limite;
    U64 pila = (U64)&teb;
    salida = GetStdHandle((DWORD)-11);
    di("teb.exe: lo que el CRT lee en gs:\r\n");

    teb = gs64(0x30);
    mira(teb != 0 && *(U64 *)(teb + 0x30) == teb, "gs:[0x30] es el TEB y se apunta a si mismo", teb);

    peb = gs64(0x60);
    mira(peb != 0 && peb == *(U64 *)(teb + 0x60), "gs:[0x60] es el PEB, el mismo del TEB", peb);

    mira(*(U64 *)(peb + 0x10) == (U64)&__ImageBase, "PEB+0x10 es la base de esta imagen", *(U64 *)(peb + 0x10));

    base = *(U64 *)(teb + 0x08);
    limite = *(U64 *)(teb + 0x10);
    mira(limite < pila && pila < base, "la pila esta entre StackLimit y StackBase", pila);

    SetLastError(0x1234);
    mira(GetLastError() == 0x1234 && gs32(0x68) == 0x1234, "SetLastError deja su valor en gs:[0x68]", gs32(0x68));

    mira(GetCurrentProcessId() == (DWORD) * (U64 *)(teb + 0x40) && GetCurrentThreadId() == (DWORD) * (U64 *)(teb + 0x48),
         "GetCurrent*Id son los del TEB", ((U64)GetCurrentProcessId() << 32) | GetCurrentThreadId());

    di(fallos ? "teb.exe: el TEB NO es el que un .exe espera\r\n" : "teb.exe: el TEB y el PEB son los de Windows\r\n");
    ExitProcess(fallos);
}
