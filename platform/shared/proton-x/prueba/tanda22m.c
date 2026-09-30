/* tanda22m.c -- la TANDA 22 compilada por MSVC (30-09): la gemela de
 * tanda22.c, con tanda22md.dll (MSVC) en vez de tanda22d.dll (clang sin
 * CRT, que Windows ignoro). Mismas comprobaciones y mismos `dato`.
 *
 * En la consola de "x64 Native Tools Command Prompt for VS", despues de
 * hacer tanda22md.dll:
 *   cl /nologo /MD /O1 tanda22m.c tanda22md.lib /Fe:tanda22m.exe
 * Sale con el numero de fallos. */
#include <windows.h>

__declspec(dllimport) int leer(void);
__declspec(dllimport) void poner(int v);
__declspec(dllimport) int en_main(void);
__declspec(dllimport) int antes_de_main(void);
__declspec(dllimport) int callbacks_hilo(void);
__declspec(dllimport) unsigned indice(void);
__declspec(dllimport) unsigned long long su_bloque(void);
extern ULONG _tls_index;

static __declspec(thread) int del_exe = 5;
static unsigned fallos;

static void di(const char *t) {
    DWORD n = 0, k = 0;
    while (t[n])
        n++;
    WriteFile(GetStdHandle(STD_OUTPUT_HANDLE), t, n, &k, NULL);
}

static void mira(int bien, const char *que) {
    if (!bien)
        fallos++;
    di(bien ? "  bien  " : "  MAL   ");
    di(que);
    di("\r\n");
}

static void dato(const char *que, unsigned long long v) {
    char b[24];
    int i = 23;
    b[i] = 0;
    do {
        b[--i] = (char)('0' + v % 10);
        v /= 10;
    } while (v && i > 0);
    di("  dato  ");
    di(que);
    di(&b[i]);
    di("\r\n");
}

static volatile int en_hilo_dll = -1, en_hilo_exe = -1;

static DWORD WINAPI hilo(void *p) {
    (void)p;
    en_hilo_dll = leer();
    en_hilo_exe = del_exe;
    poner(55);
    del_exe = 66;
    return 0;
}

int main(void) {
    HANDLE h;
    unsigned long long mio = ((unsigned long long *)__readgsqword(0x58))[_tls_index];
    dato("indice del .exe: ", _tls_index);
    dato("indice de la DLL: ", indice());
    dato("callbacks antes de su DllMain: ", (unsigned long long)antes_de_main());
    dato("lo que vio su DllMain: ", (unsigned long long)(unsigned)en_main());
    dato("bloque del .exe == bloque de la DLL (1 si): ", mio == su_bloque());
    mira(antes_de_main() == 1, "el callback de TLS de la DLL corre ANTES de su DllMain");
    mira(en_main() == 42, "su DllMain ve su variable de hilo con su valor inicial");
    mira(leer() == 7, "lo que su DllMain escribio, lo ve el hilo principal");
    mira(del_exe == 5, "el .exe tiene su propio TLS, aparte");
    poner(100);
    del_exe = 200;
    mira(leer() == 100 && del_exe == 200, "cada modulo escribe en lo suyo");
    h = CreateThread(NULL, 0, hilo, NULL, 0, NULL);
    mira(h != NULL && WaitForSingleObject(h, 5000) == 0, "un hilo nuevo corre y acaba");
    mira(en_hilo_dll == 42 && en_hilo_exe == 5, "el hilo nuevo tiene bloques NUEVOS, con los valores iniciales");
    mira(leer() == 100 && del_exe == 200, "lo que escribio el hilo no toca los del principal");
    mira(callbacks_hilo() >= 1, "el callback de TLS de la DLL vio el THREAD_ATTACH");
    CloseHandle(h);
    di("tanda22m.exe: el TLS de una DLL, hecho por MSVC\r\n");
    return (int)fallos;
}
