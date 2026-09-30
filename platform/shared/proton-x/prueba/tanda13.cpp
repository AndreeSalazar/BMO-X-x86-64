// tanda13.cpp -- el .exe de la TANDA 13 de Cyberpunk (30-09): lo que LANZA
// msvcp140.dll (_Xlength_error, _Xout_of_range, _Xinvalid_argument,
// _Xbad_alloc, _Xbad_function_call, _Throw_Cpp_error, _Throw_C_error,
// _Throw_future_error), cogido por el NOMBRE de su clase de MSVC como lo
// coge el juego; exception_ptr (guardar y relanzar, una de msvcp y una
// propia), uncaught_exceptions en un destructor que desenrolla, y _Lockit.
//
// Sin la biblioteca de C++: las clases se DECLARAN (solo hace falta su
// nombre para el catch, y sus miembros donde MSVC los pone). Sale con el
// numero de fallos. En Windows dice lo mismo.
typedef void *HANDLE;
typedef unsigned long DWORD;
#define IMPORTA extern "C" __declspec(dllimport)
IMPORTA HANDLE __stdcall GetStdHandle(DWORD n);
IMPORTA int __stdcall WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void __stdcall ExitProcess(unsigned c);

#if defined(__clang__)
extern const void *const vftable_type_info __asm__("??_7type_info@@6B@");
const void *const vftable_type_info = nullptr;
void operator delete(void *) noexcept {}
void operator delete(void *, unsigned long long) noexcept {}
#endif

namespace std {
// Como las pone MSVC: vptr, el texto y su bandera (24 bytes).
class exception {
  public:
    virtual ~exception();
    virtual const char *what() const;
    const char *_Texto;
    bool _Libre;
};
class logic_error : public exception {};
class runtime_error : public exception {};
class length_error : public logic_error {};
class out_of_range : public logic_error {};
class invalid_argument : public logic_error {};
class bad_alloc : public exception {};
class bad_function_call : public exception {};
class error_code {
  public:
    int _Valor;
    const void *_Categoria;
};
class _Lockit {
  public:
    __declspec(dllimport) explicit _Lockit(int);
    __declspec(dllimport) ~_Lockit();
    int _Locktype;
};
[[noreturn]] __declspec(dllimport) void __cdecl _Xlength_error(const char *);
[[noreturn]] __declspec(dllimport) void __cdecl _Xout_of_range(const char *);
[[noreturn]] __declspec(dllimport) void __cdecl _Xinvalid_argument(const char *);
[[noreturn]] __declspec(dllimport) void __cdecl _Xbad_alloc();
[[noreturn]] __declspec(dllimport) void __cdecl _Xbad_function_call();
[[noreturn]] __declspec(dllimport) void __cdecl _Throw_Cpp_error(int);
[[noreturn]] __declspec(dllimport) void __cdecl _Throw_C_error(int);
[[noreturn]] __declspec(dllimport) void __cdecl _Throw_future_error(const error_code &);
__declspec(dllimport) int __cdecl uncaught_exceptions() noexcept;
__declspec(dllimport) bool __cdecl uncaught_exception() noexcept;
} // namespace std

__declspec(dllimport) void __cdecl __ExceptionPtrCreate(void *);
__declspec(dllimport) void __cdecl __ExceptionPtrDestroy(void *);
__declspec(dllimport) void __cdecl __ExceptionPtrCopy(void *, const void *);
__declspec(dllimport) bool __cdecl __ExceptionPtrToBool(const void *);
__declspec(dllimport) void __cdecl __ExceptionPtrCurrentException(void *);
[[noreturn]] __declspec(dllimport) void __cdecl __ExceptionPtrRethrow(const void *);

static unsigned fallos;

static void di(const char *t) {
    DWORD n = 0, k = 0;
    while (t[n])
        n++;
    WriteFile(GetStdHandle((DWORD)-11), t, n, &k, 0);
}

static void mira(bool bien, const char *que) {
    if (!bien)
        fallos++;
    di(bien ? "  bien  " : "  MAL   ");
    di(que);
    di("\r\n");
}

static bool igual(const char *a, const char *b) {
    while (*a && *a == *b)
        a++, b++;
    return *a == *b;
}

// Una propia, con su constructor de copia (que cuenta).
static int copias;
struct Mia {
    int v;
    explicit Mia(int v) : v(v) {}
    Mia(const Mia &o) : v(o.v) { copias++; }
};

static int durante;
struct Mira {
    ~Mira() { durante = std::uncaught_exceptions(); }
};

// Las del error_code de MSVC: el valor y la categoria (+24 y +32).
static int valor(const std::exception &e) { return *(const int *)((const char *)&e + 24); }
static const void *categoria(const std::exception &e) { return *(const void *const *)((const char *)&e + 32); }
typedef const char *(*Nombre)(const void *);

extern "C" void inicio() {
    bool ok = false;
    try {
        std::_Xlength_error("largo de mas");
    } catch (std::length_error &e) {
        ok = igual(e.what(), "largo de mas");
    }
    mira(ok, "_Xlength_error: std::length_error, con su texto");
    ok = false;
    try {
        std::_Xout_of_range("fuera");
    } catch (std::invalid_argument &) {
    } catch (std::logic_error &e) {
        ok = igual(e.what(), "fuera");
    }
    mira(ok, "_Xout_of_range: lo coge std::logic_error (y no invalid_argument)");
    ok = false;
    try {
        std::_Xinvalid_argument("malo");
    } catch (std::exception &e) {
        ok = igual(e.what(), "malo");
    }
    mira(ok, "_Xinvalid_argument: lo coge std::exception");
    ok = false;
    try {
        std::_Xbad_alloc();
    } catch (std::bad_alloc &e) {
        ok = igual(e.what(), "bad allocation");
    }
    mira(ok, "_Xbad_alloc: \"bad allocation\"");
    ok = false;
    try {
        std::_Xbad_function_call();
    } catch (std::bad_function_call &e) {
        ok = igual(e.what(), "bad function call");
    }
    mira(ok, "_Xbad_function_call: \"bad function call\"");
    ok = false;
    try {
        std::_Throw_Cpp_error(4);
    } catch (std::runtime_error &e) {
        const void *c = categoria(e);
        Nombre nombre = (*(Nombre *const *)c)[1];
        ok = valor(e) == 1 && igual(e.what(), "operation not permitted") && igual(nombre(c), "generic") && *(const unsigned long long *)((const char *)c + 8) == 3;
    }
    mira(ok, "_Throw_Cpp_error: un system_error de errc 1, categoria \"generic\" (_Addr 3)");
    ok = false;
    try {
        std::_Throw_C_error(3);
    } catch (std::runtime_error &e) {
        ok = valor(e) == 16;
    }
    mira(ok, "_Throw_C_error(_Thrd_busy): device_or_resource_busy");
    ok = false;
    try {
        std::error_code ec = {1, nullptr};
        std::_Throw_future_error(ec);
    } catch (std::logic_error &e) {
        ok = valor(e) == 1 && igual(e.what(), "broken promise");
    }
    mira(ok, "_Throw_future_error: future_error \"broken promise\"");

    // -- uncaught_exceptions, mientras se desenrolla
    durante = -1;
    try {
        Mira m;
        std::_Xout_of_range("x");
    } catch (...) {
    }
    mira(durante == 1 && std::uncaught_exceptions() == 0 && !std::uncaught_exception(), "uncaught_exceptions: 1 al desenrollar, 0 despues");

    // -- exception_ptr
    void *p[2], *q[2], *r[2];
    __ExceptionPtrCreate(r);
    mira(!__ExceptionPtrToBool(r), "__ExceptionPtrCreate: vacio");
    try {
        std::_Xout_of_range("guardada");
    } catch (...) {
        __ExceptionPtrCurrentException(p);
    }
    __ExceptionPtrCopy(q, p);
    __ExceptionPtrDestroy(p);
    ok = false;
    try {
        __ExceptionPtrRethrow(q);
    } catch (std::out_of_range &e) {
        ok = igual(e.what(), "guardada");
    }
    mira(__ExceptionPtrToBool(q) && ok, "current_exception y rethrow_exception de una de msvcp (tras copiar y soltar)");
    __ExceptionPtrDestroy(q);
    try {
        throw Mia(41);
    } catch (...) {
        __ExceptionPtrCurrentException(p);
    }
    int visto = 0;
    try {
        __ExceptionPtrRethrow(p);
    } catch (Mia &m) {
        visto = m.v;
    }
    mira(visto == 41 && copias >= 1, "exception_ptr de una propia: se copia con su constructor y se relanza");
    __ExceptionPtrDestroy(p);

    // -- _Lockit
    {
        std::_Lockit a(0);
        std::_Lockit b(0);
        mira(a._Locktype == 0 && b._Locktype == 0, "_Lockit: dos del mismo tipo, uno dentro de otro");
    }
    di("tanda13.exe: lo que lanza msvcp140 es lo de Windows\r\n");
    ExitProcess(fallos);
}
