/* sombras.c -- el OBRERO de P3c2 de PROTON-X, que corre en WINDOWS: compila
 * el HLSL que un juego pidio en BMO-X, con el d3dcompiler_47 de este Windows.
 *
 *    sombras.exe A:\window\sombras
 *
 * En BMO-X, D3DCompile no tiene compilador: deja en esa carpeta cada pedido
 * como <huella>.hls (la fuente) y <huella>.ent (entrada, perfil, banderas,
 * macros; ver platform/shared/proton-x/src/sombras.rs). Esto compila cada
 * .hls que no tenga su .cso y lo deja al lado; la proxima vez, en BMO-X,
 * D3DCompile devuelve esos bytes. Son los MISMOS que el juego tendria en
 * Windows: el compilador es el mismo.
 *
 * Carga d3dcompiler_47.dll en marcha (LoadLibraryA + GetProcAddress), como
 * el propio Windows lo tiene en System32. Sale con el numero de fallos. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned long long U64;

#define IMPORTA __declspec(dllimport)
#define WINAPI __stdcall
IMPORTA HANDLE WINAPI GetStdHandle(DWORD n);
IMPORTA int WINAPI WriteFile(HANDLE h, const void *b, DWORD n, DWORD *escritos, void *ov);
IMPORTA int WINAPI ReadFile(HANDLE h, void *b, DWORD n, DWORD *leidos, void *ov);
IMPORTA HANDLE WINAPI CreateFileA(const char *n, DWORD acceso, DWORD comp, void *seg, DWORD disp, DWORD band, HANDLE pl);
IMPORTA DWORD WINAPI GetFileSize(HANDLE h, DWORD *alto);
IMPORTA int WINAPI CloseHandle(HANDLE h);
IMPORTA void WINAPI ExitProcess(unsigned int c);
IMPORTA char *WINAPI GetCommandLineA(void);
IMPORTA HANDLE WINAPI FindFirstFileA(const char *patron, void *datos);
IMPORTA int WINAPI FindNextFileA(HANDLE h, void *datos);
IMPORTA int WINAPI FindClose(HANDLE h);
IMPORTA HANDLE WINAPI LoadLibraryA(const char *n);
IMPORTA void *WINAPI GetProcAddress(HANDLE m, const char *n);
IMPORTA void *WINAPI VirtualAlloc(void *a, U64 n, DWORD tipo, DWORD prot);

typedef long(WINAPI *D3DCOMPILE)(const void *src, U64 n, const char *nombre, const void *macros, void *include, const char *entrada, const char *perfil, unsigned f1, unsigned f2, void **codigo, void **errores);
typedef struct { void **vt; } BLOB;
typedef void *(WINAPI *PUNTERO)(void *);
typedef U64(WINAPI *MEDIDA)(void *);

#define INVALIDO ((HANDLE)(long long)-1)
/* WIN32_FIND_DATAA: el nombre en +44 (260 bytes). */
typedef struct { DWORD atr, fechas[6], alto, bajo, r0, r1; char nombre[260]; char corto[14]; } HALLAZGO;

static HANDLE salida;

static void di(const char *s) {
    DWORD n = 0, e;
    while (s[n]) n++;
    WriteFile(salida, s, n, &e, 0);
}

static void di_n(const char *s, U64 n) {
    DWORD e;
    WriteFile(salida, s, (DWORD)n, &e, 0);
}

static int largo(const char *s) {
    int n = 0;
    while (s[n]) n++;
    return n;
}

/* a + b en c (con tope). */
static void junta(char *c, const char *a, const char *b) {
    int i = 0, j = 0;
    while (a[i] && i < 500) c[i] = a[i], i++;
    while (b[j] && i < 510) c[i++] = b[j++];
    c[i] = 0;
}

/* El fichero entero (en memoria del proceso), o 0. */
static char *leer(const char *ruta, DWORD *n) {
    HANDLE h = CreateFileA(ruta, 0x80000000, 1, 0, 3, 0x80, 0);
    char *b;
    DWORD leidos = 0;
    if (h == INVALIDO) return 0;
    *n = GetFileSize(h, 0);
    b = VirtualAlloc(0, *n + 1, 0x3000, 4);
    if (b && ReadFile(h, b, *n, &leidos, 0) && leidos == *n) b[*n] = 0;
    else b = 0;
    CloseHandle(h);
    return b;
}

static int existe(const char *ruta) {
    HANDLE h = CreateFileA(ruta, 0x80000000, 1, 0, 3, 0x80, 0);
    if (h == INVALIDO) return 0;
    CloseHandle(h);
    return 1;
}

static int escribir(const char *ruta, const void *b, U64 n) {
    DWORD e = 0;
    HANDLE h = CreateFileA(ruta, 0x40000000, 0, 0, 2, 0x80, 0);
    int bien;
    if (h == INVALIDO) return 0;
    bien = WriteFile(h, b, (DWORD)n, &e, 0) && e == n;
    return CloseHandle(h) && bien;
}

/* Parte el .ent (en su sitio): entrada, perfil, f1, f2 y las macros (hasta 32). */
static unsigned hexa(const char *s) {
    unsigned v = 0;
    for (; *s; s++) v = v * 16 + (unsigned)(*s <= '9' ? *s - '0' : (*s | 32) - 'a' + 10);
    return v;
}

void inicio(void) {
    char *cl = GetCommandLineA(), dir[512], patron[520], ruta[520];
    HALLAZGO d;
    HANDLE hb, lib;
    D3DCOMPILE compilar;
    unsigned hechos = 0, fallos = 0, ya = 0;
    int i;

    salida = GetStdHandle((DWORD)-11);
    /* La carpeta: lo que va detras del nombre del programa (con o sin comillas). */
    if (*cl == '"') { cl++; while (*cl && *cl != '"') cl++; if (*cl) cl++; }
    else { while (*cl && *cl != ' ') cl++; }
    while (*cl == ' ') cl++;
    for (i = 0; cl[i] && i < 500; i++) dir[i] = cl[i];
    while (i > 0 && (dir[i - 1] == ' ' || dir[i - 1] == '"')) i--;
    if (i == 0) dir[i++] = '.';
    dir[i] = 0;
    if (dir[i - 1] != '\\') { dir[i] = '\\'; dir[i + 1] = 0; }

    lib = LoadLibraryA("d3dcompiler_47.dll");
    compilar = lib ? (D3DCOMPILE)GetProcAddress(lib, "D3DCompile") : 0;
    if (!compilar) {
        di("sombras.exe: no encuentro d3dcompiler_47.dll (viene con Windows 10/11)\r\n");
        ExitProcess(1);
    }
    di("sombras.exe: compilando lo pendiente en ");
    di(dir);
    di("\r\n");

    junta(patron, dir, "*.hls");
    hb = FindFirstFileA(patron, &d);
    if (hb != INVALIDO) {
        do {
            char base[520], *src, *ent, *linea[40], *p;
            DWORD nsrc = 0, nent = 0;
            const char *macros[66];
            int nl = 0, m = 0;
            void *codigo = 0, *errores = 0;
            long r;
            int k = largo(d.nombre) - 4;
            junta(base, dir, d.nombre);
            base[largo(base) - 4] = 0;
            junta(ruta, base, ".cso");
            if (existe(ruta)) { ya++; continue; }
            junta(ruta, base, ".hls");
            src = leer(ruta, &nsrc);
            junta(ruta, base, ".ent");
            ent = leer(ruta, &nent);
            if (!src || !ent) { di("  NO  sin su .ent: "); di(d.nombre); di("\r\n"); fallos++; continue; }
            /* Las lineas del .ent. */
            for (p = ent, linea[nl++] = p; *p && nl < 40; p++)
                if (*p == '\n' || *p == '\r') { *p = 0; if (p[1] && p[1] != '\n') linea[nl++] = p + 1; }
            if (nl < 4) { di("  NO  un .ent roto: "); di(d.nombre); di("\r\n"); fallos++; continue; }
            for (i = 4; i < nl && m < 64; i++) {
                char *igual = linea[i];
                while (*igual && *igual != '=') igual++;
                if (!*igual) continue;
                *igual = 0;
                macros[m++] = linea[i];
                macros[m++] = igual + 1;
            }
            macros[m] = 0;
            macros[m + 1] = 0;
            r = compilar(src, nsrc, d.nombre, m ? macros : 0, (void *)1, linea[0], linea[1], hexa(linea[2]), hexa(linea[3]), &codigo, &errores);
            (void)k;
            if (r == 0 && codigo) {
                junta(ruta, base, ".cso");
                if (escribir(ruta, ((PUNTERO)((BLOB *)codigo)->vt[3])(codigo), ((MEDIDA)((BLOB *)codigo)->vt[4])(codigo))) {
                    di("  bien  ");
                    hechos++;
                } else {
                    di("  NO    (no se pudo escribir el .cso) ");
                    fallos++;
                }
                di(d.nombre);
                di(" -> ");
                di(linea[0]);
                di(" / ");
                di(linea[1]);
                di("\r\n");
            } else {
                di("  NO  ");
                di(d.nombre);
                di(": el HLSL no compila:\r\n");
                if (errores) di_n(((PUNTERO)((BLOB *)errores)->vt[3])(errores), ((MEDIDA)((BLOB *)errores)->vt[4])(errores));
                di("\r\n");
                fallos++;
            }
        } while (FindNextFileA(hb, &d));
        FindClose(hb);
    }
    di("sombras.exe: ");
    {
        char b[64];
        int n = 0;
        unsigned v[3] = {hechos, ya, fallos}, j;
        const char *t[3] = {" compilados, ", " ya estaban, ", " no\r\n"};
        for (j = 0; j < 3; j++) {
            char d2[10];
            int k = 0;
            unsigned x = v[j];
            do { d2[k++] = (char)('0' + x % 10); x /= 10; } while (x);
            while (k) b[n++] = d2[--k];
            b[n] = 0;
            di(b);
            n = 0;
            di(t[j]);
        }
    }
    ExitProcess(fallos);
}
