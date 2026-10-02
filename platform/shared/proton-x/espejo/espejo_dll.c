/* ESPEJO -- la mitad Windows del diario de PROTON-X.
 *
 * No hay una biblioteca de enganche aqui.  Se recorren las importaciones PE
 * del .exe, se cambia cada ranura de su IAT por un salto de 20 bytes y ese
 * salto llega al trampolin comun de abajo.  El comun guarda todos los
 * argumentos Win64 antes de escribir la primera visita y despues salta a la
 * direccion de Windows que habia en la IAT.
 *
 * Paso 0 es a proposito solo el .exe principal.  Una DLL de juego puede
 * cargar mientras DllMain aun tiene el loader lock; recorrer esas DLL es el
 * paso 1, no una suposicion escondida en este fichero.
 */
typedef unsigned char U8;
typedef unsigned short U16;
typedef unsigned int U32;
typedef unsigned long long U64;
typedef unsigned long long SIZE_T;
typedef void *PVOID;
typedef PVOID HANDLE;
typedef int BOOL;
typedef const unsigned short *LPCWSTR;
typedef unsigned long DWORD;

#define WINAPI __stdcall
#define DLL_PROCESS_ATTACH 1
#define PAGE_READWRITE 4
#define PAGE_EXECUTE_READ 0x20
#define PAGE_EXECUTE_READWRITE 0x40
#define MEM_COMMIT 0x1000
#define MEM_RESERVE 0x2000
#define GENERIC_WRITE 0x40000000UL
#define FILE_SHARE_READ 1
#define CREATE_ALWAYS 2
#define OPEN_ALWAYS 4
#define FILE_ATTRIBUTE_NORMAL 0x80
#define FILE_END 2
#define INVALID_HANDLE_VALUE ((HANDLE)(long long)-1)

__declspec(dllimport) PVOID WINAPI VirtualAlloc(PVOID, SIZE_T, DWORD, DWORD);
__declspec(dllimport) BOOL WINAPI VirtualProtect(PVOID, SIZE_T, DWORD, DWORD *);
__declspec(dllimport) HANDLE WINAPI CreateFileW(LPCWSTR, DWORD, DWORD, PVOID, DWORD, DWORD, HANDLE);
__declspec(dllimport) BOOL WINAPI WriteFile(HANDLE, const void *, DWORD, DWORD *, PVOID);
__declspec(dllimport) BOOL WINAPI CloseHandle(HANDLE);
__declspec(dllimport) PVOID WINAPI GetModuleHandleW(LPCWSTR);
__declspec(dllimport) DWORD WINAPI GetModuleFileNameW(PVOID, unsigned short *, DWORD);

#pragma pack(push, 1)
struct Dos { U16 magic; U8 unused[58]; U32 pe; };
struct Coff { U16 machine, sections; U32 stamp, symbol_table, symbols; U16 optional, flags; };
struct Dir { U32 rva, size; };
/* En PE32+ los directorios empiezan exactamente en el byte 112 del optional
 * header.  No se modela lo que no se lee: asi no se desplaza sin querer. */
struct Optional64 { U16 magic; U8 unused[110]; struct Dir dirs[16]; };
struct Nt { U32 magic; struct Coff coff; struct Optional64 opt; };
struct Import { U32 attrs, stamp, forwarder, name, first; };
union Thunk { U64 address; U64 ordinal; U32 rva; };
#pragma pack(pop)

enum { PUESTOS = 4096 };
/* Par (nombre de DLL, nombre de funcion) por puesto. */
static U64 destinos[PUESTOS * 2];
static U8 vistos[PUESTOS];
static U32 puestos;
static U32 secuencia;
static HANDLE diario = INVALID_HANDLE_VALUE;
static U16 ruta_diario[32768];
static volatile U32 cerrojo_diario;
static volatile U32 cabecera_escrita;
static const char cabecera[] =
    "# ESPEJO de Windows: cada funcion de Windows, la PRIMERA vez que el .exe la llama\r\n"
    "# orden hilo dll funcion\r\n";

static void cerrar_cerrojo(void) {
    __atomic_store_n(&cerrojo_diario, 0, __ATOMIC_RELEASE);
}

static void abrir_cerrojo(void) {
    while (__atomic_exchange_n(&cerrojo_diario, 1, __ATOMIC_ACQUIRE)) {
        __asm__ __volatile__("pause");
    }
}

static void escribir_bajo_cerrojo(const char *texto, U32 n) {
    DWORD escritos = 0;
    if (diario == INVALID_HANDLE_VALUE) {
        diario = CreateFileW(ruta_diario, GENERIC_WRITE, FILE_SHARE_READ, 0,
                             CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, 0);
    }
    if (diario == INVALID_HANDLE_VALUE) return;
    if (!__atomic_exchange_n(&cabecera_escrita, 1, __ATOMIC_ACQ_REL)) {
        WriteFile(diario, cabecera, (DWORD)(sizeof(cabecera) - 1), &escritos, 0);
    }
    escritos = 0;
    WriteFile(diario, texto, n, &escritos, 0);
}

static void numero(char **p, U32 n) {
    char b[10]; U32 k = 0;
    do { b[k++] = (char)('0' + n % 10); n /= 10; } while (n);
    while (k) *(*p)++ = b[--k];
}

/* Llamada por el asm con los registros de la aplicacion ya a salvo. */
void primera(U32 i) {
    char linea[600]; char *p = linea;
    const char *dll = (const char *)(U64)destinos[i * 2];
    const char *funcion = (const char *)(U64)destinos[i * 2 + 1];
    if (__atomic_exchange_n(&vistos[i], 1, __ATOMIC_ACQ_REL)) return;
    abrir_cerrojo();
    numero(&p, ++secuencia); *p++ = ' '; numero(&p, 7); *p++ = ' ';
    while (*dll) *p++ = *dll++;
    *p++ = ' ';
    while (*funcion) *p++ = *funcion++;
    *p++ = '\r'; *p++ = '\n';
    escribir_bajo_cerrojo(linea, (U32)(p - linea));
    cerrar_cerrojo();
}

/* Cada puerta contiene: mov r11d, indice; mov rax, comun; jmp rax. */
void espejo_comun(void);

static void puerta(U8 *p, U32 i) {
    U64 comun = (U64)(PVOID)espejo_comun;
    p[0] = 0x41; p[1] = 0xbb; *(U32 *)(p + 2) = i;
    p[6] = 0x48; p[7] = 0xb8; *(U64 *)(p + 8) = comun;
    p[16] = 0xff; p[17] = 0xe0;
}

/* El indice entra en r11.  rax, r10 y r11 son volatiles; los argumentos de
 * enteros, los seis XMM y los argumentos de pila quedan intactos. */
/* El asm no puede indexar una estructura de pares: la mitad impar de
 * destinos guarda el nombre y esta tabla, solo el destino real. */
U64 destino_llamadas[PUESTOS];

/* Un marco Win64 con unwind metadata. 0x20 bytes son shadow space; el indice,
 * cuatro registros de argumentos y XMM0..5 ocupan el resto. */
__asm__(
    ".text\n"
    ".globl espejo_comun\n"
    ".seh_proc espejo_comun\n"
    "espejo_comun:\n"
    "subq $0xb8, %rsp\n"
    ".seh_stackalloc 0xb8\n"
    ".seh_endprologue\n"
    "movl %r11d, 0x20(%rsp)\n"
    "movq %rcx, 0x28(%rsp)\n" "movq %rdx, 0x30(%rsp)\n"
    "movq %r8, 0x38(%rsp)\n" "movq %r9, 0x40(%rsp)\n"
    "movdqu %xmm0, 0x48(%rsp)\n" "movdqu %xmm1, 0x58(%rsp)\n"
    "movdqu %xmm2, 0x68(%rsp)\n" "movdqu %xmm3, 0x78(%rsp)\n"
    "movdqu %xmm4, 0x88(%rsp)\n" "movdqu %xmm5, 0x98(%rsp)\n"
    "movl %r11d, %ecx\n" "callq primera\n"
    "movdqu 0x48(%rsp), %xmm0\n" "movdqu 0x58(%rsp), %xmm1\n"
    "movdqu 0x68(%rsp), %xmm2\n" "movdqu 0x78(%rsp), %xmm3\n"
    "movdqu 0x88(%rsp), %xmm4\n" "movdqu 0x98(%rsp), %xmm5\n"
    "movq 0x28(%rsp), %rcx\n" "movq 0x30(%rsp), %rdx\n"
    "movq 0x38(%rsp), %r8\n" "movq 0x40(%rsp), %r9\n"
    "movl 0x20(%rsp), %r11d\n"
    "addq $0xb8, %rsp\n"
    "leaq destino_llamadas(%rip), %r10\n"
    "jmpq *(%r10,%r11,8)\n"
    ".seh_endproc\n"
);

static void enganchar(PVOID modulo) {
    U8 *base = (U8 *)modulo;
    struct Dos *dos = (struct Dos *)base;
    struct Nt *nt;
    struct Import *imp;
    U8 *puertas;
    if (dos->magic != 0x5a4d) return;
    nt = (struct Nt *)(base + dos->pe);
    if (nt->magic != 0x4550 || nt->opt.magic != 0x20b) return;
    if (!nt->opt.dirs[1].rva || !nt->opt.dirs[1].size) return;
    imp = (struct Import *)(base + nt->opt.dirs[1].rva);
    puertas = (U8 *)VirtualAlloc(0, PUESTOS * 24, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
    if (!puertas) return;
    for (U32 i = 0; i < PUESTOS; ++i) puerta(puertas + i * 24, i);
    DWORD viejo_codigo;
    if (!VirtualProtect(puertas, PUESTOS * 24, PAGE_EXECUTE_READ, &viejo_codigo)) return;
    U32 max_desc = nt->opt.dirs[1].size / sizeof(*imp);
    for (U32 desc = 0; desc < max_desc && imp->name && puestos < PUESTOS; ++imp, ++desc) {
        if (!imp->first) continue;
        if (!imp->attrs) continue; /* sin INT los nombres ya se perdieron al cargar */
        union Thunk *nombres = (union Thunk *)(base + (imp->attrs ? imp->attrs : imp->first));
        union Thunk *iat = (union Thunk *)(base + imp->first);
        const char *dll = (const char *)(base + imp->name);
        for (; nombres->address && puestos < PUESTOS; ++nombres, ++iat) {
            const char *funcion;
            DWORD viejo;
            if (nombres->ordinal >> 63) continue;
            funcion = (const char *)(base + nombres->rva + 2);
            destinos[puestos * 2] = (U64)(PVOID)dll;
            destinos[puestos * 2 + 1] = (U64)(PVOID)funcion;
            destino_llamadas[puestos] = iat->address;
            if (VirtualProtect(iat, sizeof(*iat), PAGE_READWRITE, &viejo)) {
                iat->address = (U64)(PVOID)(puertas + puestos * 24);
                VirtualProtect(iat, sizeof(*iat), viejo, &viejo);
                puestos++;
            }
        }
    }
}

BOOL WINAPI DllMain(PVOID instancia, DWORD razon, PVOID reservado) {
    DWORD n;
    (void)instancia; (void)reservado;
    if (razon != DLL_PROCESS_ATTACH) return 1;
    n = GetModuleFileNameW(instancia, ruta_diario, 32767);
    if (n == 0 || n >= 32767) return 0;
    while (n && ruta_diario[n - 1] != '\\' && ruta_diario[n - 1] != '/') --n;
    if (n + 11 >= 32767) return 0;
    ruta_diario[n++] = 'E'; ruta_diario[n++] = 'S'; ruta_diario[n++] = 'P';
    ruta_diario[n++] = 'E'; ruta_diario[n++] = 'J'; ruta_diario[n++] = 'O';
    ruta_diario[n++] = '.'; ruta_diario[n++] = 'T'; ruta_diario[n++] = 'X';
    ruta_diario[n++] = 'T'; ruta_diario[n] = 0;
    enganchar(GetModuleHandleW(0));
    return puestos != 0;
}
