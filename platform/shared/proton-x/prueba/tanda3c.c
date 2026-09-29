/* tanda3c.c -- el .exe de la TANDA 3 de Cyberpunk, paso 4b (29-09): el
 * mapeo de ficheros (del fichero de paginacion y de un fichero), los
 * puertos de finalizacion, los Open* por nombre, OpenProcess/OpenThread y
 * ReadProcessMemory del propio proceso, WaitForMultipleObjectsEx,
 * GetOverlappedResultEx, CancelIoEx y FormatMessageA.
 *
 * Deja tanda3c.txt en su carpeta. Sale con el numero de fallos. En Windows
 * dice lo mismo (el texto de FormatMessageA, en el idioma de la maquina:
 * solo se mira que lo haya). */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned long long U64;
typedef unsigned short WCHAR;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA int W ReadFile(HANDLE h, void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA DWORD W GetLastError(void);
IMPORTA HANDLE W CreateFileA(const char *n, DWORD a, DWORD c, void *s, DWORD d, DWORD f, HANDLE t);
IMPORTA int W SetFilePointerEx(HANDLE h, long long d, long long *n, DWORD m);
IMPORTA int W CloseHandle(HANDLE h);
IMPORTA HANDLE W CreateEventW(void *a, int manual, int inicial, const void *n);
IMPORTA int W SetEvent(HANDLE h);
IMPORTA DWORD W WaitForSingleObject(HANDLE h, DWORD ms);
IMPORTA HANDLE W CreateFileMappingW(HANDLE f, void *a, DWORD prot, DWORD alto, DWORD bajo, const WCHAR *n);
IMPORTA HANDLE W CreateFileMappingA(HANDLE f, void *a, DWORD prot, DWORD alto, DWORD bajo, const char *n);
IMPORTA HANDLE W OpenFileMappingW(DWORD acc, int her, const WCHAR *n);
IMPORTA void *W MapViewOfFile(HANDLE h, DWORD acc, DWORD alto, DWORD bajo, U64 n);
IMPORTA int W UnmapViewOfFile(const void *p);
IMPORTA int W FlushViewOfFile(const void *p, U64 n);
IMPORTA HANDLE W CreateIoCompletionPort(HANDLE f, HANDLE p, U64 clave, DWORD hilos);
IMPORTA int W PostQueuedCompletionStatus(HANDLE p, DWORD n, U64 clave, void *ov);
IMPORTA int W GetQueuedCompletionStatus(HANDLE p, DWORD *n, U64 *clave, void **ov, DWORD ms);
IMPORTA HANDLE W OpenEventW(DWORD acc, int her, const WCHAR *n);
IMPORTA HANDLE W OpenMutexW(DWORD acc, int her, const WCHAR *n);
IMPORTA HANDLE W OpenSemaphoreW(DWORD acc, int her, const WCHAR *n);
IMPORTA HANDLE W CreateSemaphoreExW(void *a, long ini, long max, const WCHAR *n, DWORD f, DWORD acc);
IMPORTA DWORD W GetCurrentProcessId(void);
IMPORTA DWORD W GetCurrentThreadId(void);
IMPORTA HANDLE W GetCurrentProcess(void);
IMPORTA HANDLE W OpenProcess(DWORD acc, int her, DWORD pid);
IMPORTA HANDLE W OpenThread(DWORD acc, int her, DWORD tid);
IMPORTA int W ReadProcessMemory(HANDLE p, const void *d, void *b, U64 n, U64 *leidos);
IMPORTA DWORD W WaitForMultipleObjectsEx(DWORD n, const HANDLE *h, int todos, DWORD ms, int alerta);
IMPORTA int W GetOverlappedResultEx(HANDLE h, void *ov, DWORD *n, DWORD ms, int alerta);
IMPORTA int W CancelIoEx(HANDLE h, void *ov);
IMPORTA DWORD W FormatMessageA(DWORD f, const void *fuente, DWORD id, DWORD idioma, char *b, DWORD n, void *args);
IMPORTA void *W LocalFree(void *p);

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

static int igual(const char *a, const char *b, int n) {
    int k;
    for (k = 0; k < n; k++)
        if (a[k] != b[k])
            return 0;
    return 1;
}

static U64 secreto = 0x0123456789ABCDEFull;

#define INVALIDO ((HANDLE)(long long)-1)

void inicio(void) {
    /* -- el mapeo del fichero de paginacion */
    {
        HANDLE m = CreateFileMappingW(INVALIDO, 0, 4, 0, 65536, 0);
        char *a = (char *)MapViewOfFile(m, 0xF001F, 0, 0, 0);
        char *b = (char *)MapViewOfFile(m, 4, 0, 0, 4096);
        int vacio = a && a[100] == 0 && a[65535] == 0;
        if (a)
            a[100] = 'x';
        mira(m && a && b && vacio && b[100] == 'x', "CreateFileMappingW del fichero de paginacion: dos vistas, la misma memoria, a cero");
        mira(!MapViewOfFile(m, 4, 0, 4096, 0) && GetLastError() == 1132, "MapViewOfFile fuera de los 64 KiB: ERROR_MAPPED_ALIGNMENT");
        mira(UnmapViewOfFile(a) && UnmapViewOfFile(b) && !UnmapViewOfFile(b), "UnmapViewOfFile (y otra vez: no)");
        CloseHandle(m);
    }
    /* -- el mapeo de un fichero */
    {
        HANDLE f = CreateFileA("tanda3c.txt", 0xC0000000, 0, 0, 2, 0x80, 0);
        DWORD k = 0;
        char buf[16] = {0};
        HANDLE m;
        char *v;
        WriteFile(f, "hola mapeo", 10, &k, 0);
        m = CreateFileMappingA(f, 0, 4, 0, 0, 0);
        v = m ? (char *)MapViewOfFile(m, 0xF001F, 0, 0, 0) : 0;
        mira(v && igual(v, "hola mapeo", 10), "CreateFileMappingA de un fichero: la vista trae lo que tiene");
        if (v)
            v[0] = 'H';
        mira(v && FlushViewOfFile(v, 0) && UnmapViewOfFile(v) && CloseHandle(m), "FlushViewOfFile, UnmapViewOfFile y CloseHandle");
        SetFilePointerEx(f, 0, 0, 0);
        mira(ReadFile(f, buf, 16, &k, 0) && k == 10 && igual(buf, "Hola mapeo", 10), "lo escrito en la vista esta en el fichero");
        {
            unsigned char ov[32] = {0};
            DWORD n = 0;
            ov[8] = 12; /* InternalHigh: los bytes; Internal 0: acabado */
            mira(GetOverlappedResultEx(f, ov, &n, 0, 0) && n == 12, "GetOverlappedResultEx de una E/S acabada");
        }
        mira(!CancelIoEx(f, 0) && GetLastError() == 1168, "CancelIoEx sin nada pendiente: ERROR_NOT_FOUND");
        CloseHandle(f);
    }
    /* -- los nombres que no hay */
    {
        int bien = !OpenFileMappingW(4, 0, L"BMO_tanda3c_no_esta") && GetLastError() == 2;
        bien = bien && !OpenEventW(0x1F0003, 0, L"BMO_tanda3c_no_esta") && GetLastError() == 2;
        bien = bien && !OpenMutexW(0x1F0001, 0, L"BMO_tanda3c_no_esta") && GetLastError() == 2;
        bien = bien && !OpenSemaphoreW(0x1F0003, 0, L"BMO_tanda3c_no_esta") && GetLastError() == 2;
        mira(bien, "OpenFileMappingW, OpenEventW, OpenMutexW y OpenSemaphoreW de un nombre que no hay: ERROR_FILE_NOT_FOUND");
    }
    /* -- un puerto de finalizacion */
    {
        HANDLE p = CreateIoCompletionPort(INVALIDO, 0, 0, 0);
        DWORD n = 0;
        U64 clave = 0;
        void *ov = (void *)1;
        int bien = p && PostQueuedCompletionStatus(p, 5, 77, (void *)&secreto) && PostQueuedCompletionStatus(p, 6, 78, 0);
        bien = bien && GetQueuedCompletionStatus(p, &n, &clave, &ov, 0) && n == 5 && clave == 77 && ov == (void *)&secreto;
        bien = bien && GetQueuedCompletionStatus(p, &n, &clave, &ov, 100) && n == 6 && clave == 78 && ov == 0;
        mira(bien, "CreateIoCompletionPort, PostQueuedCompletionStatus y GetQueuedCompletionStatus: en su orden");
        ov = (void *)1;
        mira(!GetQueuedCompletionStatus(p, &n, &clave, &ov, 0) && GetLastError() == 258 && ov == 0, "GetQueuedCompletionStatus vacio: WAIT_TIMEOUT");
        CloseHandle(p);
    }
    /* -- lo demas */
    {
        HANDLE s = CreateSemaphoreExW(0, 1, 2, 0, 0, 0x1F0003);
        mira(s && WaitForSingleObject(s, 0) == 0 && WaitForSingleObject(s, 0) == 0x102, "CreateSemaphoreExW");
        CloseHandle(s);
    }
    {
        HANDLE p = OpenProcess(0x1000, 0, GetCurrentProcessId());
        HANDLE t = OpenThread(0x800, 0, GetCurrentThreadId());
        U64 v = 0, leidos = 0;
        mira(p && t && ReadProcessMemory(GetCurrentProcess(), &secreto, &v, 8, &leidos) && leidos == 8 && v == secreto, "OpenProcess, OpenThread y ReadProcessMemory del propio proceso");
        CloseHandle(p);
        CloseHandle(t);
    }
    {
        HANDLE e[2];
        e[0] = CreateEventW(0, 1, 0, 0);
        e[1] = CreateEventW(0, 1, 0, 0);
        SetEvent(e[1]);
        mira(WaitForMultipleObjectsEx(2, e, 0, 0, 0) == 1 && WaitForMultipleObjectsEx(2, e, 1, 0, 0) == 0x102, "WaitForMultipleObjectsEx: uno si, los dos no");
        CloseHandle(e[0]);
        CloseHandle(e[1]);
    }
    {
        char *texto = 0;
        char chico[4];
        DWORD n = FormatMessageA(0x1300, 0, 2, 0, (char *)&texto, 0, 0);
        mira(n > 5 && texto && texto[n - 2] == '\r' && texto[n - 1] == '\n' && !LocalFree(texto), "FormatMessageA con ALLOCATE_BUFFER: el texto del error 2");
        mira(FormatMessageA(0x1200, 0, 2, 0, chico, 4, 0) == 0 && GetLastError() == 122, "FormatMessageA que no cabe: ERROR_INSUFFICIENT_BUFFER");
    }
    di("tanda3c.exe: el mapeo y los puertos dicen lo de Windows\r\n");
    ExitProcess(fallos);
}
