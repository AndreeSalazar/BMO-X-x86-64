/* tanda5.c -- el .exe de la TANDA 5 de Cyberpunk (29-09): user32, grupo 1.
 * Los rectangulos (exactos), las medidas del sistema, el DPI, los monitores
 * y la geometria de una ventana (oculta).
 *
 * Tu pantalla no es la de la casa (la casa tiene una fija): de las medidas
 * se miran las RELACIONES que Windows cumple siempre (el monitor primario
 * mide lo que dice GetSystemMetrics, el area de trabajo cabe dentro, a 192
 * DPI una medida es el doble...). Sale con el numero de fallos. En Windows
 * dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned long long U64;
typedef unsigned short WCHAR;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA HANDLE W GetModuleHandleW(const WCHAR *n);
IMPORTA int W SetRect(int *r, int a, int b, int c, int d);
IMPORTA int W SetRectEmpty(int *r);
IMPORTA int W CopyRect(int *d, const int *s);
IMPORTA int W InflateRect(int *r, int dx, int dy);
IMPORTA int W OffsetRect(int *r, int dx, int dy);
IMPORTA int W IntersectRect(int *d, const int *a, const int *b);
IMPORTA int W UnionRect(int *d, const int *a, const int *b);
IMPORTA int W SubtractRect(int *d, const int *a, const int *b);
IMPORTA int W EqualRect(const int *a, const int *b);
IMPORTA int W IsRectEmpty(const int *r);
IMPORTA int W PtInRect(const int *r, U64 p);
IMPORTA int W GetSystemMetrics(int i);
IMPORTA int W GetSystemMetricsForDpi(int i, unsigned dpi);
IMPORTA int W SystemParametersInfoW(unsigned a, unsigned p, void *v, unsigned f);
IMPORTA unsigned W GetDpiForSystem(void);
IMPORTA unsigned W GetDpiForWindow(HANDLE h);
IMPORTA int W IsProcessDPIAware(void);
IMPORTA void *W GetThreadDpiAwarenessContext(void);
IMPORTA int W AreDpiAwarenessContextsEqual(void *a, void *b);
IMPORTA HANDLE W MonitorFromPoint(U64 p, DWORD f);
IMPORTA HANDLE W MonitorFromWindow(HANDLE h, DWORD f);
IMPORTA int W GetMonitorInfoW(HANDLE m, void *i);
IMPORTA int W EnumDisplayMonitors(HANDLE dc, const int *r, int(W *f)(HANDLE, HANDLE, int *, long long), long long d);
IMPORTA int W EnumDisplaySettingsW(const WCHAR *n, DWORD modo, void *dm);
IMPORTA int W EnumDisplayDevicesW(const WCHAR *n, DWORD i, void *dd, DWORD f);
IMPORTA unsigned short W RegisterClassExW(const void *c);
IMPORTA HANDLE W CreateWindowExW(DWORD ex, const WCHAR *clase, const WCHAR *t, DWORD estilo, int x, int y, int w, int h, HANDLE padre, HANDLE menu, HANDLE inst, void *p);
IMPORTA int W DestroyWindow(HANDLE h);
IMPORTA long long W DefWindowProcW(HANDLE h, unsigned m, U64 w, long long l);
IMPORTA int W AdjustWindowRect(int *r, DWORD estilo, int menu);
IMPORTA int W GetClientRect(HANDLE h, int *r);
IMPORTA int W GetWindowRect(HANDLE h, int *r);
IMPORTA int W ClientToScreen(HANDLE h, int *p);
IMPORTA int W ScreenToClient(HANDLE h, int *p);
IMPORTA HANDLE W GetDesktopWindow(void);

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

static int es(const int *r, int a, int b, int c, int d) {
    return r[0] == a && r[1] == b && r[2] == c && r[3] == d;
}

static int empieza_w(const WCHAR *s, const char *p) {
    while (*p)
        if (*s++ != (WCHAR)*p++)
            return 0;
    return 1;
}

static U64 punto(int x, int y) {
    return (U64)(unsigned)x | (U64)(unsigned)y << 32;
}

static HANDLE primario;
static int monitores, vio_primario;

static int W cada_monitor(HANDLE m, HANDLE dc, int *r, long long d) {
    (void)dc, (void)r, (void)d;
    monitores++;
    if (m == primario)
        vio_primario = 1;
    return 1;
}

static long long W proc(HANDLE h, unsigned m, U64 w, long long l) {
    return DefWindowProcW(h, m, w, l);
}

void inicio(void) {
    int a[4], b[4], c[4];
    int cx = GetSystemMetrics(0), cy = GetSystemMetrics(1);
    /* -- los rectangulos */
    SetRect(a, 1, 2, 11, 12);
    CopyRect(b, a);
    mira(es(a, 1, 2, 11, 12) && EqualRect(a, b) && !IsRectEmpty(a), "SetRect, CopyRect, EqualRect, IsRectEmpty");
    InflateRect(b, 2, -1);
    OffsetRect(b, 10, 20);
    mira(es(b, 9, 23, 23, 31), "InflateRect y OffsetRect");
    SetRect(b, 5, 5, 20, 8);
    mira(IntersectRect(c, a, b) && es(c, 5, 5, 11, 8), "IntersectRect: lo comun");
    SetRect(b, 50, 50, 60, 60);
    mira(!IntersectRect(c, a, b) && es(c, 0, 0, 0, 0) && IsRectEmpty(c), "IntersectRect sin nada comun: vacio y FALSE");
    SetRectEmpty(c);
    mira(UnionRect(c, a, c) && es(c, 1, 2, 11, 12) && UnionRect(c, a, b) && es(c, 1, 2, 60, 60), "UnionRect (un vacio no cuenta)");
    SetRect(a, 0, 0, 10, 10);
    SetRect(b, 0, 0, 10, 4);
    mira(SubtractRect(c, a, b) && es(c, 0, 4, 10, 10), "SubtractRect: le quita una franja entera");
    SetRect(b, 2, 2, 4, 4);
    mira(SubtractRect(c, a, b) && es(c, 0, 0, 10, 10), "SubtractRect de un hueco del medio: se queda entero");
    mira(PtInRect(a, punto(0, 0)) && PtInRect(a, punto(9, 9)) && !PtInRect(a, punto(10, 5)) && !PtInRect(a, punto(5, -1)), "PtInRect: el borde derecho y el de abajo, fuera");
    /* -- las medidas */
    mira(cx > 0 && cy > 0 && GetSystemMetrics(78) >= cx && GetSystemMetrics(79) >= cy && GetSystemMetrics(80) >= 1, "GetSystemMetrics: la pantalla, la virtual y los monitores");
    {
        int m96 = GetSystemMetricsForDpi(2, 96), m192 = GetSystemMetricsForDpi(2, 192);
        mira(m96 == GetSystemMetrics(2) && m96 > 0 && m192 >= 2 * m96 - 1 && m192 <= 2 * m96 + 1, "GetSystemMetricsForDpi: a 192 DPI, el doble");
    }
    SetRectEmpty(a);
    mira(SystemParametersInfoW(0x30, 0, a, 0) && a[0] >= 0 && a[1] >= 0 && a[2] <= cx && a[3] <= cy && a[2] > a[0] && a[3] > a[1], "SystemParametersInfoW(SPI_GETWORKAREA): cabe en la pantalla");
    {
        DWORD pega[2] = {8, 0};
        int raton[3] = {-1, -1, -1};
        mira(SystemParametersInfoW(0x3A, 8, pega, 0) && SystemParametersInfoW(3, 0, raton, 0) && raton[2] >= 0, "SystemParametersInfoW: STICKYKEYS y el raton");
    }
    /* -- el DPI (un .exe sin manifiesto no se entera: 96) */
    mira(!IsProcessDPIAware() && GetDpiForSystem() == 96 && AreDpiAwarenessContextsEqual(GetThreadDpiAwarenessContext(), (void *)(long long)-1), "el DPI de un .exe sin manifiesto: 96, y no se entera");
    /* -- los monitores */
    primario = MonitorFromPoint(punto(0, 0), 1);
    {
        unsigned char i[104] = {0};
        int *m = (int *)(i + 4), *t = (int *)(i + 20);
        *(DWORD *)i = 104;
        mira(primario && GetMonitorInfoW(primario, i) && es(m, 0, 0, cx, cy) && (*(DWORD *)(i + 36) & 1) && es(t, a[0], a[1], a[2], a[3]) && empieza_w((WCHAR *)(i + 40), "\\\\.\\DISPLAY"), "GetMonitorInfoW: el primario mide la pantalla, y su area de trabajo");
    }
    mira(EnumDisplayMonitors(0, 0, cada_monitor, 0) && monitores >= 1 && vio_primario && monitores == GetSystemMetrics(80), "EnumDisplayMonitors: todos, y el primario entre ellos");
    {
        static unsigned char dm[220];
        *(unsigned short *)(dm + 68) = 220;
        mira(EnumDisplaySettingsW(0, 0xFFFFFFFF, dm) && *(DWORD *)(dm + 168) == 32 && *(DWORD *)(dm + 172) > 0 && *(DWORD *)(dm + 176) > 0 && *(DWORD *)(dm + 184) > 0, "EnumDisplaySettingsW: el modo actual, a 32 bits");
    }
    {
        static unsigned char dd[840];
        DWORD k;
        int visto = 0;
        for (k = 0; k < 16 && !visto; k++) {
            *(DWORD *)dd = 840;
            if (!EnumDisplayDevicesW(0, k, dd, 0))
                break;
            if (*(DWORD *)(dd + 324) & 4)
                visto = empieza_w((WCHAR *)(dd + 4), "\\\\.\\DISPLAY");
        }
        mira(visto, "EnumDisplayDevicesW: el adaptador primario");
    }
    /* -- una ventana (oculta), de 320x200 de cliente */
    {
        static unsigned char clase[80];
        HANDLE h;
        int p[2] = {5, 7};
        *(DWORD *)clase = 80;
        *(void **)(clase + 8) = (void *)proc;
        *(HANDLE *)(clase + 24) = GetModuleHandleW(0);
        *(const WCHAR **)(clase + 64) = L"BMO_tanda5";
        RegisterClassExW(clase);
        SetRect(a, 0, 0, 320, 200);
        AdjustWindowRect(a, 0xCF0000, 0);
        h = CreateWindowExW(0, L"BMO_tanda5", L"tanda5", 0xCF0000, 0, 0, a[2] - a[0], a[3] - a[1], 0, 0, GetModuleHandleW(0), 0);
        mira(h && GetClientRect(h, b) && es(b, 0, 0, 320, 200), "AdjustWindowRect y CreateWindowExW: 320x200 de cliente");
        mira(GetWindowRect(h, c) && c[2] - c[0] >= 320 && c[3] - c[1] >= 200, "GetWindowRect: la ventana entera");
        mira(ClientToScreen(h, p) && ScreenToClient(h, p) && p[0] == 5 && p[1] == 7, "ClientToScreen y ScreenToClient, ida y vuelta");
        mira(GetDpiForWindow(h) == 96 && MonitorFromWindow(h, 2) == primario, "GetDpiForWindow y MonitorFromWindow");
        mira(GetClientRect(GetDesktopWindow(), c) && es(c, 0, 0, cx, cy), "GetDesktopWindow: mide la pantalla");
        DestroyWindow(h);
    }
    di("tanda5.exe: las medidas de user32 dicen lo de Windows\r\n");
    ExitProcess(fallos);
}
