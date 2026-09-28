/* saludo.c -- la DLL de P5a de PROTON-X: lo que un juego trae ademas de su
 * .exe. Exporta tres funciones con ordinales FIJOS (saludo.def), importa de
 * kernel32 (las importaciones de una DLL tambien se resuelven) y su DllMain
 * deja una marca: tiene que correr ANTES que la entrada del .exe.
 *
 * La usa usadll.exe. Las dos van juntas en la misma carpeta (window/, o
 * donde se lancen en Windows). */
typedef unsigned long DWORD;
typedef void *HANDLE;
#define IMPORTA __declspec(dllimport)
#define WINAPI __stdcall
IMPORTA DWORD WINAPI GetCurrentProcessId(void);

static int adjunta;
static DWORD proceso;

int suma(int a, int b) { return a + b; }
const char *frase(void) { return "hola desde saludo.dll"; }
/* 1 si DllMain vio DLL_PROCESS_ATTACH, y el id del proceso que leyo alli. */
int visto_attach(DWORD *pid) { *pid = proceso; return adjunta; }

int WINAPI entrada(HANDLE yo, DWORD motivo, void *r) {
    (void)yo;
    (void)r;
    if (motivo == 1) {
        adjunta++;
        proceso = GetCurrentProcessId();
    }
    return 1;
}
