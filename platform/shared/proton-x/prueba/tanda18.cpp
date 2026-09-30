// tanda18.cpp -- el .exe de la TANDA 18 de Cyberpunk (30-09): el LOCALE de
// msvcp140.dll, como lo usa el codigo inline de las cabeceras de MSVC: los
// id (DATOS importados), el locale global y sus _Locimp, facet, _Locinfo,
// _Yarn, ctype<char> y los dos codecvt (el de char y el de wchar_t).
//
// Sin la biblioteca de C++: las clases se DECLARAN con sus miembros donde
// MSVC los pone y sus virtuales en el orden de sus cabeceras (clang las pone
// en la vtabla como MSVC). Lo que en MSVC es inline (use_facet, _Getfacet,
// is, widen, encoding...) se escribe aqui igual: lee los campos y llama por
// la vtabla. Sale con el numero de fallos. En Windows dice lo mismo.
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned long long size_t;
#define IMPORTA extern "C" __declspec(dllimport)
IMPORTA HANDLE __stdcall GetStdHandle(DWORD n);
IMPORTA int __stdcall WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void __stdcall ExitProcess(unsigned c);
IMPORTA void __cdecl free(void *p);

void operator delete(void *) noexcept {}
void operator delete(void *, size_t) noexcept {}
inline void *operator new(size_t, void *p) noexcept { return p; }

struct _Mbstatet {
    unsigned long _Wchar;
    unsigned short _Byte, _State;
};
struct _Collvec {
    unsigned int _Page;
    wchar_t *_LocaleName;
};
struct _Cvtvec {
    unsigned int _Page;
    unsigned int _Mbcurmax;
    int _Isclocale;
    unsigned char _Isleadbyte[32];
};

struct P; // las pruebas: amigas de todo

namespace std {
template <class E> class _Yarn {
  public:
    __declspec(dllimport) _Yarn &operator=(const E *);
    __declspec(dllimport) const E *c_str() const;
    const E *_C_str() const { return _Myptr ? _Myptr : &_Nul; }
    E *_Myptr;
    E _Nul;
};

class _Locinfo {
  public:
    __declspec(dllimport) _Locinfo(const char *);
    __declspec(dllimport) ~_Locinfo();
    __declspec(dllimport) _Collvec _Getcoll() const;
    __declspec(dllimport) _Cvtvec _Getcvt() const;
    __declspec(dllimport) const unsigned short *_W_Getdays() const;
    __declspec(dllimport) const unsigned short *_W_Getmonths() const;
    const char *_Getname() const { return _Newlocname._C_str(); }
    int _Lock;
    _Yarn<char> _Days, _Months;
    _Yarn<wchar_t> _W_Days, _W_Months;
    _Yarn<char> _Oldlocname, _Newlocname;
};

class _Facet_base {
  public:
    virtual ~_Facet_base() noexcept {}
    virtual void _Incref() noexcept = 0;
    virtual _Facet_base *_Decref() noexcept = 0;
};

class locale {
  public:
    class id {
      public:
        __declspec(dllimport) operator size_t();
        size_t _Id;
    };
    class facet : public _Facet_base {
      public:
        __declspec(dllimport) void _Incref() noexcept override;
        __declspec(dllimport) _Facet_base *_Decref() noexcept override;
        unsigned long _Myrefs;

      protected:
        __declspec(dllimport) explicit facet(size_t);
        __declspec(dllimport) ~facet() noexcept override;
        friend P;
    };
    class _Locimp : public facet {
      private:
        __declspec(dllimport) static _Locimp *_New_Locimp(const _Locimp &);
        __declspec(dllimport) static void _Locimp_Addfac(_Locimp *, facet *, size_t);
        __declspec(dllimport) void _Addfac(facet *, size_t);

      public:
        facet **_Facetvec;
        size_t _Facetcount;
        int _Catmask;
        bool _Xparent;
        _Yarn<char> _Name;
        friend P;
    };

    locale() noexcept : _Ptr(_Init(true)) {}
    ~locale() noexcept {
        if (_Ptr)
            delete _Ptr->_Decref();
    }
    const facet *_Getfacet(size_t i) const {
        const facet *f = i < _Ptr->_Facetcount ? _Ptr->_Facetvec[i] : nullptr;
        if (f || !_Ptr->_Xparent)
            return f;
        _Locimp *g = _Getgloballocale();
        return i < g->_Facetcount ? g->_Facetvec[i] : nullptr;
    }
    _Locimp *_Ptr;

  private:
    __declspec(dllimport) static _Locimp *_Init(bool);
    __declspec(dllimport) static _Locimp *_Getgloballocale();
    friend P;
};

struct ctype_base : public locale::facet {};

template <class E> class ctype;
template <> class ctype<char> : public ctype_base {
  public:
    bool is(short m, char c) const { return (_Table[(unsigned char)c] & m) != 0; }
    __declspec(dllimport) char tolower(char) const;
    __declspec(dllimport) const char *tolower(char *, const char *) const;
    char toupper(char c) const { return do_toupper(c); }
    char widen(char c) const { return do_widen(c); }
    char narrow(char c, char d) const { return do_narrow(c, d); }
    __declspec(dllimport) static size_t _Getcat(const locale::facet ** = nullptr, const locale * = nullptr);
    __declspec(dllimport) static locale::id id;

  protected:
    virtual ~ctype() noexcept;
    virtual char do_tolower(char) const;
    virtual const char *do_tolower(char *, const char *) const;
    virtual char do_toupper(char) const;
    virtual const char *do_toupper(char *, const char *) const;
    virtual char do_widen(char) const;
    virtual const char *do_widen(const char *, const char *, char *) const;
    virtual char do_narrow(char, char) const;
    virtual const char *do_narrow(const char *, const char *, char, char *) const;

  public:
    unsigned int _Page;
    const short *_Table;
    int _Delfl;
    wchar_t *_LocaleName;
};

class codecvt_base : public locale::facet {
  public:
    __declspec(dllimport) bool always_noconv() const noexcept;
    int max_length() const noexcept { return do_max_length(); }
    int encoding() const noexcept { return do_encoding(); }

  protected:
    virtual bool do_always_noconv() const noexcept;
    virtual int do_max_length() const noexcept;
    virtual int do_encoding() const noexcept;
};

template <class E, class B, class S> class codecvt;
template <> class codecvt<char, char, _Mbstatet> : public codecvt_base {
  public:
    __declspec(dllimport) int in(_Mbstatet &, const char *, const char *, const char *&, char *, char *, char *&) const;
    __declspec(dllimport) int out(_Mbstatet &, const char *, const char *, const char *&, char *, char *, char *&) const;
    __declspec(dllimport) int unshift(_Mbstatet &, char *, char *, char *&) const;
    int length(_Mbstatet &s, const char *a, const char *b, size_t n) const { return do_length(s, a, b, n); }
    __declspec(dllimport) static size_t _Getcat(const locale::facet ** = nullptr, const locale * = nullptr);
    __declspec(dllimport) static locale::id id;

  protected:
    virtual ~codecvt() noexcept;
    virtual int do_in(_Mbstatet &, const char *, const char *, const char *&, char *, char *, char *&) const;
    virtual int do_out(_Mbstatet &, const char *, const char *, const char *&, char *, char *, char *&) const;
    virtual int do_unshift(_Mbstatet &, char *, char *, char *&) const;
    virtual int do_length(_Mbstatet &, const char *, const char *, size_t) const;
};

template <> class codecvt<wchar_t, char, _Mbstatet> : public codecvt_base {
  public:
    __declspec(dllimport) int in(_Mbstatet &, const char *, const char *, const char *&, wchar_t *, wchar_t *, wchar_t *&) const;
    __declspec(dllimport) int out(_Mbstatet &, const wchar_t *, const wchar_t *, const wchar_t *&, char *, char *, char *&) const;
    __declspec(dllimport) explicit codecvt(size_t);
    __declspec(dllimport) static locale::id id;

  protected:
    __declspec(dllimport) ~codecvt() noexcept override;
    virtual int do_in(_Mbstatet &, const char *, const char *, const char *&, wchar_t *, wchar_t *, wchar_t *&) const;
    virtual int do_out(_Mbstatet &, const wchar_t *, const wchar_t *, const wchar_t *&, char *, char *, char *&) const;
    virtual int do_unshift(_Mbstatet &, char *, char *, char *&) const;
    virtual int do_length(_Mbstatet &, const char *, const char *, size_t) const;

  public:
    _Cvtvec _Cvt;
    friend P;
};

template <class E> class collate {
  public:
    __declspec(dllimport) static locale::id id;
};
} // namespace std

using namespace std;

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

template <class C> static bool igual(const C *a, const char *b) {
    while (*a && (unsigned)*a == (unsigned char)*b)
        a++, b++;
    return (unsigned)*a == (unsigned char)*b;
}

template <class C> static bool empieza(const C *a, const char *b) {
    while (*b && (unsigned)*a == (unsigned char)*b)
        a++, b++;
    return *b == 0;
}

// use_facet como el de las cabeceras: del locale, o _Getcat.
template <class F> static const F &usar(const locale &l) {
    size_t i = F::id;
    const locale::facet *f = l._Getfacet(i);
    if (!f) {
        F::_Getcat(&f, &l);
        const_cast<locale::facet *>(f)->_Incref();
    }
    return *static_cast<const F *>(f);
}

// Una faceta propia (como las del juego): su vtabla es de aqui.
struct Mia : public locale::facet {
    Mia() : facet(1) {}
    static locale::id id;
};
locale::id Mia::id;

struct P {
    static void locale_y_ids() {
        size_t a = ctype<char>::id, b = ctype<char>::id, c = codecvt<char, char, _Mbstatet>::id;
        size_t d = codecvt<wchar_t, char, _Mbstatet>::id, e = collate<char>::id;
        mira(a != 0 && a == b && c != 0 && c != a && d != c && d != a && e != 0 && e != d, "locale::id: cada faceta su numero, y el mismo cada vez (DATOS importados)");
        locale l;
        mira(l._Ptr == locale::_Getgloballocale() && igual(l._Ptr->_Name.c_str(), "C"), "locale(): el global, \"C\" (_Init y _Getgloballocale)");
    }

    static void con_ctype() {
        locale l;
        const ctype<char> &ct = usar<ctype<char>>(l);
        mira(ct.tolower('Q') == 'q' && ct.toupper('a') == 'A' && ct.widen('x') == 'x' && ct.narrow('y', '?') == 'y', "ctype<char>: tolower (de msvcp), toupper, widen y narrow (por su vtabla)");
        mira(ct.is(1, 'A') && !ct.is(1, 'a') && ct.is(4, '7') && ct.is(8, ' ') && !ct.is(4, 'z'), "ctype<char>::is: su tabla (mayuscula, cifra, espacio)");
        char t[] = "HoLA, Mundo";
        const char *fin = ct.tolower(t, t + 11);
        mira(fin == t + 11 && igual(t, "hola, mundo"), "ctype<char>::tolower de un tramo");
        const locale::facet *f = nullptr;
        size_t cat = ctype<char>::_Getcat(&f, &l);
        const ctype<char> *n = static_cast<const ctype<char> *>(f);
        mira(cat == 2 && n && n->_Myrefs == 0 && n->tolower('Z') == 'z' && n->is(1, 'Z'), "ctype<char>::_Getcat: una nueva, de LC_CTYPE, sin referencias");
        const_cast<locale::facet *>(f)->_Incref();
        delete const_cast<locale::facet *>(f)->_Decref();
    }

    static void con_codecvt() {
        locale l;
        const codecvt<char, char, _Mbstatet> &cv = usar<codecvt<char, char, _Mbstatet>>(l);
        _Mbstatet st = {};
        const char *s = "abc", *m1 = nullptr;
        char o[8], *m2 = nullptr;
        int r1 = cv.in(st, s, s + 3, m1, o, o + 8, m2);
        bool en = r1 == 3 && m1 == s && m2 == o;
        int r2 = cv.out(st, s, s + 3, m1, o, o + 8, m2);
        int r3 = cv.unshift(st, o, o + 8, m2);
        mira(cv.always_noconv() && en && r2 == 3 && r3 == 3 && m2 == o, "codecvt<char,char>: noconv (in, out, unshift, always_noconv)");
        mira(cv.max_length() == 1 && cv.encoding() == 1 && cv.length(st, s, s + 3, 2) == 2, "codecvt<char,char>: max_length, encoding y length (por su vtabla)");

        alignas(8) unsigned char sitio[sizeof(codecvt<wchar_t, char, _Mbstatet>)];
        codecvt<wchar_t, char, _Mbstatet> *w = new (sitio) codecvt<wchar_t, char, _Mbstatet>(1);
        const wchar_t *h = L"hola", *wm = nullptr;
        char b[8] = {}, *bm = nullptr;
        int o1 = w->out(st, h, h + 4, wm, b, b + 8, bm);
        mira(o1 == 0 && wm == h + 4 && bm == b + 4 && b[0] == 'h' && b[3] == 'a', "codecvt<wchar_t,char>::out: \"hola\" en bytes");
        wchar_t x[8] = {}, *xm = nullptr;
        const char *ad = "adios", *am = nullptr;
        int i1 = w->in(st, ad, ad + 5, am, x, x + 8, xm);
        mira(i1 == 0 && am == ad + 5 && xm == x + 5 && x[0] == L'a' && x[4] == L's', "codecvt<wchar_t,char>::in: \"adios\" en anchos");
        const wchar_t *cara = L"\x263A";
        int e1 = w->out(st, cara, cara + 1, wm, b, b + 8, bm);
        mira(e1 == 2 && !w->always_noconv() && w->encoding() == 1 && w->_Cvt._Mbcurmax == 1 && w->_Myrefs == 1, "codecvt<wchar_t,char>: sin byte en \"C\" es error; encoding 1, un byte");
        w->codecvt<wchar_t, char, _Mbstatet>::~codecvt();
    }

    static void con_locinfo_y_yarn() {
        {
            _Locinfo li("C");
            _Cvtvec c = li._Getcvt();
            _Collvec k = li._Getcoll();
            (void)k;
            mira(c._Mbcurmax == 1 && igual(li._Getname(), "C"), "_Locinfo(\"C\"): su nombre y _Getcvt (un byte)");
            mira(empieza(li._W_Getdays(), ":Sun:Sunday:Mon:Monday") && empieza(li._W_Getmonths(), ":Jan:January:Feb:February"), "_Locinfo: _W_Getdays y _W_Getmonths");
        }
        _Yarn<char> y;
        y._Myptr = nullptr;
        y._Nul = 0;
        bool vacia = igual(y.c_str(), "");
        y = "hola";
        bool una = igual(y.c_str(), "hola");
        y = "mundo";
        mira(vacia && una && igual(y.c_str(), "mundo") && y._Myptr, "_Yarn<char>: operator= y c_str (y se libera con free)");
        free(y._Myptr);
    }

    static void con_locimp() {
        locale::_Locimp *g = locale::_Getgloballocale();
        locale::_Locimp *n = locale::_Locimp::_New_Locimp(*g);
        Mia m;
        size_t i = Mia::id;
        n->_Addfac(&m, i);
        bool puesta = n->_Facetcount > i && n->_Facetvec[i] == &m && m._Myrefs == 2 && n->_Myrefs == 1;
        locale l2;
        locale::_Locimp *viejo = l2._Ptr;
        l2._Ptr = n;
        bool vista = l2._Getfacet(i) == &m && g->_Myrefs >= 1;
        Mia m2;
        locale::_Locimp::_Locimp_Addfac(n, &m2, i);
        bool cambiada = n->_Facetvec[i] == &m2 && m._Myrefs == 1 && m2._Myrefs == 2;
        l2._Ptr = viejo;
        delete n->_Decref();
        mira(puesta && vista, "_Locimp: _New_Locimp copia el global y _Addfac pone una faceta propia");
        mira(cambiada && m2._Myrefs == 1, "_Locimp_Addfac cambia la faceta; al borrar el _Locimp se sueltan");
    }
};

extern "C" void inicio() {
    P::locale_y_ids();
    P::con_ctype();
    P::con_codecvt();
    P::con_locinfo_y_yarn();
    P::con_locimp();
    di("tanda18.exe: el locale de msvcp140 es el de Windows\r\n");
    ExitProcess(fallos);
}
