// tanda20.cpp -- el .exe de la TANDA 20 de Cyberpunk (30-09): las ultimas
// DURAS del censo del metal. De msvcp140: << float (con setprecision, fixed
// y setw), << const void*, << long long; istream::read, seekg y tellg sobre
// un streambuf propio que se deja mover; _Fiopen de un nombre ANCHO; y el
// constructor de Concurrency::task_continuation_context. Y el API set de
// CFGMGR32 (api-ms-win-devices-config-l1-1-1).
//
// Las declaraciones son las de tanda19.cpp (las clases con sus miembros
// donde MSVC los pone), con lo nuevo. Sale con el numero de fallos. En
// Windows dice lo mismo. Su gemela de MSVC, con las cabeceras de verdad, es
// tanda20m.cpp.
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned long long size_t;
struct _iobuf;
typedef struct _iobuf FILE;
#define IMPORTA extern "C" __declspec(dllimport)
IMPORTA HANDLE __stdcall GetStdHandle(DWORD n);
IMPORTA int __stdcall WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void __stdcall ExitProcess(unsigned c);
IMPORTA DWORD __stdcall GetModuleFileNameA(void *m, char *b, DWORD n);
IMPORTA DWORD __stdcall GetModuleFileNameW(void *m, wchar_t *b, DWORD n);
IMPORTA DWORD __stdcall CM_Get_Device_ID_List_SizeW(DWORD *n, const wchar_t *filtro, DWORD f);
IMPORTA DWORD __stdcall CM_MapCrToWin32Err(DWORD cr, DWORD por_defecto);
IMPORTA int __cdecl fclose(FILE *f);
IMPORTA long __cdecl ftell(FILE *f);

extern "C" int _fltused = 0; // lo pide clang al usar float, sin CRT

void operator delete(void *) noexcept {}
void operator delete(void *, size_t) noexcept {}
inline void *operator new(size_t, void *p) noexcept { return p; }

struct _Mbstatet {
    unsigned long _Wchar;
    unsigned short _Byte, _State;
};
struct tm {
    int tm_sec, tm_min, tm_hour, tm_mday, tm_mon, tm_year, tm_wday, tm_yday, tm_isdst;
};

struct Amiga; // la que llega a lo privado de locale

namespace std {
template <class E> struct char_traits;
template <> struct char_traits<char> {};

class _Facet_base {
  public:
    virtual ~_Facet_base() noexcept {}
    virtual void _Incref() noexcept = 0;
    virtual _Facet_base *_Decref() noexcept = 0;
};

// Como en MSVC: dos bases vacias, y solo la primera no ocupa. std::locale
// mide 16 bytes y su _Locimp esta en el +8.
template <class T> class _Locbase {};
struct _Crt_new_delete {};

class locale : public _Locbase<int>, public _Crt_new_delete {
  public:
    class facet : public _Facet_base {
      public:
        __declspec(dllimport) void _Incref() noexcept override;
        __declspec(dllimport) _Facet_base *_Decref() noexcept override;
        unsigned long _Myrefs;
    };
    class _Locimp : public facet {};
    ~locale() noexcept {
        if (_Ptr)
            delete _Ptr->_Decref();
    }
    _Locimp *_Ptr;

  private:
    __declspec(dllimport) static _Locimp *_Getgloballocale();
    friend struct ::Amiga;
};

template <class S> class fpos {
  public:
    long long _Myoff;
    long long _Fpos;
    S _Mystate;
};

class ios_base {
  public:
    int rdstate() const { return _Mystate; }
    bool good() const { return _Mystate == 0; }
    int flags() const { return _Fmtfl; }
    int setf(int f) {
        int o = _Fmtfl;
        _Fmtfl |= f;
        return o;
    }
    int setf(int f, int m) {
        int o = _Fmtfl;
        _Fmtfl = (_Fmtfl & ~m) | (f & m);
        return o;
    }
    void unsetf(int m) { _Fmtfl &= ~m; }
    long long width() const { return _Wide; }
    __declspec(dllimport) locale getloc() const;
    virtual ~ios_base() noexcept;

    size_t _Stdstr;
    int _Mystate, _Except, _Fmtfl;
    long long _Prec, _Wide;
    void *_Arr, *_Calls;
    locale *_Ploc;
};

template <class E, class T> class basic_ostream;

template <class E, class T> class basic_streambuf {
  protected:
    __declspec(dllimport) basic_streambuf();

  public:
    __declspec(dllimport) virtual ~basic_streambuf() noexcept;
    __declspec(dllimport) locale getloc() const;
    __declspec(dllimport) int sbumpc();
    __declspec(dllimport) int sputc(E);
    __declspec(dllimport) long long sputn(const E *, long long);
    long long sgetn(E *p, long long n) { return xsgetn(p, n); }
    __declspec(dllimport) virtual void _Lock();
    __declspec(dllimport) virtual void _Unlock();
    E *eback() const { return *_IGfirst; }
    E *gptr() const { return *_IGnext; }
    E *pbase() const { return *_IPfirst; }
    E *pptr() const { return *_IPnext; }
    void setg(E *f, E *n, E *l) {
        *_IGfirst = f;
        *_IGnext = n;
        *_IGcount = (int)(l - n);
    }
    void setp(E *f, E *l) {
        *_IPfirst = f;
        *_IPnext = f;
        *_IPcount = (int)(l - f);
    }

  protected:
    virtual int overflow(int = -1);
    virtual int pbackfail(int = -1);
    __declspec(dllimport) virtual long long showmanyc();
    virtual int underflow();
    __declspec(dllimport) virtual int uflow();
    __declspec(dllimport) virtual long long xsgetn(E *, long long);
    __declspec(dllimport) virtual long long xsputn(const E *, long long);
    virtual fpos<_Mbstatet> seekoff(long long, int, int = 3);
    virtual fpos<_Mbstatet> seekpos(fpos<_Mbstatet>, int = 3);
    __declspec(dllimport) virtual basic_streambuf *setbuf(E *, long long);
    __declspec(dllimport) virtual int sync();
    __declspec(dllimport) virtual void imbue(const locale &);
    __declspec(dllimport) void _Init();
    __declspec(dllimport) E *_Pninc();

  public:
    E *_Gfirst, *_Pfirst, **_IGfirst, **_IPfirst, *_Gnext, *_Pnext, **_IGnext, **_IPnext;
    int _Gcount, _Pcount, *_IGcount, *_IPcount;
    locale *_Plocale;
};

template <class E, class T> class basic_ios : public ios_base {
  public:
    __declspec(dllimport) void clear(int = 0, bool = false);
    __declspec(dllimport) void setstate(int, bool = false);
    __declspec(dllimport) E widen(char) const;
    basic_streambuf<E, T> *rdbuf() const { return _Mystrbuf; }
    basic_ostream<E, T> *tie() const { return _Tiestr; }
    E fill() const { return _Fillch; }
    E fill(E c) {
        E o = _Fillch;
        _Fillch = c;
        return o;
    }
    __declspec(dllimport) ~basic_ios() noexcept override;

  protected:
    __declspec(dllimport) basic_ios();

  public:
    basic_streambuf<E, T> *_Mystrbuf;
    basic_ostream<E, T> *_Tiestr;
    E _Fillch;
};

template <class E, class T> class basic_ostream : virtual public basic_ios<E, T> {
  public:
    __declspec(dllimport) explicit basic_ostream(basic_streambuf<E, T> *, bool = false);
    __declspec(dllimport) ~basic_ostream() noexcept override;
    __declspec(dllimport) basic_ostream &operator<<(int);
    __declspec(dllimport) basic_ostream &operator<<(unsigned int);
    __declspec(dllimport) basic_ostream &operator<<(unsigned long);
    __declspec(dllimport) basic_ostream &operator<<(unsigned long long);
    __declspec(dllimport) basic_ostream &operator<<(float);
    __declspec(dllimport) basic_ostream &operator<<(const void *);
    __declspec(dllimport) basic_ostream &operator<<(long long);
    __declspec(dllimport) basic_ostream &operator<<(basic_ostream &(__cdecl *)(basic_ostream &));
    __declspec(dllimport) basic_ostream &operator<<(ios_base &(__cdecl *)(ios_base &));
    __declspec(dllimport) basic_ostream &put(E);
    __declspec(dllimport) basic_ostream &write(const E *, long long);
    __declspec(dllimport) basic_ostream &flush();
    __declspec(dllimport) void _Osfx() noexcept;
};

template <class E, class T> class basic_istream : virtual public basic_ios<E, T> {
  public:
    __declspec(dllimport) explicit basic_istream(basic_streambuf<E, T> *, bool = false);
    __declspec(dllimport) ~basic_istream() noexcept override;
    long long gcount() const { return _Chcount; }
    __declspec(dllimport) basic_istream &read(E *, long long);
    __declspec(dllimport) basic_istream &seekg(long long, int);
    __declspec(dllimport) fpos<_Mbstatet> tellg();
    long long _Chcount;
};

template <class E, class T> class basic_iostream : public basic_istream<E, T>, public basic_ostream<E, T> {
  public:
    __declspec(dllimport) explicit basic_iostream(basic_streambuf<E, T> *);
    __declspec(dllimport) ~basic_iostream() noexcept override;
};

typedef basic_streambuf<char, char_traits<char>> streambuf;
typedef basic_ostream<char, char_traits<char>> ostream;
typedef basic_istream<char, char_traits<char>> istream;
typedef basic_iostream<char, char_traits<char>> iostream;

extern __declspec(dllimport) ostream cerr;

template <class A> struct _Smanip {
    void(__cdecl *_Pfun)(ios_base &, A);
    A _Manarg;
};
__declspec(dllimport) _Smanip<long long> __cdecl setw(long long);
__declspec(dllimport) _Smanip<long long> __cdecl setprecision(long long);
template <class E, class T, class A> basic_ostream<E, T> &operator<<(basic_ostream<E, T> &o, const _Smanip<A> &m) {
    (*m._Pfun)(o, m._Manarg);
    return o;
}

__declspec(dllimport) FILE *__cdecl _Fiopen(const char *, int, int);
__declspec(dllimport) FILE *__cdecl _Fiopen(const wchar_t *, int, int);

template <class E, class T> class ostreambuf_iterator {
  public:
    bool _Failed;
    basic_streambuf<E, T> *_Strbuf;
};

template <class E, class It> class time_put : public locale::facet {
  public:
    __declspec(dllimport) It put(It, ios_base &, E, const tm *, const E *, const E *) const;
    __declspec(dllimport) static size_t _Getcat(const locale::facet ** = nullptr, const locale * = nullptr);

  protected:
    virtual It do_put(It, ios_base &, E, const tm *, char, char = 0) const;

  public:
    void *_Tnames;
};
static_assert(sizeof(locale) == 16, "std::locale de MSVC: 16 bytes");
} // namespace std

namespace Concurrency {
// Como en MSVC: la captura (un puntero, o 1 = diferida) y _M_RunInline.
class task_continuation_context {
  public:
    void *_M_context;
    bool _M_RunInline;

  private:
    __declspec(dllimport) task_continuation_context();
    friend struct ::Amiga;
};
} // namespace Concurrency

using namespace std;

struct Amiga {
    static locale::_Locimp *global() { return locale::_Getgloballocale(); }
    static void contexto(void *sitio) { new (sitio) Concurrency::task_continuation_context(); }
};

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

// Un streambuf propio (como los del juego): escribe en su bufer; lee de
// lo que se le de; lo demas, de msvcp.
struct Cuerda : public streambuf {
    char buf[256];
    int syncs = 0;
    Cuerda() { setp(buf, buf + 255); }
    const char *texto() {
        *pptr() = 0;
        return buf;
    }
    void vaciar() { setp(buf, buf + 255); }
    int overflow(int) override { return -1; }
    int pbackfail(int) override { return -1; }
    int underflow() override { return -1; }
    fpos<_Mbstatet> seekoff(long long, int, int) override { return fpos<_Mbstatet>{-1, 0, {}}; }
    fpos<_Mbstatet> seekpos(fpos<_Mbstatet>, int) override { return fpos<_Mbstatet>{-1, 0, {}}; }
    int sync() override {
        syncs++;
        return 0;
    }
    // Lo protegido de msvcp, desde dentro.
    void reiniciar() { _Init(); }
    char *pninc() { return _Pninc(); }
    long long base_showmanyc() { return streambuf::showmanyc(); }
    streambuf *base_setbuf() { return streambuf::setbuf(nullptr, 0); }
    int base_sync() { return streambuf::sync(); }
    void base_imbue(const locale &l) { streambuf::imbue(l); }
};

static void hex64(unsigned long long v) {
    char t[19] = "0x";
    for (int k = 0; k < 16; k++)
        t[2 + k] = "0123456789ABCDEF"[(v >> (60 - 4 * k)) & 15];
    t[18] = 0;
    di(t);
}

// getloc devuelve un locale POR VALOR. Se llama en crudo (this y un bufer
// con marca) y se mira donde queda: su _Locimp, en el +8 del bufer.
extern "C" unsigned long long __cdecl crudo_sb_getloc(const void *, void *) __asm__("?getloc@?$basic_streambuf@DU?$char_traits@D@std@@@std@@QEBA?AVlocale@2@XZ");
extern "C" unsigned long long __cdecl crudo_ios_getloc(const void *, void *) __asm__("?getloc@ios_base@std@@QEBA?AVlocale@2@XZ");

static bool mirar_getloc(const char *que, unsigned long long (*f)(const void *, void *), const void *este) {
    unsigned long long marca[2] = {0x4242424242424242ull, 0x4242424242424242ull};
    unsigned long long rax = f(este, marca);
    unsigned long long g = (unsigned long long)Amiga::global();
    bool ok = rax == (unsigned long long)marca && marca[1] == g;
    if (!ok) {
        di("        ");
        di(que);
        di(": rax ");
        hex64(rax);
        di(", bufer ");
        hex64(marca[1]);
        di(" (en ");
        hex64((unsigned long long)&marca[1]);
        di("), global ");
        hex64(g);
        di("\r\n");
    }
    return ok;
}

// Los flujos (con base virtual) NO se destruyen aqui: clang arma su
// destruccion llamando a ??1 con un `this` que el de msvcp140 no espera, y
// en Windows revienta. MSVC llama a ??_D (tanda19m.exe, 15/15 en Windows):
// ese es el oraculo de los destructores. Cada flujo vive en su sitio.
template <class F, class A> static F &sin_destruir(A a) {
    alignas(16) static unsigned char sitio[4][sizeof(F)];
    static int n;
    return *new (sitio[n++ & 3]) F(a);
}

static ios_base &hex_(ios_base &b) {
    b.setf(0x800, 0xE00);
    return b;
}
static ios_base &dec_(ios_base &b) {
    b.setf(0x200, 0xE00);
    return b;
}

// Un streambuf que lee de un texto y se deja MOVER (seekoff sobre su zona
// de lectura): lo que usan seekg y tellg.
struct Lectora : public streambuf {
    char *ini;
    long long largo;
    explicit Lectora(char *t, long long n) : ini(t), largo(n) { setg(t, t, t + n); }
    int overflow(int) override { return -1; }
    int pbackfail(int) override { return -1; }
    int underflow() override { return -1; }
    fpos<_Mbstatet> seekoff(long long off, int way, int) override {
        long long pos = way == 0 ? off : way == 1 ? (gptr() - eback()) + off : largo + off;
        if (pos < 0 || pos > largo)
            return fpos<_Mbstatet>{-1, 0, {}};
        setg(ini, ini + pos, ini + largo);
        return fpos<_Mbstatet>{pos, 0, {}};
    }
    fpos<_Mbstatet> seekpos(fpos<_Mbstatet> p, int m) override { return seekoff(p._Myoff, 0, m); }
};

static void con_numeros() {
    Cuerda c;
    ostream &o = sin_destruir<ostream>(static_cast<streambuf *>(&c));
    o << 1.5f;
    o.put(' ');
    o << setprecision(3) << 3.14159f;
    o.put(' ');
    o.setf(0x2000, 0x3000);
    o << setprecision(2) << 2.5f;
    o.setf(0, 0x3000);
    o << setprecision(6);
    o.put(' ');
    o << setw(6) << 1.25f;
    mira(igual(c.texto(), "1.5 3.14 2.50   1.25"), "<< float: %g, setprecision(3), fixed con 2 y setw(6)");
    c.vaciar();
    o << (const void *)0x1234;
    o.put(' ');
    o << (const void *)nullptr;
    mira(igual(c.texto(), "0000000000001234 0000000000000000"), "<< const void*: 16 cifras en mayusculas, sin 0x");
    c.vaciar();
    o << -9000000000ll;
    o.put(' ');
    o << (long long)9223372036854775807ll;
    o.put(' ');
    o << hex_ << -1ll << dec_;
    mira(igual(c.texto(), "-9000000000 9223372036854775807 ffffffffffffffff"), "<< long long: negativo, el mayor, y en hex");
}

static void con_lectura() {
    char texto[] = "0123456789";
    Lectora l(texto, 10);
    istream &i = sin_destruir<istream>(static_cast<streambuf *>(&l));
    char b[8] = {};
    i.read(b, 4);
    mira(i.gcount() == 4 && igual(b, "0123") && i.good(), "read(4): cuatro, y gcount 4");
    fpos<_Mbstatet> p = i.tellg();
    mira(p._Myoff == 4, "tellg: en el 4");
    i.seekg(7, 0);
    char c[4] = {};
    i.read(c, 2);
    mira(igual(c, "78") && i.tellg()._Myoff == 9, "seekg(7, beg) y read(2): \"78\", y tellg 9");
    i.seekg(-3, 1);
    mira(i.tellg()._Myoff == 6, "seekg(-3, cur): el 6");
    char d[8] = {};
    i.read(d, 8);
    mira(i.gcount() == 4 && igual(d, "6789") && i.rdstate() == 3, "read de mas: los que hay, y eofbit|failbit");
    fpos<_Mbstatet> q = i.tellg();
    mira(q._Myoff == -1, "tellg tras un fallo: -1");
    i.seekg(0, 0);
    bool sigue = i.rdstate() == 2;
    i.clear();
    i.seekg(0, 0);
    mira(sigue && i.good() && i.tellg()._Myoff == 0, "seekg quita eofbit pero no failbit; tras clear, vuelve al 0");
    i.seekg(99, 0);
    mira(i.rdstate() == 2, "seekg fuera: failbit");
}

static void con_fiopen_ancho() {
    wchar_t ruta[260];
    GetModuleFileNameW(nullptr, ruta, 260);
    FILE *f = _Fiopen(ruta, 0x1 | 0x4 | 0x20, 0x40);
    long fin = f ? ftell(f) : 0;
    if (f)
        fclose(f);
    mira(fin > 1000 && !_Fiopen(L"no_existe_bmo.xyz", 0x1, 0x40), "_Fiopen de un nombre ancho: su propio .exe (al final), y el que no hay");
}

static void con_contexto() {
    alignas(16) unsigned char sitio[32];
    for (unsigned char &x : sitio)
        x = 0x42;
    Amiga::contexto(sitio);
    Concurrency::task_continuation_context *t = (Concurrency::task_continuation_context *)sitio;
    mira((unsigned long long)t->_M_context == 1 && !t->_M_RunInline, "task_continuation_context(): captura diferida (1) y sin correr en linea");
}

static void con_aparatos() {
    DWORD n = 0;
    mira(CM_Get_Device_ID_List_SizeW(&n, nullptr, 0) == 0 && n >= 1, "api-ms-win-devices-config: CM_Get_Device_ID_List_SizeW");
    mira(CM_MapCrToWin32Err(0x25, 5) == 1168 && CM_MapCrToWin32Err(0x99, 5) == 5, "api-ms-win-devices-config: CM_MapCrToWin32Err");
}

extern "C" void inicio() {
    con_numeros();
    con_lectura();
    con_fiopen_ancho();
    con_contexto();
    con_aparatos();
    di("tanda20.exe: las ultimas DURAS son las de Windows\r\n");
    ExitProcess(fallos);
}
