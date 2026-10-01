/* tanda31.c -- el .exe de la TANDA 31 (01-10): la RED LOCAL, sin cable.
 * Por Cyberpunk: Galaxy (REDGalaxy64.dll, 0x879645 y 0x7c69b0) forma un par
 * de sockets TCP en 127.0.0.1 para despertar su hilo -- socket,
 * SO_REUSEADDR, bind(127.0.0.1:0), getsockname, listen, socket, connect,
 * accept, send y recv de un byte --, y su logger abre un UDP (2, 2, 17) no
 * bloqueante. Aqui se hace lo mismo, mas lo que un Windows sin cable contesta
 * en el bucle local: FIONREAD, recv no bloqueante sin datos, select, el
 * cierre del otro (recv 0), connect a un puerto sin nadie (rechazado) y un
 * datagrama UDP de un socket a otro. Nada sale de la maquina.
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
/* ws2_32, por ordinal como Cyberpunk (tanda31_ws2_32.def) */
IMPORTA int W WSAStartup(unsigned short v, void *datos);
IMPORTA int W WSACleanup(void);
IMPORTA int W WSAGetLastError(void);
IMPORTA U64 W socket(int af, int tipo, int proto);
IMPORTA int W closesocket(U64 s);
IMPORTA int W setsockopt(U64 s, int nivel, int op, const void *v, int n);
IMPORTA int W bind(U64 s, const void *a, int n);
IMPORTA int W getsockname(U64 s, void *a, int *n);
IMPORTA int W listen(U64 s, int cola);
IMPORTA int W connect(U64 s, const void *a, int n);
IMPORTA U64 W accept(U64 s, void *a, int *n);
IMPORTA int W send(U64 s, const void *b, int n, int f);
IMPORTA int W recv(U64 s, void *b, int n, int f);
IMPORTA int W sendto(U64 s, const void *b, int n, int f, const void *a, int an);
IMPORTA int W recvfrom(U64 s, void *b, int n, int f, void *a, int *an);
IMPORTA int W ioctlsocket(U64 s, long cmd, unsigned long *arg);
IMPORTA int W select(int n, void *r, void *w, void *e, const void *plazo);

#define NO_SOCKET ((U64)-1)
#define FIONBIO ((long)0x8004667E)
#define FIONREAD ((long)0x4004667F)
#define WSAEWOULDBLOCK 10035
#define WSAENOTSOCK 10038
#define WSAECONNREFUSED 10061

struct Dir {
    unsigned short familia, puerto;
    unsigned char ip[4];
    unsigned char cero[8];
};
struct Fds {
    unsigned cuenta, relleno;
    U64 s[64];
};

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

static void local(struct Dir *d, unsigned short puerto_red) {
    int i;
    d->familia = 2;
    d->puerto = puerto_red;
    d->ip[0] = 127, d->ip[1] = 0, d->ip[2] = 0, d->ip[3] = 1;
    for (i = 0; i < 8; i++)
        d->cero[i] = 0;
}

static int es_bucle(const struct Dir *d) {
    return d->familia == 2 && d->ip[0] == 127 && d->ip[1] == 0 && d->ip[2] == 0 && d->ip[3] == 1;
}

void inicio(void);

void inicio(void) {
    static unsigned char wsa[408];
    struct Dir d, e;
    struct Fds fds;
    int n, uno = 1;
    unsigned long arg;
    char b[8];
    U64 esc, cli, srv, u1, u2;
    unsigned short puerto;
    static const struct { long s, us; } cero = {0, 0};

    mira(WSAStartup(0x0202, wsa) == 0, "WSAStartup(2.2)");

    /* -- el par TCP de Galaxy (0x879645) -- */
    esc = socket(2, 1, 0);
    mira(esc != NO_SOCKET, "socket(AF_INET, SOCK_STREAM): el que escucha");
    mira(setsockopt(esc, 0xFFFF, 4, (const char *)&uno, 4) == 0, "setsockopt(SO_REUSEADDR)");
    local(&d, 0);
    mira(bind(esc, &d, 16) == 0, "bind(127.0.0.1:0)");
    n = 16;
    mira(getsockname(esc, &d, &n) == 0 && n == 16 && es_bucle(&d) && d.puerto != 0, "getsockname: 127.0.0.1 y un puerto que no es 0");
    puerto = d.puerto;
    mira(listen(esc, 1) == 0, "listen");
    cli = socket(2, 1, 0);
    mira(cli != NO_SOCKET, "socket: el que conecta");
    mira(connect(cli, &d, 16) == 0, "connect a ese puerto (bloqueante): 0");
    n = 16;
    srv = accept(esc, &e, &n);
    mira(srv != NO_SOCKET && n == 16 && es_bucle(&e), "accept: el otro extremo, desde 127.0.0.1");
    b[0] = 'X';
    mira(send(cli, b, 1, 0) == 1, "send de un byte");
    b[0] = 0;
    mira(recv(srv, b, 1, 0) == 1 && b[0] == 'X', "recv: ese byte, el mismo");

    /* -- lo que un bucle local contesta -- */
    mira(send(srv, "abc", 3, 0) == 3, "send de tres bytes al reves");
    arg = 0;
    mira(ioctlsocket(cli, FIONREAD, &arg) == 0 && arg == 3, "ioctlsocket(FIONREAD): 3 esperando");
    mira(recv(cli, b, 8, 0) == 3 && b[0] == 'a' && b[2] == 'c', "recv: los tres");
    arg = 1;
    mira(ioctlsocket(cli, FIONBIO, &arg) == 0, "ioctlsocket(FIONBIO): no bloqueante");
    mira(recv(cli, b, 8, 0) == -1 && WSAGetLastError() == WSAEWOULDBLOCK, "recv sin datos: WSAEWOULDBLOCK");
    send(cli, "z", 1, 0);
    fds.cuenta = 1;
    fds.s[0] = srv;
    mira(select(0, &fds, 0, 0, &cero) == 1 && fds.cuenta == 1 && fds.s[0] == srv, "select: el que tiene datos esta listo");
    recv(srv, b, 1, 0);
    mira(closesocket(cli) == 0, "closesocket del que conecto");
    mira(recv(srv, b, 8, 0) == 0, "recv del otro: 0, se cerro");
    mira(closesocket(srv) == 0 && closesocket(esc) == 0, "closesocket de los otros dos");
    mira(closesocket(esc) == -1 && WSAGetLastError() == WSAENOTSOCK, "closesocket otra vez: WSAENOTSOCK");
    cli = socket(2, 1, 0);
    local(&d, puerto);
    mira(connect(cli, &d, 16) == -1 && WSAGetLastError() == WSAECONNREFUSED, "connect a un puerto sin nadie: WSAECONNREFUSED");
    closesocket(cli);

    /* -- el UDP del logger (0x8116b0) y un datagrama local -- */
    u1 = socket(2, 2, 17);
    mira(u1 != NO_SOCKET, "socket(2, 2, 17): el UDP del logger");
    local(&d, 0);
    n = 16;
    mira(bind(u1, &d, 16) == 0 && getsockname(u1, &d, &n) == 0 && d.puerto != 0, "bind UDP a 127.0.0.1:0 y su puerto");
    u2 = socket(2, 2, 17);
    arg = 1;
    mira(ioctlsocket(u2, FIONBIO, &arg) == 0, "ioctlsocket(FIONBIO) en UDP, como el logger");
    mira(sendto(u2, "hola", 4, 0, &d, 16) == 4, "sendto de 4 bytes al otro UDP");
    n = 16;
    mira(recvfrom(u1, b, 8, 0, &e, &n) == 4 && b[0] == 'h' && b[3] == 'a' && es_bucle(&e), "recvfrom: \"hola\", desde 127.0.0.1");
    n = 16;
    mira(recvfrom(u2, b, 8, 0, &e, &n) == -1 && WSAGetLastError() == WSAEWOULDBLOCK, "recvfrom sin nada, no bloqueante: WSAEWOULDBLOCK");
    mira(closesocket(u1) == 0 && closesocket(u2) == 0, "closesocket de los UDP");
    mira(WSACleanup() == 0, "WSACleanup");
    di("tanda31.exe: la red local de Galaxy, sin cable, como en Windows\r\n");
    ExitProcess(fallos);
}
