/* tanda41.c -- el .exe de la TANDA 41 (02-10): cuantos procesadores, dicho
 * IGUAL por todas partes. Por Cyberpunk: monta una cola de trabajo por
 * nucleo sin contar el principal y vacia "la ultima" con `cuantas - 1`; la
 * casa decia UN procesador, le salian cero colas y leia tabla[-1]
 * (Cyberpunk2077.exe+0x24d9a9 -> +0x24d8b3). Aqui, como en Windows:
 * GetSystemInfo, su mascara, el PEB, GetProcessAffinityMask, los nucleos y
 * el grupo de GetLogicalProcessorInformationEx, los nucleos de
 * GetLogicalProcessorInformation y los CPU sets dicen el MISMO numero, los
 * nucleos no se pisan y tienen SMT si tienen mas de uno, las caches caen
 * dentro, y hay al menos dos (la pared). No depende de que CPU haya.
 *
 * Sale con el numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned long long U64;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA void W GetSystemInfo(void *si);
IMPORTA HANDLE W GetCurrentProcess(void);
IMPORTA int W GetProcessAffinityMask(HANDLE h, U64 *proceso, U64 *sistema);
IMPORTA int W GetLogicalProcessorInformation(void *b, DWORD *n);
IMPORTA int W GetLogicalProcessorInformationEx(int rel, void *b, DWORD *n);
IMPORTA int W GetSystemCpuSetInformation(void *b, DWORD n, DWORD *dev, HANDLE p, DWORD banderas);

static unsigned fallos;
static unsigned char si[48], b[16384];

static void di(const char *t) {
    DWORD n = 0, k = 0;
    while (t[n])
        n++;
    WriteFile(GetStdHandle((DWORD)-11), t, n, &k, 0);
}

static void mira(int bien, const char *que) {
    if (!bien)
        fallos++;
    di(bien ? "  bien  " : "  MAL   ");
    di(que);
    di("\r\n");
}

static unsigned bits(U64 m) {
    unsigned n = 0;
    while (m) {
        n += (unsigned)(m & 1);
        m >>= 1;
    }
    return n;
}

static DWORD d32(const unsigned char *p) { return *(const DWORD *)p; }
static U64 d64(const unsigned char *p) { return *(const U64 *)p; }

void inicio(void);

void inicio(void) {
    DWORD n, largo, i, nucleos = 0, logicos = 0, smt_bien = 1, caches_bien = 1;
    U64 mascara, proceso = 0, sistema = 0, juntos = 0, peb;
    int pisan = 0, r;

    GetSystemInfo(si);
    n = d32(si + 32);
    mascara = d64(si + 24);
    mira(n >= 2, "GetSystemInfo: al menos dos procesadores (la pared de Cyberpunk)");
    mira(bits(mascara) == n, "y su mascara tiene tantos bits como procesadores");
    __asm__ volatile("movq %%gs:0x60, %0" : "=r"(peb));
    mira(*(const DWORD *)(peb + 0xB8) == n, "el PEB dice los mismos (NumberOfProcessors)");
    r = GetProcessAffinityMask(GetCurrentProcess(), &proceso, &sistema);
    mira(r && sistema == mascara && proceso == mascara, "GetProcessAffinityMask: la misma mascara");

    largo = sizeof b;
    r = GetLogicalProcessorInformationEx(0, b, &largo); /* RelationProcessorCore */
    for (i = 0; r && i + 8 <= largo; i += d32(b + i + 4)) {
        U64 m = d64(b + i + 32); /* GroupMask[0].Mask */
        unsigned k = bits(m);
        nucleos++;
        logicos += k;
        pisan |= (juntos & m) != 0;
        juntos |= m;
        smt_bien &= (b[i + 8] == 1) == (k > 1); /* Flags: LTP_PC_SMT */
        if (!d32(b + i + 4))
            break;
    }
    mira(r && nucleos >= 1 && logicos == n && !pisan && juntos == mascara, "Ex: los nucleos suman los mismos y no se pisan");
    mira(r && smt_bien, "Ex: SMT si y solo si el nucleo tiene mas de un logico");

    largo = sizeof b;
    r = GetLogicalProcessorInformationEx(4, b, &largo); /* RelationGroup */
    mira(r && b[8 + 24 + 1] == n && d64(b + 8 + 24 + 40) == mascara, "Ex: el grupo 0 tiene los mismos activos y la mascara");

    largo = sizeof b;
    r = GetLogicalProcessorInformationEx(2, b, &largo); /* RelationCache */
    for (i = 0; r && i + 8 <= largo; i += d32(b + i + 4)) {
        U64 m = d64(b + i + 40); /* tras 12 + 18 + GroupCount */
        unsigned nivel = b[i + 8];
        caches_bien &= nivel >= 1 && nivel <= 4 && m && (m & ~mascara) == 0;
        if (!d32(b + i + 4))
            break;
    }
    mira(r && caches_bien, "Ex: cada cache con su nivel y dentro de la mascara");

    largo = sizeof b;
    r = GetLogicalProcessorInformation(b, &largo);
    {
        DWORD k = 0;
        for (i = 0; r && i + 32 <= largo; i += 32)
            k += d32(b + i + 8) == 0;
        mira(r && k == nucleos, "GetLogicalProcessorInformation: los mismos nucleos");
    }

    largo = 0;
    GetSystemCpuSetInformation(b, sizeof b, &largo, GetCurrentProcess(), 0);
    {
        DWORD k = 0;
        for (i = 0; i + 32 <= largo; i += d32(b + i)) {
            if (d32(b + i + 4) == 0) /* CpuSetInformation */
                k++;
            if (!d32(b + i))
                break;
        }
        mira(k == n, "GetSystemCpuSetInformation: un CPU set por procesador");
    }
    di("tanda41.exe: cuantos procesadores, igual por todas partes\r\n");
    ExitProcess(fallos);
}
