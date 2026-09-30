/* tanda14.c -- el .exe de la TANDA 14a de Cyberpunk (30-09): las DLL chicas
 * del censo (DURAS: sin ellas el juego no arranca). winmm, shlwapi, shell32
 * (las carpetas), ntdll, gdi32, powrprof, normaliz (Punycode), ole32 y
 * rpcrt4 (memoria de COM y GUID), el ETW de advapi32 y XInput.
 *
 * Tus carpetas no son las de la casa: se miran las RELACIONES que Windows
 * cumple (SHGetFolderPathW da lo mismo que el entorno, las carpetas
 * conocidas lo mismo que las CSIDL...). Deja tanda14.txt en su carpeta.
 * Sale con el numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned short WCHAR;
typedef unsigned long long U64;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA DWORD W GetTickCount(void);
IMPORTA DWORD W GetEnvironmentVariableW(const WCHAR *n, WCHAR *b, DWORD t);
IMPORTA HANDLE W CreateFileW(const WCHAR *n, DWORD a, DWORD c, void *s, DWORD d, DWORD f, HANDLE t);
IMPORTA int W CloseHandle(HANDLE h);
IMPORTA DWORD W GetCurrentProcessId(void);
IMPORTA void W GetSystemInfo(void *si);
IMPORTA DWORD W timeGetTime(void);
IMPORTA unsigned W timeBeginPeriod(unsigned p);
IMPORTA unsigned W timeEndPeriod(unsigned p);
IMPORTA unsigned W timeGetDevCaps(void *t, unsigned cb);
IMPORTA int W PathFileExistsW(const WCHAR *p);
IMPORTA int W PathRemoveFileSpecW(WCHAR *p);
IMPORTA long W SHGetFolderPathW(HANDLE h, int csidl, HANDLE t, DWORD f, WCHAR *p);
IMPORTA int W SHGetSpecialFolderPathW(HANDLE h, WCHAR *p, int csidl, int crear);
IMPORTA long W SHGetKnownFolderPath(const void *id, DWORD f, HANDLE t, WCHAR **p);
IMPORTA long W NtQueryInformationProcess(HANDLE h, unsigned clase, void *b, unsigned n, unsigned *sale);
IMPORTA long W RtlRunOnceExecuteOnce(void *una, void *f, void *param, void **ctx);
IMPORTA long W RtlUTF8ToUnicodeN(WCHAR *d, unsigned max, unsigned *sale, const char *s, unsigned n);
IMPORTA HANDLE W GetStockObject(int i);
IMPORTA long W CallNtPowerInformation(int nivel, void *e, unsigned en, void *s, unsigned sn);
IMPORTA int W IdnToAscii(DWORD f, const WCHAR *s, int n, WCHAR *d, int dn);
IMPORTA void *W CoTaskMemAlloc(U64 n);
IMPORTA void *W CoTaskMemRealloc(void *p, U64 n);
IMPORTA void W CoTaskMemFree(void *p);
IMPORTA long W CoCreateGuid(void *g);
IMPORTA int W StringFromGUID2(const void *g, WCHAR *b, int n);
IMPORTA long W UuidCreate(void *g);
IMPORTA DWORD W EventRegister(const void *g, void *cb, void *ctx, U64 *h);
IMPORTA DWORD W EventUnregister(U64 h);
IMPORTA DWORD W EventActivityIdControl(DWORD codigo, void *g);
IMPORTA DWORD W XInputGetState(DWORD usuario, void *estado);

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

static int igual_w(const WCHAR *a, const WCHAR *b) {
    while (*a && *a == *b)
        a++, b++;
    return *a == *b;
}

static int igual_wa(const WCHAR *a, const char *b) {
    while (*b)
        if (*a++ != (WCHAR)*b++)
            return 0;
    return *a == 0;
}

static void copia(WCHAR *d, const WCHAR *s) {
    while ((*d++ = *s++))
        ;
}

/* PathRemoveFileSpecW sobre una copia de `s`: devuelve lo que devolvio y si
 * quedo `queda`. */
static int quita(const char *s, const char *queda, int devuelve) {
    WCHAR b[64];
    int k = 0;
    while ((b[k] = (WCHAR)s[k]))
        k++;
    return PathRemoveFileSpecW(b) == devuelve && igual_wa(b, queda);
}

static int llamadas;
static int W una_vez(void *una, void *param, void **ctx) {
    (void)una;
    llamadas++;
    *ctx = param;
    return 1;
}

static const unsigned char LOCAL_APPDATA[16] = {0x85, 0x27, 0xB3, 0xF1, 0xBA, 0x6F, 0xCF, 0x4F, 0x9D, 0x55, 0x7B, 0x8E, 0x7F, 0x15, 0x70, 0x91};
static const unsigned char SAVED_GAMES[16] = {0xFF, 0x32, 0x5C, 0x4C, 0x9D, 0xBB, 0xB0, 0x43, 0xB5, 0xB4, 0x2D, 0x72, 0xE5, 0x4E, 0xAA, 0xA4};

static int es_v4(const unsigned char *g) {
    return (g[7] >> 4) == 4 && (g[8] & 0xC0) == 0x80;
}

static unsigned char si[48];
static WCHAR a[300], b[300], c[300];

void inicio(void) {
    /* -- winmm */
    {
        DWORD t = timeGetTime(), k = GetTickCount();
        mira(t - k + 100 < 200, "timeGetTime: el mismo reloj que GetTickCount");
    }
    mira(timeBeginPeriod(1) == 0 && timeEndPeriod(1) == 0 && timeBeginPeriod(0) == 97, "timeBeginPeriod y timeEndPeriod: 1 ms si, 0 no (TIMERR_NOCANDO)");
    {
        unsigned tc[2] = {0, 0};
        mira(timeGetDevCaps(tc, 8) == 0 && tc[0] == 1 && tc[1] == 1000000, "timeGetDevCaps: de 1 a 1.000.000 ms");
    }
    /* -- shlwapi */
    mira(quita("C:\\a\\b.txt", "C:\\a", 1) && quita("C:\\a.txt", "C:\\", 1) && quita("C:\\", "C:\\", 0) && quita("\\a", "\\", 1), "PathRemoveFileSpecW: el nombre fuera, la barra de la raiz se queda");
    {
        HANDLE f = CreateFileW(L"tanda14.txt", 0x40000000, 0, 0, 2, 0x80, 0);
        CloseHandle(f);
        mira(PathFileExistsW(L"tanda14.txt") && !PathFileExistsW(L"no_esta_tanda14.txt"), "PathFileExistsW: el que se acaba de crear si, otro no");
    }
    /* -- shell32: las carpetas */
    mira(SHGetFolderPathW(0, 0x1A, 0, 0, a) == 0 && GetEnvironmentVariableW(L"APPDATA", b, 300) && igual_w(a, b), "SHGetFolderPathW(CSIDL_APPDATA) es %APPDATA%");
    mira(SHGetFolderPathW(0, 0x1C, 0, 0, a) == 0 && GetEnvironmentVariableW(L"LOCALAPPDATA", b, 300) && igual_w(a, b), "SHGetFolderPathW(CSIDL_LOCAL_APPDATA) es %LOCALAPPDATA%");
    mira(SHGetFolderPathW(0, 0x28, 0, 0, a) == 0 && GetEnvironmentVariableW(L"USERPROFILE", b, 300) && igual_w(a, b), "SHGetFolderPathW(CSIDL_PROFILE) es %USERPROFILE%");
    mira(SHGetSpecialFolderPathW(0, c, 0x1C, 0) && SHGetFolderPathW(0, 0x1C, 0, 0, a) == 0 && igual_w(a, c), "SHGetSpecialFolderPathW: lo mismo");
    {
        WCHAR *p = 0;
        int bien = SHGetKnownFolderPath(LOCAL_APPDATA, 0, 0, &p) == 0 && p && igual_w(p, a);
        CoTaskMemFree(p);
        mira(bien, "SHGetKnownFolderPath(FOLDERID_LocalAppData): lo mismo, en CoTaskMem");
    }
    {
        WCHAR *p = 0;
        int bien = SHGetKnownFolderPath(SAVED_GAMES, 0, 0, &p) == 0 && p;
        SHGetFolderPathW(0, 0x28, 0, 0, b);
        {
            int k = 0;
            while (b[k])
                k++;
            copia(b + k, L"\\Saved Games");
        }
        mira(bien && igual_w(p, b), "SHGetKnownFolderPath(FOLDERID_SavedGames): %USERPROFILE%\\Saved Games (las partidas)");
        CoTaskMemFree(p);
    }
    /* -- ntdll */
    {
        unsigned char pbi[48] = {0};
        unsigned sale = 0;
        U64 peb;
        __asm__("movq %%gs:0x60, %0" : "=r"(peb));
        mira(NtQueryInformationProcess((HANDLE)(long long)-1, 0, pbi, 48, &sale) == 0 && sale == 48 && *(U64 *)(pbi + 8) == peb && *(U64 *)(pbi + 32) == GetCurrentProcessId(), "NtQueryInformationProcess basica: el PEB de gs:[60h] y el pid");
        mira(NtQueryInformationProcess((HANDLE)(long long)-1, 0, pbi, 47, &sale) == (long)0xC0000004, "NtQueryInformationProcess con otra medida: STATUS_INFO_LENGTH_MISMATCH");
    }
    {
        U64 puerto = 1;
        DWORD banderas = 0;
        mira(NtQueryInformationProcess((HANDLE)(long long)-1, 7, &puerto, 8, 0) == 0 && puerto == 0 && NtQueryInformationProcess((HANDLE)(long long)-1, 31, &banderas, 4, 0) == 0 && banderas == 1, "NtQueryInformationProcess: nadie depura (puerto 0, banderas 1)");
    }
    {
        void *una = 0, *ctx = 0;
        int bien = RtlRunOnceExecuteOnce(&una, (void *)una_vez, (void *)0x1234, &ctx) == 0 && ctx == (void *)0x1234;
        ctx = 0;
        bien = bien && RtlRunOnceExecuteOnce(&una, (void *)una_vez, (void *)0x5678, &ctx) == 0 && ctx == (void *)0x1234;
        mira(bien && llamadas == 1, "RtlRunOnceExecuteOnce: una vez, y el contexto de la primera");
    }
    {
        WCHAR d[16];
        unsigned sale = 0;
        int bien = RtlUTF8ToUnicodeN(0, 0, &sale, "h\xC3\xA9llo", 6) == 0 && sale == 10;
        bien = bien && RtlUTF8ToUnicodeN(d, 32, &sale, "h\xC3\xA9llo", 6) == 0 && sale == 10 && d[0] == 'h' && d[1] == 0xE9 && d[4] == 'o';
        mira(bien, "RtlUTF8ToUnicodeN: la medida y la conversion");
        mira(RtlUTF8ToUnicodeN(d, 32, &sale, "a\xFF" "b", 3) == 0x107 && sale == 6 && d[1] == 0xFFFD, "RtlUTF8ToUnicodeN de algo que no es UTF-8: U+FFFD y STATUS_SOME_NOT_MAPPED");
    }
    /* -- gdi32 y powrprof */
    mira(GetStockObject(0) && GetStockObject(0) == GetStockObject(0) && GetStockObject(0) != GetStockObject(4) && !GetStockObject(9), "GetStockObject: fijos, distintos, y el 9 no existe");
    {
        static unsigned b24[64 * 6];
        unsigned n, k;
        int bien;
        GetSystemInfo(si);
        n = *(unsigned *)(si + 32);
        bien = n >= 1 && n <= 64 && CallNtPowerInformation(11, 0, 0, b24, n * 24) == 0;
        for (k = 0; bien && k < n; k++)
            bien = b24[k * 6] == k && b24[k * 6 + 1] > 0;
        mira(bien, "CallNtPowerInformation(ProcessorInformation): uno por procesador, con sus MHz");
        mira(CallNtPowerInformation(11, 0, 0, b24, n * 24 - 1) == (long)0xC0000023, "CallNtPowerInformation corto: STATUS_BUFFER_TOO_SMALL");
    }
    /* -- normaliz */
    mira(IdnToAscii(0, L"b\u00fccher.example", -1, a, 64) == 22 && igual_wa(a, "xn--bcher-kva.example") && IdnToAscii(0, L"b\u00fccher.example", -1, 0, 0) == 22, "IdnToAscii: Punycode, y la medida sin destino");
    mira(IdnToAscii(0, L"example.com", -1, a, 64) == 12 && igual_wa(a, "example.com"), "IdnToAscii de ASCII: igual");
    /* -- ole32 y rpcrt4 */
    {
        char *p = (char *)CoTaskMemAlloc(8);
        int bien;
        p[0] = 'b', p[7] = 'x';
        p = (char *)CoTaskMemRealloc(p, 4096);
        bien = p && p[0] == 'b' && p[7] == 'x';
        CoTaskMemFree(p);
        mira(bien, "CoTaskMemAlloc, CoTaskMemRealloc (lo de antes se queda) y CoTaskMemFree");
    }
    {
        unsigned char g1[16], g2[16], u[16];
        int k, distintos = 0, bien;
        for (k = 0; k < 16; k++)
            g1[k] = g2[k] = 0;
        bien = CoCreateGuid(g1) == 0 && CoCreateGuid(g2) == 0 && UuidCreate(u) == 0;
        for (k = 0; k < 16; k++)
            distintos |= g1[k] != g2[k];
        mira(bien && distintos && es_v4(g1) && es_v4(g2) && es_v4(u), "CoCreateGuid y UuidCreate: nuevos, de la version 4");
        mira(StringFromGUID2(g1, a, 64) == 39 && a[0] == '{' && a[9] == '-' && a[14] == '-' && a[19] == '-' && a[24] == '-' && a[37] == '}' && a[38] == 0 && StringFromGUID2(g1, a, 38) == 0, "StringFromGUID2: {8-4-4-4-12} y su cero; si no cabe, 0");
    }
    /* -- ETW y XInput */
    {
        unsigned char g[16] = {1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16}, act[16] = {0};
        U64 h = 0;
        int k, algo = 0;
        int bien = EventRegister(g, 0, 0, &h) == 0 && h != 0 && EventUnregister(h) == 0 && EventActivityIdControl(3, act) == 0;
        for (k = 0; k < 16; k++)
            algo |= act[k];
        mira(bien && algo, "EventRegister, EventUnregister y EventActivityIdControl (CREATE_ID: un id que no es cero)");
    }
    {
        unsigned char st[16];
        mira(XInputGetState(3, st) == 1167, "XInputGetState del cuarto mando: no hay (ERROR_DEVICE_NOT_CONNECTED)");
    }
    di("tanda14.exe: las DLL chicas del censo dicen lo de Windows\r\n");
    ExitProcess(fallos);
}
