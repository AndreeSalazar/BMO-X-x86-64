/* tanda30.c -- el .exe de la TANDA 30 (01-10): Winsock ARRANCA aunque no
 * haya cable. Por Cyberpunk: Galaxy (REDGalaxy64.dll) llama a WSAStartup y,
 * si no da 0, su Init falla y el juego sale. Un Windows sin cable da 0, la
 * version 2.2 y "Running"; lo que falla es resolver NOMBRES. Una IPv4 en
 * numeros se contesta sin red, en getaddrinfo y en GetAddrInfoW, con su
 * ADDRINFO; un servicio que no es un numero ni esta en la lista: 10109.
 * WSACleanup cuenta los WSAStartup y, sin ninguno, WSANOTINITIALISED.
 *
 * Nada de lo que se mira depende de tener red: Windows con cable y sin el
 * dicen lo mismo, y la casa tambien.
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
/* ws2_32 (tanda30_ws2_32.def: por ordinal, como Cyberpunk, salvo las de nombre) */
IMPORTA int W WSAStartup(unsigned short v, void *datos);
IMPORTA int W WSACleanup(void);
IMPORTA int W WSAGetLastError(void);
IMPORTA int W getaddrinfo(const char *n, const char *s, const void *h, void **r);
IMPORTA void W freeaddrinfo(void *r);
IMPORTA int W GetAddrInfoW(const WCHAR *n, const WCHAR *s, const void *h, void **r);
IMPORTA void W FreeAddrInfoW(void *r);

/* ADDRINFOA y ADDRINFOW de x64: la misma disposicion. */
struct Info {
    int flags, familia, tipo, protocolo;
    U64 largo;
    void *canonico;
    unsigned char *direccion;
    struct Info *siguiente;
};

#define AF_INET 2
#define SOCK_STREAM 1
#define IPPROTO_TCP 6
#define WSANOTINITIALISED 10093
#define WSATYPE_NOT_FOUND 10109

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

static int igual(const char *a, const char *b) {
    while (*a && *a == *b)
        a++, b++;
    return *a == *b;
}

/* Un ADDRINFO de una IPv4 127.0.0.1 al puerto `p`, TCP. */
static int es_local(const struct Info *i, unsigned p) {
    const unsigned char *d = i->direccion;
    return i->familia == AF_INET && i->tipo == SOCK_STREAM && i->protocolo == IPPROTO_TCP && i->largo == 16 && i->siguiente == 0 && d && d[0] == AF_INET && d[1] == 0 &&
           d[2] == (p >> 8) && d[3] == (p & 0xFF) && d[4] == 127 && d[5] == 0 && d[6] == 0 && d[7] == 1;
}

void inicio(void);

void inicio(void) {
    static unsigned char wsa[408];
    struct Info pista = {0, AF_INET, SOCK_STREAM, IPPROTO_TCP, 0, 0, 0, 0};
    struct Info *r = 0;
    int e;

    e = WSAStartup(0x0202, wsa);
    mira(e == 0, "WSAStartup(2.2) da 0, aunque no haya cable");
    mira(wsa[0] == 2 && wsa[1] == 2 && wsa[2] == 2 && wsa[3] == 2, "WSADATA: wVersion y wHighVersion 2.2");
    mira(igual((const char *)wsa + 16, "WinSock 2.0") && igual((const char *)wsa + 273, "Running"), "WSADATA: \"WinSock 2.0\" y \"Running\"");

    e = getaddrinfo("127.0.0.1", "80", &pista, (void **)&r);
    mira(e == 0 && r != 0, "getaddrinfo(\"127.0.0.1\", \"80\") se contesta sin red");
    mira(r && es_local(r, 80), "su ADDRINFO: AF_INET, TCP, 16 B, 127.0.0.1:80, uno solo");
    if (r)
        freeaddrinfo(r);

    r = (struct Info *)1;
    e = GetAddrInfoW(L"127.0.0.1", L"443", &pista, (void **)&r);
    mira(e == 0 && r != (struct Info *)1 && r && es_local(r, 443), "GetAddrInfoW(L\"127.0.0.1\", L\"443\"): lo mismo, en ancho");
    if (r && r != (struct Info *)1)
        FreeAddrInfoW(r);

    r = (struct Info *)1;
    e = getaddrinfo("127.0.0.1", "no-es-un-puerto", &pista, (void **)&r);
    mira(e == WSATYPE_NOT_FOUND && r == 0, "un servicio que no existe: WSATYPE_NOT_FOUND y NULL");

    r = (struct Info *)1;
    e = getaddrinfo("bmo.invalid", "80", &pista, (void **)&r);
    mira(e != 0 && r == 0, "un nombre que no existe (.invalid): un error y NULL");

    e = WSAStartup(0x0101, wsa);
    mira(e == 0 && wsa[0] == 1 && wsa[1] == 1 && wsa[2] == 2 && wsa[3] == 2, "WSAStartup(1.1) otra vez: 0, usa 1.1 y la alta es 2.2");

    mira(WSACleanup() == 0, "WSACleanup del segundo WSAStartup: 0");
    mira(WSACleanup() == 0, "WSACleanup del primero: 0");
    e = WSACleanup();
    mira(e == -1 && WSAGetLastError() == WSANOTINITIALISED, "WSACleanup de mas: SOCKET_ERROR y WSANOTINITIALISED");

    r = (struct Info *)1;
    e = getaddrinfo("127.0.0.1", "80", &pista, (void **)&r);
    mira(e == WSANOTINITIALISED, "getaddrinfo sin WSAStartup: WSANOTINITIALISED");
    mira(r == (struct Info *)1, "y sin WSAStartup NO toca el puntero (la primera ronda en Windows: no lo pone a NULL)");
    di("tanda30.exe: Winsock arranca sin cable y contesta una IPv4 en numeros\r\n");
    ExitProcess(fallos);
}
