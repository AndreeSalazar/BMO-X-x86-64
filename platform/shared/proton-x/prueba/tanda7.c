/* tanda7.c -- el .exe de la TANDA 7 de Cyberpunk (29-09): user32, grupo 3.
 * El teclado (el estado del hilo, las tablas de teclas, lo que escribe una
 * tecla), el cursor, la captura, el raw input (registrar y la lista de
 * dispositivos) y el portapapeles (con GlobalAlloc).
 *
 * Solo lo que Windows contesta igual en cualquier maquina: nada de pulsar
 * teclas de verdad. El cursor se pone donde ya estaba. El portapapeles se
 * usa con "BMO tanda7" y al final se le devuelve el texto que tenia (el
 * historial de Windows, Win+V, se queda con una linea mas). Sale con el
 * numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned long long U64;
typedef long long I64;
typedef unsigned short WCHAR;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA HANDLE W GetModuleHandleW(const WCHAR *n);
IMPORTA DWORD W GetLastError(void);
IMPORTA void W SetLastError(DWORD e);
IMPORTA HANDLE W GlobalAlloc(unsigned f, U64 n);
IMPORTA void *W GlobalLock(HANDLE h);
IMPORTA int W GlobalUnlock(HANDLE h);
IMPORTA U64 W GlobalSize(HANDLE h);
IMPORTA unsigned short W RegisterClassExW(const void *c);
IMPORTA HANDLE W CreateWindowExW(DWORD ex, const WCHAR *clase, const WCHAR *t, DWORD estilo, int x, int y, int w, int h, HANDLE padre, HANDLE menu, HANDLE inst, void *p);
IMPORTA int W DestroyWindow(HANDLE h);
IMPORTA I64 W DefWindowProcW(HANDLE h, unsigned m, U64 w, I64 l);
IMPORTA int W GetSystemMetrics(int i);
IMPORTA short W GetKeyState(int vk);
IMPORTA short W GetAsyncKeyState(int vk);
IMPORTA int W GetKeyboardState(unsigned char *t);
IMPORTA int W SetKeyboardState(unsigned char *t);
IMPORTA unsigned W MapVirtualKeyW(unsigned c, unsigned tipo);
IMPORTA short W VkKeyScanW(WCHAR c);
IMPORTA int W ToUnicode(unsigned vk, unsigned sc, const unsigned char *t, WCHAR *b, int n, unsigned f);
IMPORTA int W GetKeyNameTextW(long l, WCHAR *b, int n);
IMPORTA HANDLE W GetKeyboardLayout(DWORD hilo);
IMPORTA int W GetKeyboardLayoutNameW(WCHAR *b);
IMPORTA int W GetKeyboardType(int que);
IMPORTA int W GetCursorPos(int *p);
IMPORTA int W SetCursorPos(int x, int y);
IMPORTA int W GetCursorInfo(void *ci);
IMPORTA int W ShowCursor(int si);
IMPORTA HANDLE W LoadCursorW(HANDLE inst, const WCHAR *id);
IMPORTA HANDLE W LoadIconW(HANDLE inst, const WCHAR *id);
IMPORTA int W ClipCursor(const int *r);
IMPORTA int W GetClipCursor(int *r);
IMPORTA HANDLE W SetCapture(HANDLE h);
IMPORTA HANDLE W GetCapture(void);
IMPORTA int W ReleaseCapture(void);
IMPORTA unsigned W GetDoubleClickTime(void);
IMPORTA int W RegisterRawInputDevices(const void *d, unsigned n, unsigned medida);
IMPORTA unsigned W GetRegisteredRawInputDevices(void *d, unsigned *n, unsigned medida);
IMPORTA unsigned W GetRawInputDeviceList(void *d, unsigned *n, unsigned medida);
IMPORTA unsigned W GetRawInputDeviceInfoW(HANDLE d, unsigned que, void *datos, unsigned *medida);
IMPORTA unsigned W GetRawInputData(HANDLE h, unsigned que, void *datos, unsigned *medida, unsigned cabecera);
IMPORTA int W OpenClipboard(HANDLE h);
IMPORTA int W CloseClipboard(void);
IMPORTA int W EmptyClipboard(void);
IMPORTA HANDLE W SetClipboardData(unsigned f, HANDLE h);
IMPORTA HANDLE W GetClipboardData(unsigned f);
IMPORTA int W IsClipboardFormatAvailable(unsigned f);
IMPORTA DWORD W GetClipboardSequenceNumber(void);
IMPORTA unsigned W RegisterClipboardFormatW(const WCHAR *n);
IMPORTA int W GetClipboardFormatNameW(unsigned f, WCHAR *b, int n);

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

static int igual_w(const WCHAR *a, const char *b) {
    while (*b)
        if (*a++ != (WCHAR)*b++)
            return 0;
    return *a == 0;
}

static int igual_a(const char *a, const char *b) {
    while (*b)
        if (*a++ != *b++)
            return 0;
    return *a == 0;
}

static I64 W proc(HANDLE h, unsigned m, U64 w, I64 l) {
    return DefWindowProcW(h, m, w, l);
}

static int cerca(int a, int b) {
    return a - b <= 1 && b - a <= 1;
}

/* Un HGLOBAL con este texto (y su cero). */
static HANDLE global_de(const WCHAR *t, int n) {
    HANDLE g = GlobalAlloc(2, (U64)(n + 1) * 2);
    WCHAR *p = g ? (WCHAR *)GlobalLock(g) : 0;
    int k;
    if (!p)
        return 0;
    for (k = 0; k < n; k++)
        p[k] = t[k];
    p[n] = 0;
    GlobalUnlock(g);
    return g;
}

static unsigned char antes[256], teclas[256];
static WCHAR guardado[4096];

void inicio(void) {
    HANDLE inst = GetModuleHandleW(0), h;
    static unsigned char clase[80];
    WCHAR b[64];
    *(DWORD *)clase = 80;
    *(void **)(clase + 8) = (void *)proc;
    *(HANDLE *)(clase + 24) = inst;
    *(const WCHAR **)(clase + 64) = L"BMO_tanda7";
    RegisterClassExW(clase);
    h = CreateWindowExW(0, L"BMO_tanda7", L"tanda7", 0xCF0000, 0, 0, 100, 100, 0, 0, inst, 0);
    /* -- el estado del teclado del hilo */
    GetKeyboardState(antes);
    teclas['A'] = 0x80;
    teclas[0x14] = 0x01;
    mira(SetKeyboardState(teclas) && GetKeyState('A') < 0 && (GetKeyState(0x14) & 1) && GetKeyState(0x14) >= 0 && GetKeyState('B') == 0, "SetKeyboardState y GetKeyState: abajo y el interruptor");
    {
        unsigned char t[256];
        mira(GetKeyboardState(t) && t['A'] == 0x80 && t[0x14] == 1 && t['B'] == 0, "GetKeyboardState");
    }
    SetKeyboardState(antes);
    mira(GetAsyncKeyState(0x87) == 0, "GetAsyncKeyState de F24: nadie la pulsa");
    /* -- las tablas del teclado */
    mira(MapVirtualKeyW('A', 0) == 0x1E && MapVirtualKeyW(0x1E, 1) == 'A' && MapVirtualKeyW(0x1B, 0) == 1 && MapVirtualKeyW(0x0D, 0) == 0x1C, "MapVirtualKeyW: de tecla virtual a scancode y de vuelta");
    mira(MapVirtualKeyW(0x2A, 3) == 0xA0 && MapVirtualKeyW(0x36, 3) == 0xA1 && MapVirtualKeyW('A', 2) == 'A' && MapVirtualKeyW('7', 2) == '7', "MapVirtualKeyW: los dos Shift, y el caracter");
    mira(VkKeyScanW('a') == 0x41 && VkKeyScanW('A') == 0x141 && VkKeyScanW('7') == 0x37 && VkKeyScanW(' ') == 0x20, "VkKeyScanW: la tecla y el Shift");
    {
        unsigned char t[256] = {0};
        WCHAR c[4] = {0};
        int bien = ToUnicode('A', 0x1E, t, c, 4, 0) == 1 && c[0] == 'a';
        t[0x10] = 0x80;
        bien = bien && ToUnicode('A', 0x1E, t, c, 4, 0) == 1 && c[0] == 'A';
        t[0x14] = 0x01;
        bien = bien && ToUnicode('A', 0x1E, t, c, 4, 0) == 1 && c[0] == 'a';
        t[0x10] = t[0x14] = 0;
        t[0x11] = 0x80;
        bien = bien && ToUnicode('A', 0x1E, t, c, 4, 0) == 1 && c[0] == 1;
        t[0x11] = 0;
        mira(bien && ToUnicode(0x70, 0x3B, t, c, 4, 0) == 0, "ToUnicode: Shift, Bloq Mayus, Ctrl+A, y F1 no escribe");
    }
    mira(GetKeyNameTextW(0x1E << 16, b, 64) == 1 && b[0] == 'A', "GetKeyNameTextW de la A");
    {
        int n = 0;
        int bien = GetKeyboardLayoutNameW(b);
        while (bien && b[n])
            n++;
        mira(bien && n == 8 && GetKeyboardLayout(0) != 0 && GetKeyboardType(0) > 0, "GetKeyboardLayout, GetKeyboardLayoutNameW (ocho cifras) y GetKeyboardType");
    }
    /* -- el cursor */
    {
        int p[2], q[2];
        int x0 = GetSystemMetrics(76), y0 = GetSystemMetrics(77);
        int bien = GetCursorPos(p) && p[0] >= x0 && p[1] >= y0 && p[0] < x0 + GetSystemMetrics(78) && p[1] < y0 + GetSystemMetrics(79);
        mira(bien && SetCursorPos(p[0], p[1]) && GetCursorPos(q) && cerca(p[0], q[0]) && cerca(p[1], q[1]), "GetCursorPos en la pantalla virtual; SetCursorPos donde estaba");
    }
    {
        unsigned char ci[24] = {0};
        *(DWORD *)ci = 24;
        mira(GetCursorInfo(ci) && ShowCursor(0) == -1 && ShowCursor(1) == 0, "GetCursorInfo y el contador de ShowCursor");
    }
    mira(LoadCursorW(0, (const WCHAR *)32512) && LoadCursorW(0, (const WCHAR *)32512) == LoadCursorW(0, (const WCHAR *)32512) && LoadCursorW(0, (const WCHAR *)32512) != LoadCursorW(0, (const WCHAR *)32513) && LoadIconW(0, (const WCHAR *)32512), "LoadCursorW y LoadIconW del sistema: el mismo cada vez");
    {
        int r[4];
        int x0 = GetSystemMetrics(76), y0 = GetSystemMetrics(77);
        mira(ClipCursor(0) && GetClipCursor(r) && r[0] == x0 && r[1] == y0 && r[2] == x0 + GetSystemMetrics(78) && r[3] == y0 + GetSystemMetrics(79), "ClipCursor(NULL): GetClipCursor da la pantalla virtual");
    }
    mira(SetCapture(h) == 0 && GetCapture() == h && ReleaseCapture() && GetCapture() == 0, "SetCapture, GetCapture y ReleaseCapture");
    mira(GetDoubleClickTime() > 0, "GetDoubleClickTime");
    /* -- el raw input */
    {
        struct {
            unsigned short pagina, uso;
            DWORD banderas;
            HANDLE destino;
        } d = {1, 2, 0, 0}, r[4];
        unsigned n = 0;
        int bien;
        d.destino = h;
        bien = RegisterRawInputDevices(&d, 1, 16) && GetRegisteredRawInputDevices(0, &n, 16) == 0 && n == 1;
        n = 4;
        bien = bien && GetRegisteredRawInputDevices(r, &n, 16) == 1 && r[0].pagina == 1 && r[0].uso == 2 && r[0].destino == h;
        mira(bien, "RegisterRawInputDevices del raton, y GetRegisteredRawInputDevices lo ve");
        d.banderas = 1; /* RIDEV_REMOVE */
        d.destino = 0;
        n = 4;
        mira(RegisterRawInputDevices(&d, 1, 16) && GetRegisteredRawInputDevices(0, &n, 16) == 0 && n == 0, "RIDEV_REMOVE: ya no hay ninguno");
    }
    {
        static struct {
            HANDLE d;
            DWORD tipo;
        } l[64];
        unsigned n = 0, k, total;
        int raton = -1, teclado = -1;
        int bien = GetRawInputDeviceList(0, &n, 16) == 0 && n >= 2 && n <= 64;
        total = bien ? GetRawInputDeviceList(l, &n, 16) : 0;
        for (k = 0; k < total && k < 64; k++) {
            if (l[k].tipo == 0 && raton < 0)
                raton = (int)k;
            if (l[k].tipo == 1 && teclado < 0)
                teclado = (int)k;
        }
        mira(bien && total >= 2 && raton >= 0 && teclado >= 0, "GetRawInputDeviceList: hay un raton y un teclado");
        if (raton >= 0) {
            DWORD info[8] = {32};
            unsigned m = 32;
            mira(GetRawInputDeviceInfoW(l[raton].d, 0x2000000B, info, &m) == 32 && info[1] == 0, "GetRawInputDeviceInfoW del raton: RID_DEVICE_INFO de tipo raton");
        } else {
            mira(0, "GetRawInputDeviceInfoW del raton: RID_DEVICE_INFO de tipo raton");
        }
        {
            unsigned m = 0;
            mira(GetRawInputData((HANDLE)0x1234, 0x10000003, 0, &m, 24) == 0xFFFFFFFF, "GetRawInputData de un HRAWINPUT que no es: -1");
        }
    }
    /* -- el portapapeles */
    {
        int tenia = 0, n = 0, bien;
        DWORD s0, s1;
        HANDLE g;
        if (OpenClipboard(h)) {
            HANDLE v = IsClipboardFormatAvailable(13) ? GetClipboardData(13) : 0;
            WCHAR *p = v ? (WCHAR *)GlobalLock(v) : 0;
            if (p) {
                while (n < 4095 && p[n]) {
                    guardado[n] = p[n];
                    n++;
                }
                tenia = 1;
                GlobalUnlock(v);
            }
            CloseClipboard();
        }
        g = global_de(L"BMO tanda7", 10);
        mira(g && GlobalSize(g) >= 22, "GlobalAlloc, GlobalLock y GlobalSize");
        s0 = GetClipboardSequenceNumber();
        bien = OpenClipboard(h) && EmptyClipboard() && SetClipboardData(13, g) == g;
        s1 = GetClipboardSequenceNumber();
        mira(bien && s1 != s0 && IsClipboardFormatAvailable(13) && IsClipboardFormatAvailable(1), "SetClipboardData de CF_UNICODETEXT: CF_TEXT tambien esta");
        {
            HANDLE u = GetClipboardData(13), a = GetClipboardData(1);
            WCHAR *pu = u ? (WCHAR *)GlobalLock(u) : 0;
            char *pa = a ? (char *)GlobalLock(a) : 0;
            mira(pu && igual_w(pu, "BMO tanda7") && pa && igual_a(pa, "BMO tanda7"), "GetClipboardData: el texto, y en bytes");
            if (pu)
                GlobalUnlock(u);
            if (pa)
                GlobalUnlock(a);
        }
        SetLastError(0);
        mira(CloseClipboard() && !GetClipboardData(13) && GetLastError() == 1418, "CloseClipboard; y GetClipboardData sin abrir: ERROR_CLIPBOARD_NOT_OPEN");
        if (OpenClipboard(h)) {
            EmptyClipboard();
            if (tenia)
                SetClipboardData(13, global_de(guardado, n));
            CloseClipboard();
        }
    }
    {
        unsigned f = RegisterClipboardFormatW(L"BMO_tanda7_formato");
        mira(f >= 0xC000 && f <= 0xFFFF && RegisterClipboardFormatW(L"BMO_tanda7_formato") == f && GetClipboardFormatNameW(f, b, 64) == 18 && igual_w(b, "BMO_tanda7_formato"), "RegisterClipboardFormatW y GetClipboardFormatNameW");
    }
    DestroyWindow(h);
    di("tanda7.exe: el teclado, el raton y el portapapeles dicen lo de Windows\r\n");
    ExitProcess(fallos);
}
