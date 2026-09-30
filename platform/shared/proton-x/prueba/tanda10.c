/* tanda10.c -- el .exe de la TANDA 10 de Cyberpunk (30-09): lo que quedaba
 * de user32. Un dialogo de una plantilla en memoria (sus controles, su
 * WM_INITDIALOG, GetDlgItem, EndDialog), el HDC de una ventana (GetDC,
 * FillRect, DrawFocusRect, DrawText midiendo, ReleaseDC), ValidateRect,
 * LoadString del STRINGTABLE propio (tanda10.rc), la estacion de ventanas
 * y los avisos de dispositivos. Solo relaciones: ninguna ventana se ve y
 * nada depende de la fuente de esta maquina.
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned short WORD;
typedef unsigned short WCHAR;
typedef unsigned long long U64;
typedef long long I64;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA HANDLE W GetModuleHandleW(const WCHAR *n);
IMPORTA DWORD W GetLastError(void);
IMPORTA void W SetLastError(DWORD e);
/* user32 (tanda10_user32.def) */
IMPORTA unsigned short W RegisterClassExW(const void *c);
IMPORTA HANDLE W CreateWindowExW(DWORD ex, const WCHAR *clase, const WCHAR *t, DWORD estilo, int x, int y, int w, int h, HANDLE padre, HANDLE menu, HANDLE inst, void *p);
IMPORTA int W DestroyWindow(HANDLE h);
IMPORTA I64 W DefWindowProcW(HANDLE h, unsigned m, U64 w, I64 l);
IMPORTA HANDLE W GetParent(HANDLE h);
IMPORTA int W GetClassNameW(HANDLE h, WCHAR *b, int n);
IMPORTA int W GetWindowTextW(HANDLE h, WCHAR *b, int n);
IMPORTA int W InvalidateRect(HANDLE h, const void *r, int borrar);
IMPORTA int W PeekMessageW(void *m, HANDLE h, unsigned de, unsigned a, unsigned quitar);
IMPORTA HANDLE W CreateDialogIndirectParamW(HANDLE inst, const void *t, HANDLE padre, I64(W *f)(HANDLE, unsigned, U64, I64), I64 param);
IMPORTA int W EndDialog(HANDLE h, I64 r);
IMPORTA HANDLE W GetDlgItem(HANDLE h, int id);
IMPORTA long W GetDialogBaseUnits(void);
IMPORTA HANDLE W GetTopWindow(HANDLE h);
IMPORTA HANDLE W GetDC(HANDLE h);
IMPORTA int W ReleaseDC(HANDLE h, HANDLE dc);
IMPORTA int W FillRect(HANDLE dc, const int *r, HANDLE pincel);
IMPORTA int W DrawFocusRect(HANDLE dc, const int *r);
IMPORTA int W ValidateRect(HANDLE h, const int *r);
IMPORTA int W DrawTextW(HANDLE dc, const WCHAR *t, int n, int *r, unsigned f);
IMPORTA int W DrawTextExW(HANDLE dc, WCHAR *t, int n, int *r, unsigned f, void *p);
IMPORTA DWORD W GetSysColor(int i);
IMPORTA int W LoadStringW(HANDLE inst, unsigned id, WCHAR *b, int n);
IMPORTA int W LoadStringA(HANDLE inst, unsigned id, char *b, int n);
IMPORTA HANDLE W GetProcessWindowStation(void);
IMPORTA int W GetUserObjectInformationW(HANDLE h, int que, void *b, DWORD n, DWORD *hace_falta);
IMPORTA HANDLE W RegisterDeviceNotificationW(HANDLE h, void *filtro, DWORD banderas);
IMPORTA int W UnregisterDeviceNotification(HANDLE h);
IMPORTA void W DisableProcessWindowsGhosting(void);

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

static I64 W proc_ventana(HANDLE h, unsigned m, U64 w, I64 l) {
    return DefWindowProcW(h, m, w, l);
}

/* -- el dialogo */
static HANDLE init_h, init_foco;
static I64 init_param;

static I64 W proc_dialogo(HANDLE h, unsigned m, U64 w, I64 l) {
    if (m == 0x110) {
        init_h = h;
        init_foco = (HANDLE)w;
        init_param = l;
        return 1;
    }
    return 0;
}

/* La plantilla, WORD a WORD (alineada a DWORD: static de WORDs empieza en 4). */
static __declspec(align(4)) WORD plantilla[128];
static int k;

static void w16(unsigned v) { plantilla[k++] = (WORD)v; }
static void w32(DWORD v) { w16(v & 0xFFFF); w16(v >> 16); }
static void texto(const WCHAR *t) {
    while (*t)
        w16(*t++);
    w16(0);
}
static void alinear(void) {
    if (k & 1)
        w16(0);
}
static void control(DWORD estilo, int x, int y, int cx, int cy, unsigned id, unsigned clase, const WCHAR *t) {
    alinear();
    w32(estilo);
    w32(0);
    w16(x), w16(y), w16(cx), w16(cy), w16(id);
    w16(0xFFFF), w16(clase);
    texto(t);
    w16(0);
}

void inicio(void);

void inicio(void) {
    static unsigned char wc[80];
    static WCHAR b[64];
    static char a[64];
    static unsigned char msg[48], filtro[32];
    static DWORD flags[3];
    HANDLE inst = GetModuleHandleW(0), h, dlg, dc, n1, n2, aviso;
    const WCHAR *p;
    int r[4], r1[4], r2[4], a1, a2, x;
    long u;
    DWORD hace_falta;

    DisableProcessWindowsGhosting();
    /* -- una ventana oculta y su HDC */
    *(DWORD *)wc = 80;
    *(void **)(wc + 8) = (void *)proc_ventana;
    *(HANDLE *)(wc + 24) = inst;
    *(const WCHAR **)(wc + 64) = L"BMO_tanda10";
    h = RegisterClassExW(wc) ? CreateWindowExW(0, L"BMO_tanda10", L"tanda10", 0x00CF0000, 10, 10, 200, 100, 0, 0, inst, 0) : 0;
    dc = h ? GetDC(h) : 0;
    r[0] = 10, r[1] = 10, r[2] = 60, r[3] = 40;
    mira(h && dc && FillRect(dc, r, (HANDLE)(5 + 1)), "GetDC y FillRect con un pincel de color de sistema");
    mira(DrawFocusRect(dc, r) && DrawFocusRect(dc, r), "DrawFocusRect (dos veces: se borra)");
    r1[0] = r1[1] = 0, r1[2] = r1[3] = 0;
    r2[0] = r2[1] = 0, r2[2] = r2[3] = 0;
    a1 = DrawTextW(dc, L"ab", -1, r1, 0x400);
    a2 = DrawTextW(dc, L"abab", 4, r2, 0x400);
    mira(a1 > 0 && a1 == r1[3] && r1[2] > 0 && r2[2] > r1[2] && a2 == a1, "DrawTextW DT_CALCRECT: mas letras, mas ancho; el mismo alto");
    r2[0] = r2[1] = 0, r2[2] = r2[3] = 0;
    a2 = DrawTextExW(dc, L"a\nb", -1, r2, 0x400, 0);
    mira(a2 == 2 * a1 && r2[3] == a2, "DrawTextExW DT_CALCRECT: dos lineas, el doble de alto");
    mira(ReleaseDC(h, dc) == 1, "ReleaseDC");
    InvalidateRect(h, 0, 0);
    mira(ValidateRect(h, 0) && !PeekMessageW(msg, h, 0x0F, 0x0F, 1), "ValidateRect: despues, ningun WM_PAINT");
    mira(GetSysColor(5) == 0xFFFFFF && GetSysColor(8) == 0, "GetSysColor: COLOR_WINDOW blanco, COLOR_WINDOWTEXT negro");
    u = GetDialogBaseUnits();
    mira((u & 0xFFFF) > 0 && (u >> 16) > (u & 0xFFFF), "GetDialogBaseUnits: mas alto que ancho");

    /* -- el dialogo */
    k = 0;
    w32(0x80C00000);
    w32(0);
    w16(2);
    w16(0), w16(0), w16(120), w16(60);
    w16(0), w16(0);
    texto(L"Dialogo");
    control(0x50010000, 5, 5, 50, 14, 1, 0x80, L"Aceptar");
    control(0x50000000, 5, 25, 50, 10, 100, 0x82, L"Hola");
    dlg = CreateDialogIndirectParamW(inst, plantilla, h, proc_dialogo, 0x55);
    n1 = dlg ? GetDlgItem(dlg, 1) : 0;
    n2 = dlg ? GetDlgItem(dlg, 100) : 0;
    mira(dlg && init_h == dlg && init_param == 0x55 && init_foco == n1, "CreateDialogIndirectParamW: WM_INITDIALOG con su param y el primer control");
    mira(GetClassNameW(dlg, b, 64) && igual_w(b, L"#32770") && GetWindowTextW(dlg, b, 64) && igual_w(b, L"Dialogo"), "el dialogo es un #32770, con su titulo");
    mira(n1 && n2 && GetParent(n1) == dlg && GetClassNameW(n1, b, 64) && igual_w(b, L"Button") && GetWindowTextW(n1, b, 64) && igual_w(b, L"Aceptar"), "GetDlgItem: el boton, hijo del dialogo");
    mira(GetClassNameW(n2, b, 64) && igual_w(b, L"Static") && GetWindowTextW(n2, b, 64) && igual_w(b, L"Hola"), "GetDlgItem: el texto");
    SetLastError(0);
    mira(!GetDlgItem(dlg, 999) && GetLastError() == 1421, "GetDlgItem de un ID que no esta: ERROR_CONTROL_ID_NOT_FOUND");
    mira(GetTopWindow(dlg) == n1 && GetTopWindow(n1) == 0, "GetTopWindow: el primer control; un control no tiene hijas");
    mira(EndDialog(dlg, 7) && DestroyWindow(dlg), "EndDialog y DestroyWindow");

    /* -- LoadString del STRINGTABLE */
    mira(LoadStringW(inst, 101, b, 64) == 10 && igual_w(b, L"Night City"), "LoadStringW");
    mira(LoadStringW(inst, 101, b, 5) == 4 && igual_w(b, L"Nigh"), "LoadStringW cortada, con su 0");
    x = LoadStringW(inst, 102, (WCHAR *)&p, 0);
    mira(x == 7 && p[0] == 'A' && p[6] == 'a', "LoadStringW con 0: el puntero a la cadena");
    mira(LoadStringA(inst, 102, a, 64) == 7 && igual(a, "Arasaka"), "LoadStringA");
    b[0] = 'x';
    mira(LoadStringW(inst, 999, b, 64) == 0 && b[0] == 0, "LoadStringW de una que no esta: 0 y vacia");

    /* -- la estacion de ventanas */
    mira(GetUserObjectInformationW(GetProcessWindowStation(), 2, b, sizeof b, &hace_falta) && hace_falta == 16 && igual_w(b, L"WinSta0"), "GetUserObjectInformationW UOI_NAME: WinSta0");
    mira(GetUserObjectInformationW(GetProcessWindowStation(), 1, flags, sizeof flags, &hace_falta) && (flags[2] & 1), "UOI_FLAGS: la estacion se ve (WSF_VISIBLE)");
    mira(!GetUserObjectInformationW(GetProcessWindowStation(), 2, b, 4, &hace_falta) && GetLastError() == 122 && hace_falta == 16, "UOI_NAME sin sitio: ERROR_INSUFFICIENT_BUFFER y lo que hace falta");

    /* -- los avisos de dispositivos */
    *(DWORD *)filtro = 32;
    *(DWORD *)(filtro + 4) = 5;
    aviso = RegisterDeviceNotificationW(h, filtro, 4);
    mira(aviso && UnregisterDeviceNotification(aviso), "RegisterDeviceNotificationW y UnregisterDeviceNotification");
    mira(DestroyWindow(h), "DestroyWindow de la ventana");
    di("tanda10.exe: lo que quedaba de user32 es lo de Windows\r\n");
    ExitProcess(fallos);
}
