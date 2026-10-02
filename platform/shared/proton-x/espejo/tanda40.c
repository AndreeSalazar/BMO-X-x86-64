/* TANDA 40: las cinco importaciones que Paso 0 debe dejar en ESPEJO.TXT. */
typedef void *HANDLE; typedef unsigned long DWORD;
#define I __declspec(dllimport)
#define W __stdcall
I HANDLE W GetStdHandle(DWORD); I int W WriteFile(HANDLE,const void *,DWORD,DWORD *,void *);
I DWORD W GetCurrentProcessId(void); I DWORD W GetLastError(void); I void W ExitProcess(unsigned);
static volatile DWORD visto;
void inicio(void) { DWORD n; const char t[]="tanda40: bien\r\n"; HANDLE h=GetStdHandle((DWORD)-11); WriteFile(h,t,sizeof(t)-1,&n,0); visto=GetCurrentProcessId(); visto=GetLastError(); ExitProcess(0); }
