/* lanzador.c -> espejo.exe (PASO 0b, CANDIDATO de main, sin revisar ni probar): lanza un .exe suspendido, carga espejo.dll dentro y solo
 * entonces deja correr al juego.  No modifica ni el juego ni BMO-X. */
typedef void *PVOID; typedef PVOID HANDLE; typedef unsigned long DWORD;
typedef unsigned long long SIZE_T; typedef int BOOL; typedef unsigned short WCHAR;
#define WINAPI __stdcall
#define CREATE_SUSPENDED 4
#define MEM_COMMIT 0x1000
#define MEM_RESERVE 0x2000
#define PAGE_READWRITE 4
#define INFINITE 0xffffffffUL
/* STARTUPINFOW: las ocho DWORD tras los tres punteros importan; omitir
 * dwFillAttribute desplaza hStdInput y CreateProcessW escribe fuera. */
struct Startup { DWORD cb; WCHAR *r; WCHAR *d; WCHAR *t; DWORD x,y,cx,cy,fx,fy,fill,flags; unsigned short show,cb2; void *r2; HANDLE in,out,err; };
struct Process { HANDLE process, thread; DWORD pid, tid; };
static WCHAR juego[2048], linea[4096], dll[2048], carpeta_juego[2048];
static struct Startup si;
static struct Process pi;
__declspec(dllimport) BOOL WINAPI CreateProcessW(const WCHAR *, WCHAR *, PVOID, PVOID, BOOL, DWORD, PVOID, const WCHAR *, struct Startup *, struct Process *);
__declspec(dllimport) PVOID WINAPI VirtualAllocEx(HANDLE, PVOID, SIZE_T, DWORD, DWORD);
__declspec(dllimport) BOOL WINAPI WriteProcessMemory(HANDLE, PVOID, const void *, SIZE_T, SIZE_T *);
__declspec(dllimport) HANDLE WINAPI CreateRemoteThread(HANDLE, PVOID, SIZE_T, PVOID, PVOID, DWORD, DWORD *);
__declspec(dllimport) DWORD WINAPI WaitForSingleObject(HANDLE, DWORD);
__declspec(dllimport) BOOL WINAPI GetExitCodeThread(HANDLE, DWORD *);
__declspec(dllimport) DWORD WINAPI ResumeThread(HANDLE);
__declspec(dllimport) BOOL WINAPI CloseHandle(HANDLE);
__declspec(dllimport) BOOL WINAPI TerminateProcess(HANDLE, unsigned);
__declspec(dllimport) DWORD WINAPI GetModuleFileNameW(PVOID, WCHAR *, DWORD);
__declspec(dllimport) PVOID WINAPI GetProcAddress(PVOID, const char *);
__declspec(dllimport) PVOID WINAPI GetModuleHandleW(const WCHAR *);
__declspec(dllimport) WCHAR * WINAPI GetCommandLineW(void);

static unsigned wlen(const WCHAR *s) { unsigned n=0; while(s[n]) n++; return n; }
static void copia(WCHAR *d, const WCHAR *s) { while ((*d++=*s++)); }
void inicio(void) {
    WCHAR *cmd = GetCommandLineW();
    HANDLE hilo; DWORD codigo; SIZE_T fuera;
    unsigned i=0, j=0, n;
    while (cmd[i] && cmd[i] != ' ') i++;
    while (cmd[i] == ' ') i++;
    if (cmd[i] == '"') {
        i++;
        while (cmd[i] && cmd[i] != '"' && j < 2047) juego[j++] = cmd[i++];
    } else {
        while (cmd[i] && cmd[i] != ' ' && j < 2047) juego[j++] = cmd[i++];
    }
    juego[j] = 0;
    if (!j || (j == 2047 && cmd[i])) return;
    linea[0]='"'; copia(linea+1,juego); n=wlen(linea); linea[n++]='"'; linea[n]=0;
    n=GetModuleFileNameW(0,dll,2048); while(n && dll[n-1] != '\\' && dll[n-1] != '/') n--;
    copia(dll+n,(const WCHAR[]){'e','s','p','e','j','o','.','d','l','l',0});
    copia(carpeta_juego,juego);
    n = wlen(carpeta_juego);
    while (n && carpeta_juego[n - 1] != '\\' && carpeta_juego[n - 1] != '/') --n;
    carpeta_juego[n] = 0;
    si.cb=sizeof(si);
    if (!CreateProcessW(juego,linea,0,0,0,CREATE_SUSPENDED,0,n ? carpeta_juego : 0,&si,&pi)) return;
    PVOID remoto=VirtualAllocEx(pi.process,0,(wlen(dll)+1)*2,MEM_COMMIT|MEM_RESERVE,PAGE_READWRITE);
    if (!remoto || !WriteProcessMemory(pi.process,remoto,dll,(wlen(dll)+1)*2,&fuera) || fuera != (wlen(dll)+1)*2) goto fallo;
    hilo=CreateRemoteThread(pi.process,0,0,GetProcAddress(GetModuleHandleW((const WCHAR[]){'k','e','r','n','e','l','3','2','.','d','l','l',0}),"LoadLibraryW"),remoto,0,0);
    if (!hilo) goto fallo;
    if (WaitForSingleObject(hilo,INFINITE) != 0 || !GetExitCodeThread(hilo,&codigo) || !codigo) {
        CloseHandle(hilo);
        goto fallo;
    }
    CloseHandle(hilo);
    if (ResumeThread(pi.thread) == INFINITE) goto fallo;
    CloseHandle(pi.thread);
    CloseHandle(pi.process);
    return;
fallo:
    TerminateProcess(pi.process, 0xE040);
    WaitForSingleObject(pi.process, INFINITE);
    CloseHandle(pi.thread);
    CloseHandle(pi.process);
}
