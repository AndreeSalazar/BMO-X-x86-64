/* tanda15.c -- el .exe de la TANDA 15 de Cyberpunk (30-09): COM lo justo
 * (DURAS del censo). ole32: CoInitializeEx, CoInitializeSecurity,
 * CoCreateInstance, PropVariantClear, CoUninitialize, ReleaseStgMedium; y
 * OLEAUT32 importada por ORDINAL, como la importa el juego: los BSTR
 * (SysAllocString es el 2...) y los VARIANT.
 *
 * CoCreateInstance pide una clase que no existe en ningun sitio: en la casa
 * lo dice por la consola (es lo que un juego tiene que contar). Sale con el
 * numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned short WCHAR;
typedef unsigned long long U64;
typedef WCHAR *BSTR;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA HANDLE W GlobalAlloc(unsigned f, U64 n);
IMPORTA long W CoInitializeEx(void *r, DWORD f);
IMPORTA void W CoUninitialize(void);
IMPORTA long W CoInitializeSecurity(void *sd, long n, void *s, void *r, DWORD a, DWORD i, void *l, DWORD c, void *r3);
IMPORTA long W CoCreateInstance(const void *clsid, void *fuera, DWORD ctx, const void *iid, void **sale);
IMPORTA long W PropVariantClear(void *pv);
IMPORTA void *W CoTaskMemAlloc(U64 n);
IMPORTA void W ReleaseStgMedium(void *s);
IMPORTA BSTR W SysAllocString(const WCHAR *p);
IMPORTA BSTR W SysAllocStringLen(const WCHAR *p, unsigned n);
IMPORTA BSTR W SysAllocStringByteLen(const char *p, unsigned n);
IMPORTA int W SysReAllocString(BSTR *b, const WCHAR *p);
IMPORTA void W SysFreeString(BSTR b);
IMPORTA unsigned W SysStringLen(BSTR b);
IMPORTA unsigned W SysStringByteLen(BSTR b);
IMPORTA void W VariantInit(void *v);
IMPORTA long W VariantClear(void *v);

static unsigned fallos;

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

static const unsigned char NO_EXISTE[16] = {0x42, 0x4D, 0x4F, 0x58, 0x15, 0x15, 0x15, 0x15, 0xB0, 0x0B, 0x5A, 0x1E, 0x15, 0x15, 0x15, 0x15};
static const unsigned char IID_IUNKNOWN[16] = {0, 0, 0, 0, 0, 0, 0, 0, 0xC0, 0, 0, 0, 0, 0, 0, 0x46};

/* Un VARIANT / PROPVARIANT de x64: vt y 6 de relleno, y el valor en +8. */
typedef struct {
    unsigned short vt, r1, r2, r3;
    void *p;
    U64 resto;
} Variant;

void inicio(void) {
    /* -- iniciar COM */
    mira(CoInitializeEx(0, 0) == 0 && CoInitializeEx(0, 0) == 1, "CoInitializeEx: S_OK la primera vez, S_FALSE la segunda");
    mira(CoInitializeEx(0, 2) == (long)0x80010106, "CoInitializeEx con otro modelo: RPC_E_CHANGED_MODE");
    mira(CoInitializeSecurity(0, -1, 0, 0, 0, 3, 0, 0, 0) == 0 && CoInitializeSecurity(0, -1, 0, 0, 0, 3, 0, 0, 0) == (long)0x80010119, "CoInitializeSecurity: una vez; la segunda, RPC_E_TOO_LATE");
    {
        void *p = (void *)1;
        mira(CoCreateInstance(NO_EXISTE, 0, 1, IID_IUNKNOWN, &p) == (long)0x80040154 && p == 0, "CoCreateInstance de una clase que no hay: REGDB_E_CLASSNOTREG y NULL");
    }
    {
        Variant pv = {31, 0, 0, 0, 0, 0};
        WCHAR *t = (WCHAR *)CoTaskMemAlloc(4);
        t[0] = 'x', t[1] = 0;
        pv.p = t;
        mira(PropVariantClear(&pv) == 0 && pv.vt == 0 && pv.p == 0, "PropVariantClear de un VT_LPWSTR: suelto y a cero");
    }
    /* -- los BSTR (por ordinal) */
    {
        BSTR b = SysAllocString(L"hola");
        mira(b && SysStringLen(b) == 4 && SysStringByteLen(b) == 8 && ((DWORD *)b)[-1] == 8 && b[4] == 0 && b[0] == 'h', "SysAllocString: la medida en bytes delante, y el cero detras");
        mira(SysReAllocString(&b, L"adios") && SysStringLen(b) == 5 && b[4] == 's', "SysReAllocString");
        SysFreeString(b);
    }
    {
        BSTR b = SysAllocStringLen(L"abcdef", 3);
        BSTR c = SysAllocStringByteLen("xyz", 3);
        mira(b && SysStringLen(b) == 3 && b[2] == 'c' && b[3] == 0 && c && SysStringByteLen(c) == 3 && SysStringLen(c) == 1, "SysAllocStringLen y SysAllocStringByteLen");
        SysFreeString(b);
        SysFreeString(c);
    }
    {
        Variant v;
        v.vt = 99;
        VariantInit(&v);
        {
            int vacio = v.vt == 0;
            v.vt = 8; /* VT_BSTR */
            v.p = SysAllocString(L"q");
            mira(vacio && VariantClear(&v) == 0 && v.vt == 0, "VariantInit y VariantClear de un VT_BSTR");
        }
    }
    {
        struct {
            DWORD tymed, r;
            void *h;
            void *unk;
        } s = {1, 0, 0, 0};
        s.h = GlobalAlloc(2, 64);
        ReleaseStgMedium(&s);
        mira(1, "ReleaseStgMedium de un HGLOBAL: suelto, y se sigue");
    }
    /* -- y al final, fuera */
    CoUninitialize();
    CoUninitialize();
    mira(CoInitializeEx(0, 2) == 0, "CoUninitialize dos veces: el hilo puede volver a empezar, en otro modelo");
    CoUninitialize();
    di("tanda15.exe: COM lo justo dice lo de Windows\r\n");
    ExitProcess(fallos);
}
