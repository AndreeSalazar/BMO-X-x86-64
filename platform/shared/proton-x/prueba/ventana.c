/* ventana.c -- el .exe de P2 de PROTON-X: una ventana Win32 de manual.
 *
 * RegisterClassExW, CreateWindowExW, ShowWindow, UpdateWindow y el bucle de
 * GetMessageW / TranslateMessage / DispatchMessageW, como en cualquier tutorial
 * de Windows. Pinta con la CPU en un bufer suyo y lo pone en la ventana con
 * StretchDIBits (gdi32) dentro de WM_PAINT. Sin CRT y sin windows.h: las
 * estructuras van escritas aqui, con los desplazamientos de Windows x64.
 *
 *    una letra     cambia el tinte (WM_CHAR) y repinta
 *    un clic       deja un cuadrado blanco donde cayo (WM_LBUTTONDOWN)
 *    q o ESC       cierra: DestroyWindow -> WM_DESTROY -> PostQuitMessage
 *
 * Sale con (letras << 8) | clics: el banco lo mira, y en BMO-X lo dice
 * PROTON-X al acabar. En Windows corre igual. */
typedef unsigned short WCHAR;
typedef unsigned int UINT;
typedef unsigned long DWORD;
typedef long LONG;
typedef unsigned long long U64;
typedef long long I64;
typedef void *HANDLE;
typedef I64 (*WNDPROC)(HANDLE, UINT, U64, I64);

typedef struct {
    UINT cbSize, style;
    WNDPROC lpfnWndProc;
    int cbClsExtra, cbWndExtra;
    HANDLE hInstance, hIcon, hCursor, hbrBackground;
    const WCHAR *lpszMenuName, *lpszClassName;
    HANDLE hIconSm;
} WNDCLASSEXW;

typedef struct {
    HANDLE hwnd;
    UINT message;
    U64 wParam;
    I64 lParam;
    DWORD time;
    LONG x, y;
    DWORD lPrivate;
} MSG;

typedef struct {
    LONG left, top, right, bottom;
} RECT;

typedef struct {
    HANDLE hdc;
    int fErase;
    RECT rcPaint;
    int fRestore, fIncUpdate;
    unsigned char rgbReserved[32];
} PAINTSTRUCT;

typedef struct {
    DWORD biSize;
    LONG biWidth, biHeight;
    unsigned short biPlanes, biBitCount;
    DWORD biCompression, biSizeImage;
    LONG biXPelsPerMeter, biYPelsPerMeter;
    DWORD biClrUsed, biClrImportant;
} BITMAPINFOHEADER;

#define WM_CREATE 0x0001
#define WM_DESTROY 0x0002
#define WM_PAINT 0x000F
#define WM_KEYDOWN 0x0100
#define WM_CHAR 0x0102
#define WM_LBUTTONDOWN 0x0201
#define VK_ESCAPE 0x1B
#define WS_OVERLAPPEDWINDOW 0x00CF0000
#define CW_USEDEFAULT ((int)0x80000000)
#define SW_SHOW 5
#define DIB_RGB_COLORS 0
#define SRCCOPY 0x00CC0020

#define WINAPI __stdcall
#define IMPORTA __declspec(dllimport)
IMPORTA HANDLE WINAPI GetModuleHandleW(const WCHAR *nombre);
IMPORTA void WINAPI ExitProcess(UINT codigo);
IMPORTA unsigned short WINAPI RegisterClassExW(const WNDCLASSEXW *c);
IMPORTA HANDLE WINAPI CreateWindowExW(DWORD ex, const WCHAR *clase, const WCHAR *titulo, DWORD estilo, int x, int y,
                                      int ancho, int alto, HANDLE padre, HANDLE menu, HANDLE inst, void *param);
IMPORTA int WINAPI ShowWindow(HANDLE h, int como);
IMPORTA int WINAPI UpdateWindow(HANDLE h);
IMPORTA int WINAPI GetMessageW(MSG *m, HANDLE h, UINT min, UINT max);
IMPORTA int WINAPI TranslateMessage(const MSG *m);
IMPORTA I64 WINAPI DispatchMessageW(const MSG *m);
IMPORTA I64 WINAPI DefWindowProcW(HANDLE h, UINT m, U64 w, I64 l);
IMPORTA void WINAPI PostQuitMessage(int codigo);
IMPORTA int WINAPI DestroyWindow(HANDLE h);
IMPORTA int WINAPI InvalidateRect(HANDLE h, const RECT *r, int borrar);
IMPORTA HANDLE WINAPI BeginPaint(HANDLE h, PAINTSTRUCT *ps);
IMPORTA int WINAPI EndPaint(HANDLE h, const PAINTSTRUCT *ps);
IMPORTA int WINAPI StretchDIBits(HANDLE dc, int xd, int yd, int wd, int hd, int xs, int ys, int ws, int hs,
                                 const void *bits, const BITMAPINFOHEADER *bmi, UINT uso, DWORD rop);

#define ANCHO 320
#define ALTO 200

static unsigned pixeles[ANCHO * ALTO];
static unsigned tinte;
static unsigned letras, clics;
static int marca_x = -1, marca_y = -1;

/* Un degradado con el tinte, un marco de 2 pixeles y el cuadrado del clic.
 * Arriba es la fila 0: el DIB va de arriba abajo (biHeight negativo). */
static void pinta(void) {
    int x, y;
    for (y = 0; y < ALTO; y++) {
        for (x = 0; x < ANCHO; x++) {
            unsigned r = (unsigned)x * 255 / (ANCHO - 1), g = (unsigned)y * 255 / (ALTO - 1), b = 0x80;
            unsigned c;
            if (tinte % 3 == 1) { unsigned t = r; r = g; g = b; b = t; }
            if (tinte % 3 == 2) { unsigned t = r; r = b; b = g; g = t; }
            c = (r << 16) | (g << 8) | b;
            if (x < 2 || y < 2 || x >= ANCHO - 2 || y >= ALTO - 2) c = 0x00FFD700;
            if (marca_x >= 0 && x >= marca_x - 4 && x <= marca_x + 4 && y >= marca_y - 4 && y <= marca_y + 4) c = 0x00FFFFFF;
            pixeles[y * ANCHO + x] = c;
        }
    }
}

static I64 __stdcall proc(HANDLE h, UINT m, U64 w, I64 l) {
    switch (m) {
    case WM_CREATE:
        return 0;
    case WM_PAINT: {
        PAINTSTRUCT ps;
        BITMAPINFOHEADER bmi;
        HANDLE dc = BeginPaint(h, &ps);
        pinta();
        bmi.biSize = sizeof bmi;
        bmi.biWidth = ANCHO;
        bmi.biHeight = -ALTO;
        bmi.biPlanes = 1;
        bmi.biBitCount = 32;
        bmi.biCompression = 0;
        bmi.biSizeImage = 0;
        bmi.biXPelsPerMeter = 0;
        bmi.biYPelsPerMeter = 0;
        bmi.biClrUsed = 0;
        bmi.biClrImportant = 0;
        StretchDIBits(dc, 0, 0, ANCHO, ALTO, 0, 0, ANCHO, ALTO, pixeles, &bmi, DIB_RGB_COLORS, SRCCOPY);
        EndPaint(h, &ps);
        return 0;
    }
    case WM_CHAR:
        if (w == 'q') {
            DestroyWindow(h);
            return 0;
        }
        letras++;
        tinte++;
        InvalidateRect(h, 0, 0);
        return 0;
    case WM_KEYDOWN:
        if (w == VK_ESCAPE) DestroyWindow(h);
        return 0;
    case WM_LBUTTONDOWN:
        clics++;
        marca_x = (int)(short)(l & 0xFFFF);
        marca_y = (int)(short)((l >> 16) & 0xFFFF);
        InvalidateRect(h, 0, 0);
        return 0;
    case WM_DESTROY:
        PostQuitMessage((int)((letras << 8) | clics));
        return 0;
    }
    return DefWindowProcW(h, m, w, l);
}

static const WCHAR CLASE[] = L"BMOXVentana";
static const WCHAR TITULO[] = L"PROTON-X P2";

void inicio(void) {
    WNDCLASSEXW wc;
    MSG msg;
    HANDLE h, inst = GetModuleHandleW(0);
    wc.cbSize = sizeof wc;
    wc.style = 0;
    wc.lpfnWndProc = proc;
    wc.cbClsExtra = 0;
    wc.cbWndExtra = 0;
    wc.hInstance = inst;
    wc.hIcon = 0;
    wc.hCursor = 0;
    wc.hbrBackground = 0;
    wc.lpszMenuName = 0;
    wc.lpszClassName = CLASE;
    wc.hIconSm = 0;
    if (!RegisterClassExW(&wc)) ExitProcess(0xE001);
    h = CreateWindowExW(0, CLASE, TITULO, WS_OVERLAPPEDWINDOW, CW_USEDEFAULT, CW_USEDEFAULT, ANCHO, ALTO, 0, 0, inst, 0);
    if (!h) ExitProcess(0xE002);
    ShowWindow(h, SW_SHOW);
    UpdateWindow(h);
    msg.wParam = 0;
    while (GetMessageW(&msg, 0, 0, 0) > 0) {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    ExitProcess((UINT)msg.wParam);
}
