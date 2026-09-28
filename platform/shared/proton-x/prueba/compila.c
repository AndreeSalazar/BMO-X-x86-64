/* compila.c -- el .exe de P3c2 de PROTON-X: D3DCompile, como lo llama el
 * cubo de BMOX-12 (el HLSL de su estudio_d3d12, vs_5_0 y ps_5_0).
 *
 *    en Windows   compila de verdad (d3dcompiler_47): sale un DXBC
 *    en BMO-X     la primera vez no hay compilador: E_FAIL y un blob de
 *                 errores que dice que falta y como (sombras.exe en Windows);
 *                 deja window/sombras/<huella>.hls y .ent. Cuando esta el
 *                 .cso, sale ese DXBC
 *
 * Las dos cosas son `bien`: lo que se comprueba es que cada camino diga la
 * verdad. Sale con el numero de fallos. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned long long U64;

#define IMPORTA __declspec(dllimport)
#define WINAPI __stdcall
IMPORTA HANDLE WINAPI GetStdHandle(DWORD n);
IMPORTA int WINAPI WriteFile(HANDLE h, const void *b, DWORD n, DWORD *escritos, void *ov);
IMPORTA void WINAPI ExitProcess(unsigned int c);
/* d3dcompiler_47.dll */
IMPORTA long WINAPI D3DCompile(const void *src, U64 n, const char *nombre, const void *macros, void *include, const char *entrada, const char *perfil, unsigned f1, unsigned f2, void **codigo, void **errores);

/* Un ID3DBlob por su vtabla: Release (2), GetBufferPointer (3), GetBufferSize (4). */
typedef struct { void **vt; } BLOB;
typedef unsigned long(WINAPI *RELEASE)(void *);
typedef void *(WINAPI *PUNTERO)(void *);
typedef U64(WINAPI *MEDIDA)(void *);
static const unsigned char *bytes(void *b) { return ((PUNTERO)((BLOB *)b)->vt[3])(b); }
static U64 medida(void *b) { return ((MEDIDA)((BLOB *)b)->vt[4])(b); }
static void soltar(void *b) { if (b) ((RELEASE)((BLOB *)b)->vt[2])(b); }

static const char HLSL[] =
    "cbuffer Constantes : register(b0) { float4x4 WVP; float4x4 World; float4 Luz; };\n"
    "struct VSIn { float3 pos : POSITION; float3 nrm : NORMAL; float4 col : COLOR; };\n"
    "struct PSIn { float4 pos : SV_POSITION; float3 nrm : NORMAL; float4 col : COLOR; };\n"
    "PSIn VSMain(VSIn i) { PSIn o; o.pos = mul(WVP, float4(i.pos, 1.0)); o.nrm = mul((float3x3)World, i.nrm); o.col = i.col; return o; }\n"
    "float4 PSMain(PSIn i) : SV_TARGET { float d = saturate(dot(normalize(i.nrm), Luz.xyz)); return float4(i.col.rgb * (Luz.w + (1.0 - Luz.w) * d), i.col.a); }\n";

static HANDLE salida;
static unsigned fallos;

static void di(const char *s) {
    DWORD n = 0, e;
    while (s[n]) n++;
    WriteFile(salida, s, n, &e, 0);
}

static void hex(U64 v) {
    char b[19];
    int i;
    b[0] = '0';
    b[1] = 'x';
    for (i = 0; i < 16; i++) {
        unsigned d = (unsigned)(v >> (60 - 4 * i)) & 15;
        b[2 + i] = (char)(d < 10 ? '0' + d : 'a' + d - 10);
    }
    b[18] = 0;
    di(b);
}

static void mira(int bien, const char *que, U64 valor) {
    di(bien ? "  bien  " : "  MAL   ");
    di(que);
    di(" ");
    hex(valor);
    di("\r\n");
    if (!bien) fallos++;
}

/* `t` (de `n` bytes) contiene `p`. */
static int contiene(const unsigned char *t, U64 n, const char *p) {
    U64 i, j;
    for (i = 0; i < n; i++) {
        for (j = 0; p[j] && i + j < n && t[i + j] == (unsigned char)p[j]; j++) {
        }
        if (!p[j]) return 1;
    }
    return 0;
}

static void uno(const char *entrada, const char *perfil) {
    void *codigo = 0, *errores = 0;
    long r = D3DCompile(HLSL, sizeof HLSL - 1, "cubo.hlsl", 0, 0, entrada, perfil, 0, 0, &codigo, &errores);
    if (r == 0) {
        const unsigned char *b = codigo ? bytes(codigo) : 0;
        mira(b && medida(codigo) >= 32 && b[0] == 'D' && b[1] == 'X' && b[2] == 'B' && b[3] == 'C', perfil[0] == 'v' ? "D3DCompile(VSMain, vs_5_0): un DXBC" : "D3DCompile(PSMain, ps_5_0): un DXBC", codigo ? medida(codigo) : 0);
    } else {
        mira((unsigned long)r == 0x80004005 && errores && !codigo && contiene(bytes(errores), medida(errores), "sombras"),
             perfil[0] == 'v' ? "D3DCompile(VSMain) sin compilador: E_FAIL, y dice que falta y como" : "D3DCompile(PSMain) sin compilador: E_FAIL, y dice que falta y como", (unsigned long)r);
    }
    soltar(codigo);
    soltar(errores);
}

void inicio(void) {
    salida = GetStdHandle((DWORD)-11);
    di("compila.exe: D3DCompile, como lo llama el cubo de BMOX-12\r\n");
    uno("VSMain", "vs_5_0");
    uno("PSMain", "ps_5_0");
    di(fallos ? "compila.exe: ALGO NO es como tiene que ser\r\n" : "compila.exe: D3DCompile dice la verdad\r\n");
    ExitProcess(fallos);
}
