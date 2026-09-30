/* tanda22md.c -- la DLL de la TANDA 22 compilada por MSVC (30-09): la
 * gemela de tanda22d.c con el CRT de verdad (su _tls_used, su tlssup), como
 * libxess_fg.dll del juego. La de clang sin CRT, Windows la IGNORO (su
 * indice sin poner, ni un callback): esta dice si el TLS de una DLL va en
 * Windows cuando la hace MSVC.
 *
 * En la consola de "x64 Native Tools Command Prompt for VS":
 *   cl /nologo /MD /O1 /LD tanda22md.c /Fe:tanda22md.dll
 * (deja tanda22md.lib, que usa tanda22m.c). */
#include <windows.h>

static int cb_proceso, cb_hilo;
static void NTAPI al_tls(PVOID h, DWORD motivo, PVOID r) {
    (void)h;
    (void)r;
    if (motivo == DLL_PROCESS_ATTACH)
        cb_proceso++;
    if (motivo == DLL_THREAD_ATTACH)
        cb_hilo++;
}
#pragma comment(linker, "/INCLUDE:_tls_used")
#pragma comment(linker, "/INCLUDE:tanda22_cb")
#pragma const_seg(".CRT$XLB")
extern const PIMAGE_TLS_CALLBACK tanda22_cb;
const PIMAGE_TLS_CALLBACK tanda22_cb = al_tls;
#pragma const_seg()

static __declspec(thread) int por_hilo = 42;
static int visto_en_main = -1, callbacks_antes_de_main = -1;

BOOL WINAPI DllMain(HINSTANCE h, DWORD motivo, LPVOID r) {
    (void)h;
    (void)r;
    if (motivo == DLL_PROCESS_ATTACH) {
        visto_en_main = por_hilo;
        callbacks_antes_de_main = cb_proceso;
        por_hilo = 7;
    }
    return TRUE;
}

__declspec(dllexport) int leer(void) { return por_hilo; }
__declspec(dllexport) void poner(int v) { por_hilo = v; }
__declspec(dllexport) int en_main(void) { return visto_en_main; }
__declspec(dllexport) int antes_de_main(void) { return callbacks_antes_de_main; }
__declspec(dllexport) int callbacks_hilo(void) { return cb_hilo; }
extern ULONG _tls_index;
__declspec(dllexport) unsigned indice(void) { return (unsigned)_tls_index; }
__declspec(dllexport) unsigned long long su_bloque(void) { return ((unsigned long long *)__readgsqword(0x58))[_tls_index]; }
