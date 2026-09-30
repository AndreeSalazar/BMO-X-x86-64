// tanda19.cpp -- el .exe de la TANDA 19 de Cyberpunk (30-09): los FLUJOS de
// msvcp140.dll: basic_streambuf (un streambuf propio encima, como los del
// juego), basic_ostream y sus numeros, basic_istream y basic_iostream (con
// su BASE VIRTUAL), cerr (un DATO importado), setw, _Fiopen y time_put.
//
// Sin la biblioteca de C++: las clases se DECLARAN con sus miembros donde
// MSVC los pone; lo inline de las cabeceras (setp, rdbuf, flags, setf...) se
// escribe aqui igual, y llega al ios por el vbptr como en MSVC. Sale con el
// numero de fallos. En Windows dice lo mismo.
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
IMPORTA int __cdecl fclose(FILE *f);
IMPORTA long __cdecl ftell(FILE *f);

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
template <class E, class T, class A> basic_ostream<E, T> &operator<<(basic_ostream<E, T> &o, const _Smanip<A> &m) {
    (*m._Pfun)(o, m._Manarg);
    return o;
}

__declspec(dllimport) FILE *__cdecl _Fiopen(const char *, int, int);

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

using namespace std;

struct Amiga {
    static locale::_Locimp *global() { return locale::_Getgloballocale(); }
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

static void paso(const char *que) {
    di("  (paso) ");
    di(que);
    di("\r\n");
}

static ios_base &hex_(ios_base &b) {
    b.setf(0x800, 0xE00);
    return b;
}
static ios_base &dec_(ios_base &b) {
    b.setf(0x200, 0xE00);
    return b;
}
static ostream &fin_(ostream &o) {
    o.put('\n');
    return o.flush();
}

static void con_streambuf() {
    Cuerda c;
    bool hecho = c.pbase() == c.buf && c.eback() == nullptr && c._Plocale != nullptr;
    c.reiniciar();
    mira(hecho && c.pbase() == nullptr && c.pptr() == nullptr, "basic_streambuf(): su locale y los punteros (_Init los deja a cero)");
    c.vaciar();
    c.sputc('a');
    long long n = c.sputn("bcd", 3);
    *c.pninc() = 'e';
    mira(n == 3 && igual(c.texto(), "abcde"), "sputc, sputn (por xsputn) y _Pninc escriben en el bufer");
    char entrada[] = "hola mundo", sale[8] = {};
    c.setg(entrada, entrada, entrada + 10);
    int h = c.sbumpc();
    long long k = c.sgetn(sale, 4);
    c.setg(entrada, entrada + 10, entrada + 10);
    mira(h == 'h' && k == 4 && igual(sale, "ola ") && c.sbumpc() == -1, "sbumpc, xsgetn y, sin nada, uflow (underflow dice EOF)");
    locale l = {};
    l._Ptr = nullptr;
    c._Lock();
    c._Unlock();
    c.base_imbue(l);
    mira(c.base_showmanyc() == 0 && c.base_setbuf() == &c && c.base_sync() == 0, "showmanyc, setbuf, sync, imbue y _Lock de la base");
    mira(mirar_getloc("basic_streambuf::getloc", crudo_sb_getloc, &c), "basic_streambuf::getloc: el locale global, por valor");
    paso("fin del cuerpo de con_streambuf");
}

static void con_ostream() {
    Cuerda c;
    ostream o(&c);
    mira(o.rdbuf() == &c && o.good() && o.flags() == 0x201 && o.fill() == ' ' && o.width() == 0 && o.tie() == nullptr && o._Prec == 6, "basic_ostream(sb): su ios por el vbptr (skipws|dec, relleno ' ', precision 6)");
    o << 42;
    o.put(' ');
    o << -7;
    o.put(' ');
    o << 4000000000u;
    o.put(' ');
    o << (unsigned long)123;
    o.put(' ');
    o << 18446744073709551615ull;
    o.write("abc", 3);
    mira(igual(c.texto(), "42 -7 4000000000 123 18446744073709551615abc"), "<< int, unsigned, unsigned long, unsigned long long; put y write");
    c.vaciar();
    o << hex_ << 255;
    o.put(' ');
    o << -1;
    o.put(' ');
    o.setf(0x8 | 0x4);
    o << 255;
    o.unsetf(0x8 | 0x4);
    o << dec_;
    mira(igual(c.texto(), "ff ffffffff 0XFF"), "hex (un manipulador de ios_base): -1 como unsigned; showbase y uppercase");
    c.vaciar();
    o << setw(5) << 7;
    o.setf(0x40, 0x1C0);
    o << setw(4) << 3;
    o.setf(0x100, 0x1C0);
    o.setf(0x20);
    o << setw(5) << 9;
    o.unsetf(0x20);
    o.setf(0, 0x1C0);
    o.fill('*');
    o << setw(3) << 1;
    o.fill(' ');
    o << 5;
    mira(igual(c.texto(), "    7" "3   " "+   9" "**1" "5"), "setw: a la derecha, a la izquierda, por dentro (con showpos), con relleno; y se gasta");
    c.vaciar();
    int antes = c.syncs;
    o << fin_;
    bool vacio = c.syncs == antes + 1 && igual(c.texto(), "\n");
    o.setf(0x2);
    o.put('!');
    o._Osfx();
    o.unsetf(0x2);
    mira(vacio && c.syncs == antes + 3, "un manipulador de ostream (put y flush); unitbuf y _Osfx llaman a sync");
    c.vaciar();
    o.setstate(2);
    bool fallo = o.rdstate() == 2;
    o << 5;
    o.clear();
    mira(fallo && o.rdstate() == 0 && igual(c.texto(), ""), "setstate(failbit): no escribe; clear() lo quita");
    {
        ostream nada(nullptr);
        nada.clear();
        mira(nada.rdstate() == 4, "sin streambuf, badbit (y clear no lo quita)");
    }
    paso("un ostream sin streambuf, destruido");
    mira(o.widen('z') == 'z', "basic_ios::widen");
    mira(mirar_getloc("ios_base::getloc", crudo_ios_getloc, static_cast<ios_base *>(&o)), "ios_base::getloc: el locale global, por valor");
    paso("fin del cuerpo de con_ostream");
}

static void con_istream() {
    Cuerda c;
    {
        istream i(&c);
        mira(i.gcount() == 0 && i.rdbuf() == &c && i.good() && i.flags() == 0x201, "basic_istream(sb): _Chcount y su ios por el vbptr");
    }
    paso("un istream, destruido");
    iostream io(&c);
    istream &ie = io;
    ostream &os = io;
    os << 12;
    mira(ie.rdbuf() == &c && os.rdbuf() == &c && igual(c.texto(), "12") && os.good(), "basic_iostream(sb): un ios para las dos partes; escribe por la de ostream");
}

static void con_cerr() {
    bool listo = cerr.rdbuf() != nullptr && (cerr.flags() & 0x2) && cerr.good();
    cerr.write("  (cerr) hola\r\n", 15);
    mira(listo && cerr.good(), "cerr (un DATO): con streambuf, unitbuf; escribe");
}

static void con_fiopen() {
    char ruta[260];
    GetModuleFileNameA(nullptr, ruta, 260);
    FILE *f = _Fiopen(ruta, 0x1 | 0x20, 0x40);
    bool abre = f != nullptr;
    if (f)
        fclose(f);
    f = _Fiopen(ruta, 0x1 | 0x4 | 0x20, 0x40);
    long fin = f ? ftell(f) : 0;
    if (f)
        fclose(f);
    mira(abre && fin > 1000 && !_Fiopen("no_existe_bmo.xyz", 0x1, 0x40) && !_Fiopen(ruta, 0x10, 0x40), "_Fiopen: in|binary, ate (al final), el que no hay y un modo que no vale");
}

typedef time_put<char, ostreambuf_iterator<char, char_traits<char>>> TimePut;

static void con_time_put() {
    Cuerda c;
    ostream o(&c);
    const locale::facet *f = nullptr;
    size_t cat = TimePut::_Getcat(&f, nullptr);
    const TimePut *tp = static_cast<const TimePut *>(f);
    tm t = {9, 5, 13, 30, 8, 126, 3, 272, 0};
    const char *p = "%Y-%m-%d %H:%M:%S %a %b %j %p %x %% %#m %q %A %B %I";
    const char *q = p;
    while (*q)
        q++;
    ostreambuf_iterator<char, char_traits<char>> it = {false, &c};
    ostreambuf_iterator<char, char_traits<char>> r = tp->put(it, o, ' ', &t, p, q);
    mira(cat == 5 && !r._Failed && r._Strbuf == &c && igual(c.texto(), "2026-09-30 13:05:09 Wed Sep 273 PM 09/30/26 % 9 %q Wednesday September 01"), "time_put: _Getcat (LC_TIME) y put con el strftime de \"C\"");
    const_cast<locale::facet *>(f)->_Incref();
    delete const_cast<locale::facet *>(f)->_Decref();
}

extern "C" void inicio() {
    con_streambuf();
    paso("con_streambuf acabo");
    con_ostream();
    paso("con_ostream acabo");
    con_istream();
    paso("con_istream acabo");
    con_cerr();
    con_fiopen();
    con_time_put();
    di("tanda19.exe: los flujos de msvcp140 son los de Windows\r\n");
    ExitProcess(fallos);
}
