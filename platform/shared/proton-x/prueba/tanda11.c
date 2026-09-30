/* tanda11.c -- el .exe de la TANDA 11 de Cyberpunk (30-09): ADVAPI32. El
 * registro (abrir, leer, recorrer; W y A), GetUserName, CryptoAPI (los
 * proveedores, el azar y MD5/SHA-1/SHA-256 de verdad), el visor de eventos,
 * ETW y los servicios. Solo claves que tiene CUALQUIER Windows 10/11, y
 * relaciones (el fabricante del registro es el de CPUID; tantos
 * procesadores como dice GetSystemInfo): nada de esta maquina.
 *
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
IMPORTA DWORD W GetLastError(void);
IMPORTA void W SetLastError(DWORD e);
IMPORTA DWORD W GetEnvironmentVariableW(const WCHAR *n, WCHAR *b, DWORD k);
IMPORTA void W GetSystemInfo(void *si);
/* advapi32 (tanda11_advapi32.def) */
IMPORTA long W RegOpenKeyExW(HANDLE h, const WCHAR *s, DWORD o, DWORD a, HANDLE *r);
IMPORTA long W RegOpenKeyExA(HANDLE h, const char *s, DWORD o, DWORD a, HANDLE *r);
IMPORTA long W RegCloseKey(HANDLE h);
IMPORTA long W RegQueryValueExW(HANDLE h, const WCHAR *n, DWORD *r, DWORD *t, void *d, DWORD *cb);
IMPORTA long W RegGetValueW(HANDLE h, const WCHAR *s, const WCHAR *v, DWORD f, DWORD *t, void *d, DWORD *cb);
IMPORTA long W RegGetValueA(HANDLE h, const char *s, const char *v, DWORD f, DWORD *t, void *d, DWORD *cb);
IMPORTA long W RegEnumKeyExW(HANDLE h, DWORD i, WCHAR *n, DWORD *cch, DWORD *r, WCHAR *c, DWORD *cc, U64 *ft);
IMPORTA long W RegEnumKeyExA(HANDLE h, DWORD i, char *n, DWORD *cch, DWORD *r, char *c, DWORD *cc, U64 *ft);
IMPORTA long W RegEnumValueW(HANDLE h, DWORD i, WCHAR *n, DWORD *cch, DWORD *r, DWORD *t, void *d, DWORD *cb);
IMPORTA long W RegQueryInfoKeyW(HANDLE h, WCHAR *c, DWORD *cc, DWORD *r, DWORD *hijas, DWORD *mh, DWORD *mc, DWORD *vals, DWORD *mn, DWORD *md, DWORD *sd, U64 *ft);
IMPORTA long W RegQueryInfoKeyA(HANDLE h, char *c, DWORD *cc, DWORD *r, DWORD *hijas, DWORD *mh, DWORD *mc, DWORD *vals, DWORD *mn, DWORD *md, DWORD *sd, U64 *ft);
IMPORTA int W GetUserNameW(WCHAR *b, DWORD *n);
IMPORTA int W GetUserNameA(char *b, DWORD *n);
IMPORTA int W CryptAcquireContextW(U64 *p, const WCHAR *c, const WCHAR *prov, DWORD tipo, DWORD f);
IMPORTA int W CryptReleaseContext(U64 p, DWORD f);
IMPORTA int W CryptGetProvParam(U64 p, DWORD que, void *d, DWORD *n, DWORD f);
IMPORTA int W CryptEnumProvidersW(DWORD i, DWORD *r, DWORD f, DWORD *tipo, WCHAR *n, DWORD *cb);
IMPORTA int W CryptGenRandom(U64 p, DWORD n, void *b);
IMPORTA int W CryptCreateHash(U64 p, DWORD alg, U64 k, DWORD f, U64 *h);
IMPORTA int W CryptHashData(U64 h, const void *d, DWORD n, DWORD f);
IMPORTA int W CryptGetHashParam(U64 h, DWORD que, void *d, DWORD *n, DWORD f);
IMPORTA int W CryptDestroyHash(U64 h);
IMPORTA int W CryptGetUserKey(U64 p, DWORD que, U64 *k);
IMPORTA HANDLE W OpenSCManagerA(const char *m, const char *b, DWORD a);
IMPORTA HANDLE W OpenServiceA(HANDLE scm, const char *n, DWORD a);
IMPORTA int W CloseServiceHandle(HANDLE h);
IMPORTA HANDLE W RegisterEventSourceW(const WCHAR *s, const WCHAR *f);
IMPORTA int W DeregisterEventSource(HANDLE h);
IMPORTA DWORD W EventRegister(const void *guid, void *cb, void *ctx, U64 *h);
IMPORTA DWORD W EventUnregister(U64 h);
IMPORTA DWORD W EventWriteTransfer(U64 h, const void *desc, const void *act, const void *rel, DWORD n, void *datos);
IMPORTA DWORD W EventActivityIdControl(DWORD codigo, void *guid);

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

static int igual(const char *a, const char *b) {
    while (*a && *a == *b)
        a++, b++;
    return *a == *b;
}

static int empieza_w(const WCHAR *a, const WCHAR *b) {
    while (*b && *a == *b)
        a++, b++;
    return *b == 0;
}

static unsigned largo_w(const WCHAR *a) {
    unsigned n = 0;
    while (a[n])
        n++;
    return n;
}

static unsigned largo(const char *a) {
    unsigned n = 0;
    while (a[n])
        n++;
    return n;
}

static int bytes_iguales(const unsigned char *a, const unsigned char *b, unsigned n) {
    while (n--)
        if (*a++ != *b++)
            return 0;
    return 1;
}

/* El resumen de "abc" con el algoritmo `alg`, comparado con `esperado`. */
static int resumen(U64 p, DWORD alg, const unsigned char *esperado, DWORD medida) {
    U64 h = 0;
    static unsigned char v[64];
    DWORD n = sizeof v, t = 0, tn = 4, id = 0;
    int ok = CryptCreateHash(p, alg, 0, 0, &h) && CryptHashData(h, "abc", 3, 0) && CryptGetHashParam(h, 4, &t, &tn, 0) && t == medida &&
             CryptGetHashParam(h, 1, &id, &tn, 0) && id == alg && CryptGetHashParam(h, 2, v, &n, 0) && n == medida && bytes_iguales(v, esperado, medida);
    SetLastError(0);
    ok = ok && !CryptHashData(h, "d", 1, 0) && GetLastError() == 0x8009000C;
    return ok && CryptDestroyHash(h);
}

#define HKCU ((HANDLE)(U64)0x80000001)
#define HKLM ((HANDLE)(U64)0x80000002)
#define KEY_READ 0x20019
#define CV L"SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion"

void inicio(void);

void inicio(void) {
    static WCHAR b[512], e[256];
    static char a[512], fab[16];
    static unsigned char si[48], azar1[32], azar2[32], guid[16], desc[16];
    static const unsigned char MD5_ABC[] = {0x90, 0x01, 0x50, 0x98, 0x3c, 0xd2, 0x4f, 0xb0, 0xd6, 0x96, 0x3f, 0x7d, 0x28, 0xe1, 0x7f, 0x72};
    static const unsigned char SHA1_ABC[] = {0xa9, 0x99, 0x3e, 0x36, 0x47, 0x06, 0x81, 0x6a, 0xba, 0x3e, 0x25, 0x71, 0x78, 0x50, 0xc2, 0x6c, 0x9c, 0xd0, 0xd8, 0x9d};
    static const unsigned char SHA256_ABC[] = {0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22, 0x23,
                                               0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00, 0x15, 0xad};
    HANDLE k = 0, cp = 0, c0 = 0, ka = 0, scm;
    DWORD t, cb, dw, n, hijas, i, r[4];
    U64 p = 0, uk, reg = 0;
    long s;
    int hallado, ok;

    /* -- el registro: la version */
    mira(RegOpenKeyExW(HKLM, CV, 0, KEY_READ, &k) == 0 && k, "RegOpenKeyExW: HKLM\\...\\CurrentVersion");
    t = 0, cb = 4, dw = 0;
    mira(RegQueryValueExW(k, L"CurrentMajorVersionNumber", 0, &t, &dw, &cb) == 0 && t == 4 && dw == 10 && cb == 4, "RegQueryValueExW: CurrentMajorVersionNumber, un DWORD 10");
    cb = 0;
    s = RegQueryValueExW(k, L"ProductName", 0, &t, 0, &cb);
    n = cb;
    mira(s == 0 && t == 1 && cb > 2 && cb % 2 == 0 && RegQueryValueExW(k, L"ProductName", 0, &t, b, &cb) == 0 && cb == n && empieza_w(b, L"Windows") && cb == 2 * (largo_w(b) + 1), "RegQueryValueExW: ProductName, primero la medida");
    cb = 4;
    mira(RegQueryValueExW(k, L"ProductName", 0, &t, b, &cb) == 234 && cb == n, "RegQueryValueExW sin sitio: ERROR_MORE_DATA y la medida");
    mira(RegQueryValueExW(k, L"BMO_no_existe", 0, &t, b, &cb) == 2, "RegQueryValueExW de uno que no esta: ERROR_FILE_NOT_FOUND");
    cb = 4, dw = 0;
    mira(RegGetValueW(HKLM, CV, L"CurrentMajorVersionNumber", 0x10, &t, &dw, &cb) == 0 && dw == 10, "RegGetValueW RRF_RT_REG_DWORD");
    cb = sizeof b;
    mira(RegGetValueW(HKLM, CV, L"CurrentMajorVersionNumber", 0x2, &t, b, &cb) == 1630, "RegGetValueW con otro tipo: ERROR_UNSUPPORTED_TYPE");
    cb = sizeof a;
    ok = RegGetValueA(HKLM, "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion", "CurrentBuild", 0x2, &t, a, &cb) == 0 && t == 1 && largo(a) >= 5 && cb == largo(a) + 1;
    for (i = 0; ok && a[i]; i++)
        ok = a[i] >= '0' && a[i] <= '9';
    cb = sizeof b;
    mira(ok && RegGetValueW(HKLM, CV, L"CurrentBuildNumber", 0x2, &t, b, &cb) == 0 && largo_w(b) == largo(a) && b[0] == a[0], "RegGetValueA: CurrentBuild, solo cifras, igual que CurrentBuildNumber");
    mira(RegCloseKey(k) == 0 && RegCloseKey((HANDLE)(U64)0x1234) == 6, "RegCloseKey, y la de un handle que no es");
    mira(RegOpenKeyExW(HKLM, L"software\\MICROSOFT\\windows nt\\currentversion", 0, KEY_READ, &k) == 0 && RegCloseKey(k) == 0, "RegOpenKeyExW sin mirar mayusculas");
    mira(RegOpenKeyExW(HKLM, L"SOFTWARE\\BMO_no_existe", 0, KEY_READ, &k) == 2, "RegOpenKeyExW de una que no esta: ERROR_FILE_NOT_FOUND");

    /* -- el registro: el procesador */
    GetSystemInfo(si);
    hijas = 99;
    mira(RegOpenKeyExW(HKLM, L"HARDWARE\\DESCRIPTION\\System\\CentralProcessor", 0, KEY_READ, &cp) == 0 && RegQueryInfoKeyW(cp, 0, 0, 0, &hijas, 0, 0, 0, 0, 0, 0, 0) == 0 && hijas == *(DWORD *)(si + 32),
         "RegQueryInfoKeyW: tantos procesadores como dice GetSystemInfo");
    n = 64;
    mira(RegEnumKeyExW(cp, 0, b, &n, 0, 0, 0, 0) == 0 && n == 1 && igual_w(b, L"0") && (n = 64, RegEnumKeyExW(cp, hijas, b, &n, 0, 0, 0, 0)) == 259, "RegEnumKeyExW: el 0 primero, y al final ERROR_NO_MORE_ITEMS");
    {
        DWORD ra, rb, rc, rd;
        __asm__ volatile("cpuid" : "=a"(ra), "=b"(rb), "=c"(rc), "=d"(rd) : "a"(0), "c"(0));
        *(DWORD *)fab = rb, *(DWORD *)(fab + 4) = rd, *(DWORD *)(fab + 8) = rc, fab[12] = 0;
    }
    cb = sizeof b;
    mira(RegOpenKeyExW(cp, L"0", 0, KEY_READ, &c0) == 0 && RegQueryValueExW(c0, L"VendorIdentifier", 0, &t, b, &cb) == 0 && largo_w(b) == 12 && b[0] == (WCHAR)fab[0] && b[11] == (WCHAR)fab[11], "VendorIdentifier es el fabricante de CPUID");
    cb = 4, dw = 0;
    mira(RegQueryValueExW(c0, L"~MHz", 0, &t, &dw, &cb) == 0 && t == 4 && dw > 0, "~MHz: un DWORD");
    hallado = 0;
    for (i = 0;; i++) {
        n = 256, cb = sizeof b;
        s = RegEnumValueW(c0, i, e, &n, 0, &t, b, &cb);
        if (s)
            break;
        hallado |= igual_w(e, L"ProcessorNameString") && t == 1 && largo_w(b) > 0 && n == 19;
    }
    mira(hallado && s == 259, "RegEnumValueW: ProcessorNameString esta, y luego ERROR_NO_MORE_ITEMS");
    RegCloseKey(c0);
    RegCloseKey(cp);
    n = 64;
    mira(RegOpenKeyExA(HKLM, "HARDWARE\\DESCRIPTION\\System\\CentralProcessor", 0, KEY_READ, &ka) == 0 && RegQueryInfoKeyA(ka, 0, 0, 0, &hijas, r, 0, 0, 0, 0, 0, 0) == 0 && RegEnumKeyExA(ka, 0, a, &n, 0, 0, 0, 0) == 0 && igual(a, "0") && RegCloseKey(ka) == 0,
         "RegOpenKeyExA, RegQueryInfoKeyA y RegEnumKeyExA");
    cb = sizeof b;
    mira(RegOpenKeyExW(HKCU, L"Control Panel\\International", 0, KEY_READ, &k) == 0 && RegQueryValueExW(k, L"LocaleName", 0, &t, b, &cb) == 0 && t == 1 && RegCloseKey(k) == 0, "HKCU: Control Panel\\International, su LocaleName");

    /* -- el usuario */
    n = 256;
    i = GetEnvironmentVariableW(L"USERNAME", e, 256);
    mira(GetUserNameW(b, &n) && n == largo_w(b) + 1 && (i == 0 || igual_w(b, e)), "GetUserNameW: el USERNAME, con su 0 en la cuenta");
    dw = 1;
    mira(!GetUserNameW(b, &dw) && GetLastError() == 122 && dw == n, "GetUserNameW sin sitio: ERROR_INSUFFICIENT_BUFFER y lo que hace falta");
    dw = 256;
    mira(GetUserNameA(a, &dw) && dw == n && largo(a) + 1 == n, "GetUserNameA");

    /* -- CryptoAPI */
    mira(CryptAcquireContextW(&p, 0, 0, 24, 0xF0000000) && p, "CryptAcquireContextW PROV_RSA_AES, CRYPT_VERIFYCONTEXT");
    n = sizeof a, dw = 0, cb = 4;
    mira(CryptGetProvParam(p, 4, a, &n, 0) && igual(a, "Microsoft Enhanced RSA and AES Cryptographic Provider") && n == largo(a) + 1 && CryptGetProvParam(p, 16, &dw, &cb, 0) && dw == 24, "CryptGetProvParam: PP_NAME y PP_PROVTYPE");
    mira(resumen(p, 0x8003, MD5_ABC, 16), "MD5 de \"abc\" (y despues de leerlo, NTE_BAD_HASH_STATE)");
    mira(resumen(p, 0x8004, SHA1_ABC, 20), "SHA-1 de \"abc\"");
    mira(resumen(p, 0x800C, SHA256_ABC, 32), "SHA-256 de \"abc\"");
    mira(CryptGenRandom(p, 32, azar1) && CryptGenRandom(p, 32, azar2) && !bytes_iguales(azar1, azar2, 32), "CryptGenRandom: dos veces, dos distintos");
    SetLastError(0);
    mira(!CryptGetUserKey(p, 1, &uk) && GetLastError() == 0x8009000D, "CryptGetUserKey sin contenedor: NTE_NO_KEY");
    mira(CryptReleaseContext(p, 0), "CryptReleaseContext");
    SetLastError(0);
    mira(!CryptAcquireContextW(&p, 0, 0, 2, 0xF0000000) && GetLastError() == 0x80090017, "CryptAcquireContextW PROV_RSA_SIG: NTE_PROV_TYPE_NOT_DEF");
    hallado = 0;
    for (i = 0;; i++) {
        cb = sizeof b;
        if (!CryptEnumProvidersW(i, 0, 0, &t, b, &cb))
            break;
        hallado |= t == 24 && igual_w(b, L"Microsoft Enhanced RSA and AES Cryptographic Provider") && cb == 2 * (largo_w(b) + 1);
    }
    mira(hallado && GetLastError() == 259, "CryptEnumProvidersW: el de AES esta, y luego ERROR_NO_MORE_ITEMS");

    /* -- servicios y eventos */
    scm = OpenSCManagerA(0, 0, 1);
    SetLastError(0);
    mira(scm && !OpenServiceA(scm, "BMO_no_existe", 4) && GetLastError() == 1060 && CloseServiceHandle(scm), "OpenServiceA de uno que no esta: ERROR_SERVICE_DOES_NOT_EXIST");
    {
        HANDLE f = RegisterEventSourceW(0, L"BMO tanda11");
        mira(f && DeregisterEventSource(f), "RegisterEventSourceW y DeregisterEventSource");
    }
    guid[0] = 0x42, guid[15] = 0x77;
    mira(EventRegister(guid, 0, 0, &reg) == 0 && reg && EventWriteTransfer(reg, desc, 0, 0, 0, 0) == 0 && EventUnregister(reg) == 0, "ETW: EventRegister, EventWriteTransfer y EventUnregister");
    for (i = 0; i < 16; i++)
        guid[i] = 0xAA;
    ok = EventActivityIdControl(5, guid) == 0;
    for (i = 0; i < 16; i++)
        ok = ok && guid[i] == 0;
    hallado = 0;
    ok = ok && EventActivityIdControl(1, guid) == 0;
    for (i = 0; i < 16; i++)
        hallado |= guid[i] != 0;
    for (i = 0; i < 16; i++)
        guid[i] = 0;
    mira(ok && hallado && EventActivityIdControl(2, guid) == 0, "EventActivityIdControl: crear y poner da el de antes (ninguno); luego esta");
    di("tanda11.exe: ADVAPI32 es lo de Windows\r\n");
    ExitProcess(fallos);
}
