/* tanda6.c -- el .exe de la TANDA 6 de Cyberpunk (29-09): user32, grupo 2.
 * Las ventanas y sus mensajes: los "longs" (USERDATA, bytes de mas, el
 * estilo, subclasificar con GWLP_WNDPROC), la cola filtrada, los mensajes
 * del hilo, los temporizadores (con y sin TIMERPROC), MsgWaitFor..., los
 * nombres, la familia, la posicion y las variantes A.
 *
 * Todo con ventanas OCULTAS: nada se ve. Sale con el numero de fallos. En
 * Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned long long U64;
typedef long long I64;
typedef unsigned short WCHAR;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
typedef I64(W *PROC)(HANDLE, unsigned, U64, I64);
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA HANDLE W GetModuleHandleW(const WCHAR *n);
IMPORTA DWORD W GetLastError(void);
IMPORTA void W SetLastError(DWORD e);
IMPORTA DWORD W GetCurrentThreadId(void);
IMPORTA DWORD W GetCurrentProcessId(void);
IMPORTA HANDLE W CreateEventW(void *a, int manual, int inicial, const void *n);
IMPORTA int W CloseHandle(HANDLE h);
IMPORTA unsigned short W RegisterClassExW(const void *c);
IMPORTA unsigned short W RegisterClassExA(const void *c);
IMPORTA int W UnregisterClassW(const WCHAR *n, HANDLE inst);
IMPORTA int W GetClassInfoExW(HANDLE inst, const WCHAR *n, void *c);
IMPORTA HANDLE W CreateWindowExW(DWORD ex, const WCHAR *clase, const WCHAR *t, DWORD estilo, int x, int y, int w, int h, HANDLE padre, HANDLE menu, HANDLE inst, void *p);
IMPORTA HANDLE W CreateWindowExA(DWORD ex, const char *clase, const char *t, DWORD estilo, int x, int y, int w, int h, HANDLE padre, HANDLE menu, HANDLE inst, void *p);
IMPORTA int W DestroyWindow(HANDLE h);
IMPORTA I64 W DefWindowProcW(HANDLE h, unsigned m, U64 w, I64 l);
IMPORTA I64 W DefWindowProcA(HANDLE h, unsigned m, U64 w, I64 l);
IMPORTA I64 W CallWindowProcW(PROC p, HANDLE h, unsigned m, U64 w, I64 l);
IMPORTA I64 W GetWindowLongPtrW(HANDLE h, int i);
IMPORTA I64 W SetWindowLongPtrW(HANDLE h, int i, I64 v);
IMPORTA long W GetWindowLongW(HANDLE h, int i);
IMPORTA long W SetWindowLongW(HANDLE h, int i, long v);
IMPORTA U64 W GetClassLongPtrW(HANDLE h, int i);
IMPORTA int W IsWindow(HANDLE h);
IMPORTA int W IsWindowVisible(HANDLE h);
IMPORTA int W IsWindowUnicode(HANDLE h);
IMPORTA int W IsWindowEnabled(HANDLE h);
IMPORTA int W EnableWindow(HANDLE h, int si);
IMPORTA I64 W SendMessageW(HANDLE h, unsigned m, U64 w, I64 l);
IMPORTA I64 W SendMessageA(HANDLE h, unsigned m, U64 w, I64 l);
IMPORTA int W PostMessageW(HANDLE h, unsigned m, U64 w, I64 l);
IMPORTA int W PostThreadMessageW(DWORD t, unsigned m, U64 w, I64 l);
IMPORTA int W PeekMessageW(void *msg, HANDLE h, unsigned min, unsigned max, unsigned quitar);
IMPORTA I64 W DispatchMessageW(const void *msg);
IMPORTA DWORD W GetQueueStatus(unsigned f);
IMPORTA unsigned W RegisterWindowMessageW(const WCHAR *n);
IMPORTA U64 W SetTimer(HANDLE h, U64 id, unsigned ms, void *f);
IMPORTA int W KillTimer(HANDLE h, U64 id);
IMPORTA DWORD W MsgWaitForMultipleObjects(DWORD n, const HANDLE *hs, int todos, DWORD ms, DWORD mascara);
IMPORTA int W SetWindowTextW(HANDLE h, const WCHAR *t);
IMPORTA int W SetWindowTextA(HANDLE h, const char *t);
IMPORTA int W GetWindowTextW(HANDLE h, WCHAR *b, int n);
IMPORTA int W GetWindowTextA(HANDLE h, char *b, int n);
IMPORTA int W GetWindowTextLengthW(HANDLE h);
IMPORTA int W GetClassNameW(HANDLE h, WCHAR *b, int n);
IMPORTA int W GetClassNameA(HANDLE h, char *b, int n);
IMPORTA HANDLE W FindWindowW(const WCHAR *c, const WCHAR *t);
IMPORTA HANDLE W FindWindowA(const char *c, const char *t);
IMPORTA int W EnumWindows(int(W *f)(HANDLE, I64), I64 l);
IMPORTA DWORD W GetWindowThreadProcessId(HANDLE h, DWORD *pid);
IMPORTA HANDLE W GetParent(HANDLE h);
IMPORTA HANDLE W GetAncestor(HANDLE h, unsigned como);
IMPORTA HANDLE W GetWindow(HANDLE h, unsigned que);
IMPORTA HANDLE W GetDesktopWindow(void);
IMPORTA int W SetWindowPos(HANDLE h, HANDLE despues, int x, int y, int cx, int cy, unsigned f);
IMPORTA int W MoveWindow(HANDLE h, int x, int y, int cx, int cy, int repintar);
IMPORTA int W GetWindowRect(HANDLE h, int *r);
IMPORTA int W ClientToScreen(HANDLE h, int *p);
IMPORTA int W ScreenToClient(HANDLE h, int *p);

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

#define WM_APP 0x8000
#define WM_TIMER 0x0113
#define PM_REMOVE 1

/* MSG de x64: hwnd, message, wParam, lParam, time, pt. */
typedef struct {
    HANDLE hwnd;
    unsigned mensaje;
    U64 w;
    I64 l;
    DWORD hora;
    int pt[2];
    DWORD privado;
} Msg;

/* La WndProc de la clase: los WM_APP+n dan w + l. */
static I64 W proc(HANDLE h, unsigned m, U64 w, I64 l) {
    if (m > WM_APP && m < WM_APP + 16)
        return (I64)w + l;
    return DefWindowProcW(h, m, w, l);
}

/* La que la subclasifica: lo de antes, mas 100. */
static PROC vieja;
static I64 W nueva(HANDLE h, unsigned m, U64 w, I64 l) {
    if (m > WM_APP && m < WM_APP + 16)
        return CallWindowProcW(vieja, h, m, w, l) + 100;
    return CallWindowProcW(vieja, h, m, w, l);
}

static I64 W proc_a(HANDLE h, unsigned m, U64 w, I64 l) {
    return DefWindowProcA(h, m, w, l);
}

static U64 visto_id;
static void W al_tiempo(HANDLE h, unsigned m, U64 id, DWORD hora) {
    (void)h, (void)hora;
    if (m == WM_TIMER)
        visto_id = id;
}

static HANDLE buscada;
static int vista;
static int W cada_ventana(HANDLE h, I64 l) {
    if (h == buscada && l == 77)
        vista = 1;
    return 1;
}

/* Sacar lo que quede en la cola, sin despachar. */
static void vaciar(void) {
    Msg m;
    int k = 0;
    while (k++ < 1000 && PeekMessageW(&m, 0, 0, 0, PM_REMOVE))
        ;
}

void inicio(void) {
    HANDLE inst = GetModuleHandleW(0), h;
    static unsigned char clase[80];
    unsigned short atomo;
    Msg m;
    *(DWORD *)clase = 80;
    *(void **)(clase + 8) = (void *)proc;
    *(int *)(clase + 20) = 16; /* cbWndExtra */
    *(HANDLE *)(clase + 24) = inst;
    *(const WCHAR **)(clase + 64) = L"BMO_tanda6";
    atomo = RegisterClassExW(clase);
    h = CreateWindowExW(0, L"BMO_tanda6", L"tanda6", 0xCF0000, 0, 0, 320, 200, 0, 0, inst, 0);
    mira(atomo && h && IsWindow(h) && !IsWindowVisible(h) && IsWindowUnicode(h), "RegisterClassExW y CreateWindowExW: una ventana oculta, de W");
    /* -- los longs */
    {
        I64 e = GetWindowLongPtrW(h, -16);
        mira((e & 0xCF0000) == 0xCF0000 && !(e & 0x10000000) && GetWindowLongPtrW(h, -6) == (I64)inst && GetWindowLongPtrW(h, -8) == 0, "GetWindowLongPtrW: el estilo, la instancia y el padre");
    }
    mira(SetWindowLongPtrW(h, -21, 0x1234567890LL) == 0 && GetWindowLongPtrW(h, -21) == 0x1234567890LL, "GWLP_USERDATA: se guarda y se lee");
    mira(SetWindowLongPtrW(h, 8, -5) == 0 && GetWindowLongPtrW(h, 8) == -5 && SetWindowLongW(h, 0, 5) == 0 && GetWindowLongW(h, 0) == 5, "los cbWndExtra: 16 bytes de la ventana");
    SetLastError(0);
    mira(GetWindowLongPtrW(h, 16) == 0 && GetLastError() == 1413, "fuera de los cbWndExtra: ERROR_INVALID_INDEX");
    mira(GetClassLongPtrW(h, -18) == 16 && GetClassLongPtrW(h, -24) == (U64)proc, "GetClassLongPtrW: cbWndExtra y la WndProc de la clase");
    /* -- subclasificar */
    vieja = (PROC)SetWindowLongPtrW(h, -4, (I64)nueva);
    mira(vieja && GetWindowLongPtrW(h, -4) == (I64)nueva && SendMessageW(h, WM_APP + 1, 3, 4) == 107, "subclasificar con GWLP_WNDPROC y CallWindowProcW: SendMessageW pasa por las dos");
    /* -- la cola filtrada */
    {
        int bien;
        vaciar();
        bien = PostMessageW(h, WM_APP + 2, 1, 2) && PostThreadMessageW(GetCurrentThreadId(), WM_APP + 3, 5, 6);
        mira(bien && (GetQueueStatus(8) >> 16) & 8, "PostMessageW y PostThreadMessageW; GetQueueStatus los ve");
        bien = !PeekMessageW(&m, h, WM_APP + 3, WM_APP + 3, PM_REMOVE);
        bien = bien && PeekMessageW(&m, (HANDLE)(I64)-1, WM_APP, WM_APP + 15, PM_REMOVE) && m.hwnd == 0 && m.mensaje == WM_APP + 3 && m.w == 5;
        mira(bien, "PeekMessageW por ventana no ve los del hilo; con -1, solo los del hilo");
        bien = PeekMessageW(&m, 0, WM_APP + 2, WM_APP + 2, 0) && m.hwnd == h;
        bien = bien && PeekMessageW(&m, 0, WM_APP + 2, WM_APP + 2, PM_REMOVE) && DispatchMessageW(&m) == 103;
        mira(bien && !PeekMessageW(&m, 0, WM_APP, WM_APP + 15, PM_REMOVE), "PeekMessageW por numero, sin sacar y sacando; DispatchMessageW a la WndProc");
    }
    /* -- los temporizadores */
    {
        int bien = SetTimer(h, 7, 10, 0) != 0 && MsgWaitForMultipleObjects(0, 0, 0, 2000, 0x10) == 0;
        bien = bien && PeekMessageW(&m, h, WM_TIMER, WM_TIMER, PM_REMOVE) && m.hwnd == h && m.w == 7;
        mira(bien && KillTimer(h, 7), "SetTimer de una ventana, MsgWaitForMultipleObjects despierta, WM_TIMER y KillTimer");
    }
    {
        U64 id = SetTimer(0, 0, 10, (void *)al_tiempo);
        int bien = id && MsgWaitForMultipleObjects(0, 0, 0, 2000, 0x10) == 0;
        bien = bien && PeekMessageW(&m, 0, WM_TIMER, WM_TIMER, PM_REMOVE) && m.hwnd == 0 && m.w == id && m.l == (I64)al_tiempo;
        DispatchMessageW(&m);
        mira(bien && visto_id == id && KillTimer(0, id), "SetTimer sin ventana con TIMERPROC: DispatchMessageW la llama");
    }
    {
        HANDLE e = CreateEventW(0, 1, 1, 0);
        HANDLE f = CreateEventW(0, 1, 0, 0);
        vaciar();
        mira(MsgWaitForMultipleObjects(1, &e, 0, 0, 0x4FF) == 0 && MsgWaitForMultipleObjects(1, &f, 0, 30, 0x4FF) == 258, "MsgWaitForMultipleObjects: un evento encendido, y sin nada: WAIT_TIMEOUT");
        CloseHandle(e);
        CloseHandle(f);
    }
    {
        unsigned a = RegisterWindowMessageW(L"BMO_tanda6_msg");
        mira(a >= 0xC000 && a <= 0xFFFF && RegisterWindowMessageW(L"BMO_tanda6_msg") == a && RegisterWindowMessageW(L"BMO_tanda6_otro") != a, "RegisterWindowMessageW: el mismo numero para el mismo nombre");
    }
    /* -- los nombres */
    {
        WCHAR b[64];
        int bien = SetWindowTextW(h, L"tanda6 dos") && GetWindowTextLengthW(h) == 10 && GetWindowTextW(h, b, 64) == 10 && igual_w(b, "tanda6 dos");
        mira(bien && GetWindowTextW(h, b, 4) == 3 && igual_w(b, "tan") && SendMessageW(h, 0x000E, 0, 0) == 10, "SetWindowTextW, GetWindowTextW (tambien cortado) y WM_GETTEXTLENGTH");
        mira(GetClassNameW(h, b, 64) == 10 && igual_w(b, "BMO_tanda6"), "GetClassNameW");
    }
    mira(FindWindowW(L"bmo_TANDA6", 0) == h && FindWindowW(0, L"TANDA6 DOS") == h && FindWindowW(L"BMO_tanda6", L"otra") == 0, "FindWindowW por clase y por titulo, sin mayusculas");
    buscada = h;
    mira(EnumWindows(cada_ventana, 77) && vista, "EnumWindows: la ventana oculta esta");
    {
        DWORD pid = 0;
        mira(GetWindowThreadProcessId(h, &pid) == GetCurrentThreadId() && pid == GetCurrentProcessId(), "GetWindowThreadProcessId: este hilo y este proceso");
    }
    {
        static unsigned char wc[80];
        *(DWORD *)wc = 80;
        mira(GetClassInfoExW(inst, L"BMO_tanda6", wc) && *(int *)(wc + 20) == 16 && *(void **)(wc + 8) == (void *)proc, "GetClassInfoExW: la clase de vuelta");
    }
    /* -- la familia y la posicion */
    mira(GetParent(h) == 0 && GetAncestor(h, 2) == h && GetAncestor(h, 1) == GetDesktopWindow() && GetWindow(h, 4) == 0, "GetParent, GetAncestor y GetWindow de una ventana de arriba");
    {
        int r[4], s[4], p[2] = {0, 0};
        GetWindowRect(h, r);
        mira(SetWindowPos(h, 0, 10, 20, 0, 0, 0x15) && GetWindowRect(h, s) && s[0] == 10 && s[1] == 20 && s[2] - s[0] == r[2] - r[0] && s[3] - s[1] == r[3] - r[1], "SetWindowPos: se mueve, la medida no");
        mira(MoveWindow(h, 30, 40, r[2] - r[0], r[3] - r[1], 0) && GetWindowRect(h, s) && s[0] == 30 && s[1] == 40, "MoveWindow");
        mira(ClientToScreen(h, p) && p[0] >= 30 && p[1] >= 40 && ScreenToClient(h, p) && p[0] == 0 && p[1] == 0, "ClientToScreen y ScreenToClient, con la ventana movida");
    }
    mira(EnableWindow(h, 0) == 0 && !IsWindowEnabled(h) && EnableWindow(h, 1) != 0 && IsWindowEnabled(h), "EnableWindow e IsWindowEnabled");
    /* -- las A */
    {
        static unsigned char ca[80];
        char b[64];
        HANDLE ha;
        *(DWORD *)ca = 80;
        *(void **)(ca + 8) = (void *)proc_a;
        *(HANDLE *)(ca + 24) = inst;
        *(const char **)(ca + 64) = "BMO_tanda6a";
        RegisterClassExA(ca);
        ha = CreateWindowExA(0, "BMO_tanda6a", "ventana A", 0xCF0000, 0, 0, 100, 100, 0, 0, inst, 0);
        mira(ha && !IsWindowUnicode(ha) && GetWindowTextA(ha, b, 64) == 9 && igual_a(b, "ventana A") && GetClassNameA(ha, b, 64) == 11 && igual_a(b, "BMO_tanda6a"), "RegisterClassExA y CreateWindowExA: los nombres de bytes");
        mira(SetWindowTextA(ha, "otra") && SendMessageA(ha, 0x000D, 64, (I64)b) == 4 && igual_a(b, "otra") && FindWindowA("BMO_tanda6a", "otra") == ha, "SetWindowTextA, WM_GETTEXT por DefWindowProcA y FindWindowA");
        DestroyWindow(ha);
    }
    /* -- el final */
    SetLastError(0);
    mira(!UnregisterClassW(L"BMO_tanda6", inst) && GetLastError() == 1412, "UnregisterClassW con la ventana viva: ERROR_CLASS_HAS_WINDOWS");
    {
        static unsigned char wc[80];
        *(DWORD *)wc = 80;
        mira(DestroyWindow(h) && !IsWindow(h) && UnregisterClassW(L"BMO_tanda6", inst) && !GetClassInfoExW(inst, L"BMO_tanda6", wc) && GetLastError() == 1411, "DestroyWindow y UnregisterClassW: ya no estan");
    }
    di("tanda6.exe: las ventanas y sus mensajes dicen lo de Windows\r\n");
    ExitProcess(fallos);
}
