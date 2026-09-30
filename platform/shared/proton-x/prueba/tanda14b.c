/* tanda14b.c -- el .exe de la TANDA 14b de Cyberpunk (30-09): VERSION y la
 * seguridad de los ficheros (DURAS del censo).
 *
 * Lee SU PROPIO recurso de version (tanda14b.rc): la version, la traduccion
 * y un texto, como hace un juego con sus DLL. Y la seguridad de un fichero
 * que crea: GetFileSecurityW, ImpersonateSelf, OpenThreadToken, AccessCheck
 * y RevertToSelf (sus permisos de verdad no importan: se mira que se pueda
 * LEER un fichero que uno mismo acaba de crear). Deja tanda14b.txt en su
 * carpeta. Sale con el numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned short WCHAR;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA DWORD W GetModuleFileNameA(HANDLE m, char *b, DWORD n);
IMPORTA DWORD W GetLastError(void);
IMPORTA void W SetLastError(DWORD e);
IMPORTA HANDLE W CreateFileW(const WCHAR *n, DWORD a, DWORD c, void *s, DWORD d, DWORD f, HANDLE t);
IMPORTA int W CloseHandle(HANDLE h);
IMPORTA HANDLE W GetCurrentThread(void);
IMPORTA DWORD W GetFileVersionInfoSizeA(const char *n, DWORD *h);
IMPORTA int W GetFileVersionInfoA(const char *n, DWORD h, DWORD largo, void *datos);
IMPORTA int W VerQueryValueA(const void *b, const char *c, void **p, unsigned *l);
IMPORTA int W GetFileSecurityW(const WCHAR *n, DWORD que, void *sd, DWORD largo, DWORD *falta);
IMPORTA int W ImpersonateSelf(int nivel);
IMPORTA int W RevertToSelf(void);
IMPORTA int W OpenThreadToken(HANDLE h, DWORD acceso, int propio, HANDLE *t);
IMPORTA int W AccessCheck(void *sd, HANDLE t, DWORD deseado, void *mapa, void *privs, DWORD *largo, DWORD *concedido, int *estado);

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
    while (*b)
        if (*a++ != *b++)
            return 0;
    return *a == 0;
}

static char ruta[300];
static unsigned char bloque[8192];
static unsigned char sd[4096];

void inicio(void) {
    DWORD h = 7, n;
    void *p = 0;
    unsigned l = 0;
    /* -- VERSION, de si mismo */
    GetModuleFileNameA(0, ruta, 300);
    n = GetFileVersionInfoSizeA(ruta, &h);
    mira(n > 0 && n <= sizeof bloque, "GetFileVersionInfoSizeA de si mismo: tiene recurso de version");
    mira(GetFileVersionInfoA(ruta, 0, n, bloque), "GetFileVersionInfoA");
    {
        DWORD *f;
        int bien = VerQueryValueA(bloque, "\\", &p, &l) && l == 52;
        f = (DWORD *)p;
        mira(bien && f[0] == 0xFEEF04BD && f[2] == 0x00010002 && f[3] == 0x00030004 && f[4] == 0x00010002 && f[5] == 0, "VerQueryValueA(\"\\\"): VS_FIXEDFILEINFO, la version 1.2.3.4");
    }
    {
        unsigned short *t;
        int bien = VerQueryValueA(bloque, "\\VarFileInfo\\Translation", &p, &l) && l == 4;
        t = (unsigned short *)p;
        mira(bien && t[0] == 0x409 && t[1] == 1200, "VerQueryValueA(\"\\VarFileInfo\\Translation\"): 0409, 1200");
    }
    mira(VerQueryValueA(bloque, "\\StringFileInfo\\040904b0\\ProductName", &p, &l) && l >= 12 && igual((const char *)p, "BMO tanda14b"), "VerQueryValueA de un texto: ProductName, en bytes");
    mira(!VerQueryValueA(bloque, "\\StringFileInfo\\040904b0\\NoEsta", &p, &l), "VerQueryValueA de lo que no hay: FALSE");
    SetLastError(0);
    mira(GetFileVersionInfoSizeA("no_esta_tanda14b.dll", &h) == 0 && GetLastError() == 2, "GetFileVersionInfoSizeA de un fichero que no hay: ERROR_FILE_NOT_FOUND");
    /* -- la seguridad */
    {
        HANDLE f = CreateFileW(L"tanda14b.txt", 0x40000000, 0, 0, 2, 0x80, 0);
        DWORD falta = 0;
        CloseHandle(f);
        mira(!GetFileSecurityW(L"tanda14b.txt", 7, 0, 0, &falta) && GetLastError() == 122 && falta > 0 && falta <= sizeof sd, "GetFileSecurityW sin bufer: ERROR_INSUFFICIENT_BUFFER y cuanto hace falta");
        mira(GetFileSecurityW(L"tanda14b.txt", 7, sd, falta, &falta), "GetFileSecurityW: el descriptor (propietario, grupo y DACL)");
    }
    {
        HANDLE t = 0;
        mira(!OpenThreadToken(GetCurrentThread(), 0xE, 1, &t) && GetLastError() == 1008, "OpenThreadToken sin hacerse pasar por nadie: ERROR_NO_TOKEN");
    }
    {
        HANDLE t = 0;
        DWORD mapa[4] = {0x120089, 0x120116, 0x1200A0, 0x1F01FF}, concedido = 0, largo = 64;
        unsigned char privs[64];
        int estado = 0;
        int bien = ImpersonateSelf(2) && OpenThreadToken(GetCurrentThread(), 0xE, 1, &t);
        bien = bien && AccessCheck(sd, t, 0x120089, mapa, privs, &largo, &concedido, &estado) && estado && (concedido & 0x120089) == 0x120089;
        mira(bien, "ImpersonateSelf, OpenThreadToken y AccessCheck: se puede leer lo que uno crea");
        mira(RevertToSelf() && !OpenThreadToken(GetCurrentThread(), 0xE, 1, &t), "RevertToSelf: el hilo vuelve a ser el del proceso");
        CloseHandle(t);
    }
    di("tanda14b.exe: VERSION y la seguridad dicen lo de Windows\r\n");
    ExitProcess(fallos);
}
