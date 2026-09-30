/* tanda12.c -- el .exe de la TANDA 12 de Cyberpunk (30-09): red y cripto,
 * SIN red. ws2_32 importado POR ORDINAL (como Cyberpunk): lo puro de
 * verdad (htons, inet_addr, inet_pton/ntop, __WSAFDIsSet) y lo de red
 * antes de WSAStartup, que en Windows y en la casa es WSANOTINITIALISED.
 * crypt32: Base64 y hex, un almacen vacio. bcrypt: SHA-256, MD5, SHA-1, un
 * HMAC y el azar. Esta prueba NUNCA llama a WSAStartup: asi Windows (que
 * tiene red) y la casa (que no) dicen lo mismo.
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
/* ws2_32 (tanda12_ws2_32.def: por ordinal, salvo inet_pton/ntop y getaddrinfo) */
IMPORTA unsigned short W htons(unsigned short v);
IMPORTA unsigned short W ntohs(unsigned short v);
IMPORTA unsigned long W htonl(unsigned long v);
IMPORTA unsigned long W inet_addr(const char *s);
IMPORTA U64 W socket(int af, int tipo, int proto);
IMPORTA int W closesocket(U64 s);
IMPORTA int W gethostname(char *b, int n);
IMPORTA int W WSAGetLastError(void);
IMPORTA void W WSASetLastError(int e);
IMPORTA int W __WSAFDIsSet(U64 s, void *set);
IMPORTA int W inet_pton(int af, const char *s, void *d);
IMPORTA const char *W inet_ntop(int af, const void *a, char *b, U64 n);
IMPORTA int W getaddrinfo(const char *n, const char *s, const void *h, void **r);
/* crypt32 */
IMPORTA int W CryptBinaryToStringW(const void *d, DWORD n, DWORD f, WCHAR *s, DWORD *cch);
IMPORTA int W CryptStringToBinaryA(const char *s, DWORD n, DWORD f, void *d, DWORD *cb, DWORD *salto, DWORD *usado);
IMPORTA int W CryptStringToBinaryW(const WCHAR *s, DWORD n, DWORD f, void *d, DWORD *cb, DWORD *salto, DWORD *usado);
IMPORTA HANDLE W CertOpenStore(U64 prov, DWORD cod, U64 p, DWORD f, const void *para);
IMPORTA int W CertCloseStore(HANDLE h, DWORD f);
IMPORTA const void *W CertEnumCertificatesInStore(HANDLE h, const void *prev);
/* bcrypt */
IMPORTA long W BCryptOpenAlgorithmProvider(HANDLE *h, const WCHAR *alg, const WCHAR *impl, DWORD f);
IMPORTA long W BCryptCloseAlgorithmProvider(HANDLE h, DWORD f);
IMPORTA long W BCryptGetProperty(HANDLE h, const WCHAR *p, void *d, DWORD n, DWORD *res, DWORD f);
IMPORTA long W BCryptCreateHash(HANDLE a, HANDLE *h, void *obj, DWORD nobj, const void *sec, DWORD nsec, DWORD f);
IMPORTA long W BCryptHashData(HANDLE h, const void *d, DWORD n, DWORD f);
IMPORTA long W BCryptFinishHash(HANDLE h, void *d, DWORD n, DWORD f);
IMPORTA long W BCryptDestroyHash(HANDLE h);
IMPORTA long W BCryptGenRandom(HANDLE a, void *d, DWORD n, DWORD f);

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

static int bytes_iguales(const unsigned char *a, const unsigned char *b, unsigned n) {
    while (n--)
        if (*a++ != *b++)
            return 0;
    return 1;
}

/* El resumen de `d` con `alg` de bcrypt (HMAC si hay clave). */
static int resumen(const WCHAR *alg, const char *clave, const char *d, const unsigned char *esperado, DWORD medida) {
    HANDLE a = 0, h = 0;
    static unsigned char v[64];
    DWORD n = 0, largo = 0, k = 0;
    int ok;
    while (d[n])
        n++;
    while (clave && clave[k])
        k++;
    ok = BCryptOpenAlgorithmProvider(&a, alg, 0, clave ? 8 : 0) == 0 && BCryptGetProperty(a, L"HashDigestLength", &largo, 4, &k, 0) == 0 && largo == medida;
    k = 0;
    while (clave && clave[k])
        k++;
    ok = ok && BCryptCreateHash(a, &h, 0, 0, clave, k, 0) == 0 && BCryptHashData(h, d, n, 0) == 0 && BCryptFinishHash(h, v, medida, 0) == 0 && bytes_iguales(v, esperado, medida);
    return ok && BCryptDestroyHash(h) == 0 && BCryptCloseAlgorithmProvider(a, 0) == 0;
}

void inicio(void);

void inicio(void) {
    static unsigned char a4[4], a6[16], d[64], set[8 + 8 * 4], azar1[32], azar2[32];
    static char t[64];
    static WCHAR b[128];
    static const unsigned char V6[16] = {0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1};
    static const unsigned char SHA256_ABC[] = {0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22, 0x23,
                                               0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00, 0x15, 0xad};
    static const unsigned char MD5_ABC[] = {0x90, 0x01, 0x50, 0x98, 0x3c, 0xd2, 0x4f, 0xb0, 0xd6, 0x96, 0x3f, 0x7d, 0x28, 0xe1, 0x7f, 0x72};
    static const unsigned char SHA1_ABC[] = {0xa9, 0x99, 0x3e, 0x36, 0x47, 0x06, 0x81, 0x6a, 0xba, 0x3e, 0x25, 0x71, 0x78, 0x50, 0xc2, 0x6c, 0x9c, 0xd0, 0xd8, 0x9d};
    static const unsigned char HMAC_ZORRO[] = {0xf7, 0xbc, 0x83, 0xf4, 0x30, 0x53, 0x84, 0x24, 0xb1, 0x32, 0x98, 0xe6, 0xaa, 0x6f, 0xb1, 0x43,
                                               0xef, 0x4d, 0x59, 0xa1, 0x49, 0x46, 0x17, 0x59, 0x97, 0x47, 0x9d, 0xbc, 0x2d, 0x1a, 0x3c, 0xd8};
    static const unsigned char DEAD[2] = {0xde, 0xad};
    DWORD n, usado;
    void *r = (void *)1;
    HANDLE s, alg = 0, h = 0;

    /* -- ws2_32: lo puro */
    mira(htons(0x1234) == 0x3412 && ntohs(0x3412) == 0x1234 && htonl(0x01020304) == 0x04030201, "htons, ntohs y htonl (por ordinal)");
    mira(inet_addr("192.0.2.10") == htonl(0xC000020A) && inet_addr("banana") == 0xFFFFFFFF, "inet_addr, y INADDR_NONE si no es una IP");
    mira(inet_pton(2, "192.0.2.1", a4) == 1 && a4[0] == 192 && a4[3] == 1 && inet_pton(2, "256.1.1.1", a4) == 0, "inet_pton IPv4, y 0 si no lo es");
    mira(inet_pton(23, "2001:db8::1", a6) == 1 && bytes_iguales(a6, V6, 16) && inet_ntop(23, a6, t, sizeof t) == t && igual(t, "2001:db8::1"), "inet_pton e inet_ntop IPv6, ida y vuelta");
    mira(inet_pton(23, "1:0:0:2:0:0:0:3", a6) == 1 && inet_ntop(23, a6, t, sizeof t) && igual(t, "1:0:0:2::3"), "inet_ntop: la racha de ceros mas larga es la que se abrevia");
    mira(inet_pton(23, "::1", a6) == 1 && inet_ntop(23, a6, t, sizeof t) && igual(t, "::1") && inet_pton(2, "192.0.2.1", a4) == 1 && inet_ntop(2, a4, t, sizeof t) && igual(t, "192.0.2.1"), "inet_ntop ::1 y una IPv4");
    *(DWORD *)set = 2;
    *(U64 *)(set + 8) = 5, *(U64 *)(set + 16) = 7;
    mira(__WSAFDIsSet(7, set) && !__WSAFDIsSet(9, set), "__WSAFDIsSet");
    WSASetLastError(10035);
    mira(WSAGetLastError() == 10035, "WSASetLastError y WSAGetLastError");
    /* -- ws2_32: lo de red, sin WSAStartup */
    s = (HANDLE)socket(2, 1, 6);
    mira(s == (HANDLE)~0ULL && WSAGetLastError() == 10093, "socket sin WSAStartup: INVALID_SOCKET y WSANOTINITIALISED");
    mira(closesocket(1234) == -1 && WSAGetLastError() == 10093, "closesocket sin WSAStartup");
    mira(gethostname(t, sizeof t) == -1 && WSAGetLastError() == 10093, "gethostname sin WSAStartup");
    mira(getaddrinfo("localhost", "80", 0, &r) == 10093, "getaddrinfo sin WSAStartup: devuelve WSANOTINITIALISED");

    /* -- crypt32 */
    n = 0;
    mira(CryptBinaryToStringW("abc", 3, 0x40000001, 0, &n) && n == 5 && CryptBinaryToStringW("abc", 3, 0x40000001, b, &n) && n == 4 && igual_w(b, L"YWJj"), "CryptBinaryToStringW Base64 sin CRLF: la medida con el 0, luego sin el");
    n = 128;
    mira(CryptBinaryToStringW("abc", 3, 1, b, &n) && n == 6 && igual_w(b, L"YWJj\r\n"), "CryptBinaryToStringW Base64: acaba en CRLF");
    n = 128;
    mira(CryptBinaryToStringW(DEAD, 2, 0x4000000C, b, &n) && igual_w(b, L"dead"), "CryptBinaryToStringW HEXRAW");
    n = 0;
    mira(CryptStringToBinaryA("YWJj", 0, 1, 0, &n, 0, 0) && n == 3 && CryptStringToBinaryA("YWJj", 0, 1, d, &n, 0, 0) && n == 3 && bytes_iguales(d, (const unsigned char *)"abc", 3), "CryptStringToBinaryA Base64");
    n = sizeof d;
    mira(CryptStringToBinaryW(L"-----BEGIN CERTIFICATE-----\r\nYWJj\r\n-----END CERTIFICATE-----\r\n", 0, 0, d, &n, 0, &usado) && n == 3 && bytes_iguales(d, (const unsigned char *)"abc", 3), "CryptStringToBinaryW con su cabecera");
    s = CertOpenStore(2, 0, 0, 0, 0);
    mira(s && !CertEnumCertificatesInStore(s, 0) && GetLastError() == 0x80092004 && CertCloseStore(s, 0), "CertOpenStore en memoria: vacio, CRYPT_E_NOT_FOUND");

    /* -- bcrypt */
    mira(resumen(L"SHA256", 0, "abc", SHA256_ABC, 32), "BCrypt SHA-256 de \"abc\"");
    mira(resumen(L"MD5", 0, "abc", MD5_ABC, 16) && resumen(L"SHA1", 0, "abc", SHA1_ABC, 20), "BCrypt MD5 y SHA-1");
    mira(resumen(L"SHA256", "key", "The quick brown fox jumps over the lazy dog", HMAC_ZORRO, 32), "BCrypt HMAC-SHA256 (el zorro)");
    mira(BCryptOpenAlgorithmProvider(&alg, L"SHA256", 0, 0) == 0 && BCryptCreateHash(alg, &h, 0, 0, 0, 0, 0) == 0 && BCryptFinishHash(h, d, 16, 0) == (long)0xC000000D && BCryptDestroyHash(h) == 0 && BCryptCloseAlgorithmProvider(alg, 0) == 0,
         "BCryptFinishHash con otra medida: STATUS_INVALID_PARAMETER");
    mira(BCryptGenRandom(0, azar1, 32, 2) == 0 && BCryptGenRandom(0, azar2, 32, 2) == 0 && !bytes_iguales(azar1, azar2, 32), "BCryptGenRandom del sistema: dos veces, dos distintos");
    mira(BCryptOpenAlgorithmProvider(&alg, L"BMO_NO", 0, 0) == (long)0xC0000225, "BCryptOpenAlgorithmProvider de uno que no hay: STATUS_NOT_FOUND");
    di("tanda12.exe: la red y la cripto sin red son lo de Windows\r\n");
    ExitProcess(fallos);
}
