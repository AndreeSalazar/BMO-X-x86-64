// tanda40.exe -- el juez del paso 0 de ESPEJO.
//
// Carga espejo.dll (que engancha la IAT de ESTE .exe) y despues llama a cinco
// funciones conocidas de kernel32, en orden. Bajo ESPEJO esas cinco salen en
// ESPEJO.TXT; el mismo .exe en la casa de PROTON-X sale en el DIARIO. Los dos
// ficheros se comparan con `espejo-compara`.
//
// Sin CRT, reproducible (lld-link /brepro), como las demas tandas.

typedef unsigned short u16;
typedef unsigned int u32;
typedef unsigned long long u64;
#define WINAPI __attribute__((ms_abi))

extern WINAPI void *LoadLibraryW(const u16 *);
extern WINAPI u32 GetCurrentProcessId(void);
extern WINAPI u32 GetCurrentThreadId(void);
extern WINAPI u64 GetTickCount64(void);
extern WINAPI void GetSystemTimeAsFileTime(u64 *);
extern WINAPI int QueryPerformanceCounter(u64 *);
extern WINAPI void ExitProcess(u32);

// "espejo.dll" en UTF-16, a mano (sin CRT no hay literal ancho portatil).
static const u16 ESPEJO[] = { 'e','s','p','e','j','o','.','d','l','l', 0 };

void WINAPI inicio(void) {
    // Primero el espejo: engancha la IAT antes de que llamemos a las cinco.
    LoadLibraryW(ESPEJO);

    u32 acc = 0;
    acc ^= GetCurrentProcessId();
    acc ^= GetCurrentThreadId();
    acc ^= (u32)GetTickCount64();
    u64 ft = 0; GetSystemTimeAsFileTime(&ft); acc ^= (u32)ft;
    u64 pc = 0; QueryPerformanceCounter(&pc); acc ^= (u32)pc;

    // El codigo de salida no es determinista (reloj y PID), asi que se
    // normaliza a 0: lo que juzga este .exe es ESPEJO.TXT, no su salida.
    (void)acc;
    ExitProcess(0);
}
