/* tanda3.c -- el .exe de la TANDA 3 de Cyberpunk (29-09): kernel32, pasos 1
 * a 3. La hora (SYSTEMTIME <-> FILETIME), MulDiv, Encode/DecodePointer; lo
 * que dice del sistema (memoria, version, locale, GetCPInfo,
 * GetStringTypeW); y las "A": el entorno, rutas, buscar ficheros, modulos,
 * CreateEventExA, WriteConsoleA.
 *
 * Sale con el numero de fallos. En Windows dice lo mismo (salvo la memoria
 * y el locale de esa maquina, que aqui solo se miran por encima). */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned short WORD;
typedef unsigned long long U64;
typedef unsigned short WCHAR;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA DWORD W GetLastError(void);
IMPORTA HANDLE W CreateFileA(const char *n, DWORD a, DWORD c, void *s, DWORD d, DWORD f, HANDLE t);
IMPORTA int W CloseHandle(HANDLE h);
IMPORTA int W FindClose(HANDLE h);
IMPORTA int W SetEnvironmentVariableA(const char *n, const char *v);
IMPORTA DWORD W GetEnvironmentVariableA(const char *n, char *b, DWORD k);
IMPORTA char *W GetEnvironmentStrings(void);
IMPORTA int W FreeEnvironmentStringsA(char *p);
IMPORTA DWORD W ExpandEnvironmentStringsA(const char *s, char *d, DWORD n);
IMPORTA DWORD W ExpandEnvironmentStringsW(const WCHAR *s, WCHAR *d, DWORD n);
IMPORTA DWORD W GetFileAttributesA(const char *n);
IMPORTA int W CreateDirectoryA(const char *n, void *s);
IMPORTA int W DeleteFileA(const char *n);
IMPORTA DWORD W GetFullPathNameA(const char *n, DWORD k, char *b, char **p);
IMPORTA DWORD W GetTempPathA(DWORD k, char *b);
IMPORTA HANDLE W FindFirstFileExA(const char *n, int nivel, void *d, int op, void *f, DWORD b);
IMPORTA int W FindNextFileA(HANDLE h, void *d);
IMPORTA int W GetModuleHandleExA(DWORD f, const char *n, HANDLE *h);
IMPORTA DWORD W K32GetModuleBaseNameA(HANDLE p, HANDLE m, char *b, DWORD n);
IMPORTA int W QueryFullProcessImageNameA(HANDLE p, DWORD f, char *b, DWORD *n);
IMPORTA HANDLE W CreateEventExA(void *a, const char *n, DWORD f, DWORD acc);
IMPORTA int W SetEvent(HANDLE h);
IMPORTA DWORD W WaitForSingleObject(HANDLE h, DWORD ms);
IMPORTA int W WriteConsoleA(HANDLE h, const void *b, DWORD n, DWORD *e, void *r);
IMPORTA void W GetSystemTime(WORD *st);
IMPORTA int W SystemTimeToFileTime(const WORD *st, U64 *ft);
IMPORTA int W FileTimeToSystemTime(const U64 *ft, WORD *st);
IMPORTA int W MulDiv(int a, int b, int c);
IMPORTA void *W EncodePointer(void *p);
IMPORTA void *W DecodePointer(void *p);
IMPORTA int W GlobalMemoryStatusEx(void *m);
IMPORTA int W GetVersionExW(void *v);
IMPORTA int W VerifyVersionInfoW(void *v, DWORD t, U64 m);
IMPORTA U64 W VerSetConditionMask(U64 m, DWORD t, unsigned char c);
IMPORTA int W GetUserDefaultLocaleName(WCHAR *b, int n);
IMPORTA int W GetCPInfo(unsigned cp, void *i);
IMPORTA int W GetStringTypeW(DWORD t, const WCHAR *s, int n, WORD *o);

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

static int acaba(const char *a, const char *fin) {
    const char *p = a, *q = fin;
    while (*p)
        p++;
    while (*q)
        q++;
    while (q > fin && p > a && *(p - 1) == *(q - 1))
        p--, q--;
    return q == fin;
}

static int igual_w(const WCHAR *a, const WCHAR *b) {
    while (*a && *a == *b)
        a++, b++;
    return *a == *b;
}

void inicio(void) {
    char b[300], *parte = 0;
    WCHAR wb[64];
    DWORD n;
    U64 ft = 0;
    unsigned char datos[320];

    /* -- paso 1: la hora y lo sencillo */
    {
        WORD st[8], st2[8];
        WORD fija[8] = {2026, 9, 0, 29, 15, 4, 5, 250};
        GetSystemTime(st);
        mira(st[0] >= 2024 && st[1] >= 1 && st[1] <= 12 && st[3] >= 1, "GetSystemTime");
        mira(SystemTimeToFileTime(fija, &ft) && FileTimeToSystemTime(&ft, st2) && st2[0] == 2026 && st2[2] == 2 && st2[3] == 29 && st2[7] == 250, "SYSTEMTIME y FILETIME, ida y vuelta (y martes)");
        fija[1] = 13;
        mira(!SystemTimeToFileTime(fija, &ft) && GetLastError() == 87, "SystemTimeToFileTime con el mes 13: ERROR_INVALID_PARAMETER");
        mira(MulDiv(10, 3, 4) == 8 && MulDiv(1, 1, 0) == -1 && DecodePointer(EncodePointer((void *)0x1234)) == (void *)0x1234, "MulDiv, Encode/DecodePointer");
    }
    /* -- paso 2: el sistema */
    {
        static U64 m[8];
        static DWORD v[71];
        U64 mask = VerSetConditionMask(0, 2, 3);
        unsigned char cp[20];
        WORD t[2];
        WCHAR s[2] = {'A', '7'};
        m[0] = 64;
        v[0] = 284;
        mira(GlobalMemoryStatusEx(m) && m[1] > 0 && m[2] <= m[1], "GlobalMemoryStatusEx");
        /* Windows da 6.2 a un .exe SIN manifiesto (este); la casa, 10 (lo que ve
         * un juego con el suyo). */
        mira(GetVersionExW(v) && (v[1] == 10 || (v[1] == 6 && v[2] == 2)) && v[4] == 2, "GetVersionExW: 10, o 6.2 sin manifiesto");
        v[1] = 6;
        mira(VerifyVersionInfoW(v, 2, mask), "VerifyVersionInfoW: 10 >= 6");
        /* El de la maquina: en-US en la casa, el tuyo en tu Windows. */
        n = GetUserDefaultLocaleName(wb, 64);
        mira(n >= 3 && (wb[2] == '-' || wb[3] == '-'), "GetUserDefaultLocaleName: idioma-PAIS");
        mira(GetCPInfo(65001, cp) && cp[0] == 4 && GetStringTypeW(1, s, 2, t) && (t[0] & 0x101) == 0x101 && (t[1] & 4), "GetCPInfo y GetStringTypeW");
    }
    /* -- paso 3: las "A" */
    mira(SetEnvironmentVariableA("BMO_TANDA3", "cyber") && GetEnvironmentVariableA("BMO_TANDA3", b, 300) == 5 && igual(b, "cyber"), "Set/GetEnvironmentVariableA");
    mira(GetEnvironmentVariableA("BMO_TANDA3", b, 3) == 6 && GetEnvironmentVariableA("NO_HAY_TAL", b, 300) == 0 && GetLastError() == 203, "GetEnvironmentVariableA: no cabe, y no esta");
    {
        char *e = GetEnvironmentStrings(), *p = e;
        int hay = 0;
        while (*p) {
            if (igual(p, "BMO_TANDA3=cyber"))
                hay = 1;
            while (*p)
                p++;
            p++;
        }
        mira(e && hay && FreeEnvironmentStringsA(e), "GetEnvironmentStrings (A) y FreeEnvironmentStringsA");
    }
    n = ExpandEnvironmentStringsA("[%BMO_TANDA3%] %NO_HAY%", b, 300);
    mira(n == 17 && igual(b, "[cyber] %NO_HAY%"), "ExpandEnvironmentStringsA");
    /* La A que no cabe: Windows puede pedir MAS de lo justo (6). */
    mira(ExpandEnvironmentStringsW(L"%BMO_TANDA3%!", wb, 64) == 7 && igual_w(wb, L"cyber!"), "ExpandEnvironmentStringsW");
    mira(ExpandEnvironmentStringsA("%BMO_TANDA3%", b, 2) >= 6, "ExpandEnvironmentStringsA que no cabe: pide al menos lo justo");
    {
        HANDLE f = CreateFileA("tanda3_a.txt", 0x40000000, 0, 0, 2, 0x80, 0);
        HANDLE g = CreateFileA("tanda3_b.txt", 0x40000000, 0, 0, 2, 0x80, 0);
        HANDLE h;
        int vistos = 0;
        CloseHandle(f);
        CloseHandle(g);
        h = FindFirstFileExA("tanda3_*.txt", 0, datos, 0, 0, 0);
        if (h != (HANDLE)-1) {
            do
                vistos += igual((char *)datos + 44, "tanda3_a.txt") || igual((char *)datos + 44, "tanda3_b.txt");
            while (FindNextFileA(h, datos));
            FindClose(h);
        }
        mira(vistos == 2 && GetFileAttributesA("tanda3_b.txt") != 0xFFFFFFFF, "FindFirstFileExA, FindNextFileA y GetFileAttributesA");
        /* El FAT32 de BMO-X no crea carpetas ni borra desde Ring 3 todavia:
         * lo que se mira es lo que NO necesita hacerlo. */
        mira(!DeleteFileA("tanda3_no_esta.txt") && GetLastError() == 2, "DeleteFileA de uno que no esta: ERROR_FILE_NOT_FOUND");
        mira(!CreateDirectoryA("tanda3_a.txt", 0) && GetLastError() == 183, "CreateDirectoryA sobre algo que esta: ERROR_ALREADY_EXISTS");
    }
    n = GetFullPathNameA("tanda3_b.txt", 300, b, &parte);
    mira(n > 10 && b[1] == ':' && acaba(b, "\\tanda3_b.txt") && parte && igual(parte, "tanda3_b.txt"), "GetFullPathNameA con su parte");
    mira(GetFullPathNameA("tanda3_b.txt", 4, b, 0) == n + 1, "GetFullPathNameA que no cabe: lo que hace falta");
    mira(GetTempPathA(300, b) > 0 && acaba(b, "\\"), "GetTempPathA");
    {
        HANDLE m = 0;
        DWORD k = 300;
        mira(GetModuleHandleExA(0, 0, &m) && m && K32GetModuleBaseNameA((HANDLE)-1, 0, b, 300) == 10 && igual(b, "tanda3.exe"), "GetModuleHandleExA y K32GetModuleBaseNameA");
        mira(QueryFullProcessImageNameA((HANDLE)-1, 0, b, &k) && acaba(b, "tanda3.exe") && k > 10, "QueryFullProcessImageNameA");
    }
    {
        HANDLE e = CreateEventExA(0, 0, 1, 0x1F0003);
        mira(e && WaitForSingleObject(e, 0) == 0x102 && SetEvent(e) && WaitForSingleObject(e, 0) == 0 && WaitForSingleObject(e, 0) == 0, "CreateEventExA: manual, y se queda puesto");
        CloseHandle(e);
    }
    {
        DWORD k = 0;
        mira(WriteConsoleA(GetStdHandle((DWORD)-11), "  (WriteConsoleA)\r\n", 19, &k, 0) && k == 19, "WriteConsoleA");
    }
    di("tanda3.exe: kernel32 dice lo de Windows\r\n");
    ExitProcess(fallos);
}
