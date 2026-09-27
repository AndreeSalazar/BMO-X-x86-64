/* hola.c -- el primer .exe de PROTON-X (P1). Sin CRT: tres funciones de
 * kernel32 y nada mas. El puntero `mensaje` es ABSOLUTO a proposito: obliga al
 * enlazador a dejar una relocalizacion (.reloc), y el cargador tiene que
 * aplicarla si carga el .exe en otra base. */
typedef void *HANDLE;
typedef unsigned long DWORD;
__declspec(dllimport) HANDLE __stdcall GetStdHandle(DWORD n);
__declspec(dllimport) int __stdcall WriteFile(HANDLE h, const void *b, DWORD n, DWORD *escritos, void *ov);
__declspec(dllimport) void __stdcall ExitProcess(unsigned int codigo);

static const char texto[] = "hola desde un .exe de Windows\r\n";
const char *const mensaje = texto;

void inicio(void) {
    DWORD escritos = 0;
    const char *const volatile *p = &mensaje;
    WriteFile(GetStdHandle((DWORD)-11), *p, sizeof texto - 1, &escritos, 0);
    ExitProcess(escritos == sizeof texto - 1 ? 0 : 1);
}
