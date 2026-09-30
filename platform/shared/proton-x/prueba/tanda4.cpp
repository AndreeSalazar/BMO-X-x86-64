// tanda4.cpp -- el .exe de la TANDA 4 de Cyberpunk (29-09): las excepciones
// de C++ de MSVC (_CxxThrowException y __CxxFrameHandler3): throw y catch
// por valor, por referencia y por puntero, a una base (tambien con herencia
// multiple, donde el puntero se ajusta), catch(...), throw; y un throw
// dentro de un catch, los destructores de cada marco en su orden, y la
// excepcion destruida una vez al acabar de cogerla.
//
// Sin la biblioteca de C++: solo kernel32 (para escribir) y vcruntime140
// (las dos de arriba). Sale con el numero de fallos. En Windows dice lo mismo.
typedef void *HANDLE;
typedef unsigned long DWORD;
#define IMPORTA extern "C" __declspec(dllimport)
IMPORTA HANDLE __stdcall GetStdHandle(DWORD n);
IMPORTA int __stdcall WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void __stdcall ExitProcess(unsigned c);

// Lo que la biblioteca de C++ pondria y aqui no hay: el vftable de
// type_info (solo se mira el NOMBRE del tipo) y operator delete (el
// destructor virtual lo nombra; aqui nada se reserva con new). Con `cl` de
// MSVC tampoco los da vcruntime.lib (estan en msvcrt.lib, que tira de todo
// el CRT: medido en el Windows del propietario el 30-09), asi que se ponen
// aqui con los dos compiladores. El nombre decorado del vftable: con clang,
// una etiqueta de asm; con cl, /alternatename del enlazador.
extern "C" const void *const vftable_type_info
#if defined(__clang__)
    __asm__("??_7type_info@@6B@")
#endif
    ;
extern "C" const void *const vftable_type_info = nullptr;
#if !defined(__clang__)
#pragma comment(linker, "/alternatename:??_7type_info@@6B@=vftable_type_info")
#endif
void operator delete(void *) noexcept {}
void operator delete(void *, unsigned long long) noexcept {}

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

// El orden en que pasan las cosas: una letra por suceso.
static char diario[64];
static int n_diario;
static void apunta(char c) {
    if (n_diario < 63)
        diario[n_diario++] = c;
    diario[n_diario] = 0;
}
static bool dice(const char *s) {
    int k = 0;
    for (; s[k] && diario[k] == s[k]; k++)
        ;
    return s[k] == 0 && diario[k] == 0;
}
static void borra() {
    n_diario = 0;
    diario[0] = 0;
}

struct Guarda {
    char c;
    explicit Guarda(char c) : c(c) {}
    ~Guarda() { apunta(c); }
};

struct Base {
    int b = 7;
    virtual ~Base() {}
    virtual int que() const { return 1; }
};
struct Hija : Base {
    int h = 9;
    int que() const override { return 2; }
};

// Herencia multiple: la segunda base NO empieza donde el objeto (las dos
// con vtable: MSVC pone primero la que la tiene).
struct A {
    long long a = 11;
    virtual ~A() {}
};
struct B {
    long long b = 22;
    virtual ~B() {}
};
struct C : A, B {
    long long c = 33;
};

// Cuenta copias y destrucciones del objeto lanzado.
static int copias, muertes;
struct Contada {
    int v;
    explicit Contada(int v) : v(v) {}
    Contada(const Contada &o) : v(o.v) { copias++; }
    ~Contada() { muertes++; }
};

static volatile int cero = 0;

__declspec(noinline) static void lanza_int(int v) {
    Guarda g('1');
    if (!cero)
        throw v;
}

__declspec(noinline) static void lanza_hija() {
    Guarda g('2');
    Hija h;
    h.b = 70;
    if (!cero)
        throw h;
}

__declspec(noinline) static void medio() {
    Guarda g('3');
    lanza_hija();
}

__declspec(noinline) static void lanza_contada() {
    if (!cero)
        throw Contada(5);
}

static C *el_c;
__declspec(noinline) static void lanza_puntero_c() {
    if (!cero)
        throw el_c;
}

__declspec(noinline) static void relanza() {
    try {
        lanza_int(40);
    } catch (int &) {
        apunta('r');
        throw;
    }
}

__declspec(noinline) static void lanza_en_catch() {
    try {
        lanza_contada();
    } catch (Contada &c) {
        apunta('c');
        throw c.v + 100;
    }
}

__declspec(noinline) static int catch_con_try() {
    try {
        lanza_int(1);
    } catch (int) {
        try {
            lanza_int(2);
        } catch (int v) {
            return v * 10;
        }
    }
    return -1;
}

extern "C" void inicio() {
    // 1. Un int, por valor, y el destructor del marco que lanza.
    borra();
    int r = -1;
    try {
        lanza_int(42);
    } catch (int v) {
        r = v;
    }
    mira(r == 42 && dice("1"), "throw 42, catch (int): el valor, y el destructor del que lanza");
    // 2. A traves de dos marcos, a una base por referencia.
    borra();
    r = -1;
    try {
        medio();
    } catch (const Base &b) {
        r = b.b * 10 + b.que();
    }
    mira(r == 702 && dice("23"), "throw Hija, catch (const Base &): la hija entera, y los destructores de abajo arriba");
    // 3. Un catch que no casa deja pasar: el de fuera la coge.
    r = -1;
    try {
        try {
            lanza_int(7);
        } catch (Base &) {
            r = -2;
        }
    } catch (int v) {
        r = v;
    }
    mira(r == 7, "un catch de otro tipo no la coge; el de fuera si");
    // 4. catch (...).
    r = -1;
    try {
        medio();
    } catch (...) {
        r = 1;
    }
    mira(r == 1, "catch (...)");
    // 5. Por valor con constructor de copia: una copia al catch, y la
    // lanzada destruida al acabar.
    copias = muertes = 0;
    r = -1;
    try {
        lanza_contada();
    } catch (Contada c) {
        r = c.v;
    }
    mira(r == 5 && copias == 1 && muertes == 2, "catch (Contada) por valor: una copia, y las dos destruidas");
    copias = muertes = 0;
    try {
        lanza_contada();
    } catch (Contada &c) {
        r = c.v + (muertes == 0);
    }
    mira(r == 6 && copias == 0 && muertes == 1, "catch (Contada &): sin copia, y destruida al salir del catch");
    // 6. Un puntero a C cogido como B*: el puntero se ajusta a la base.
    C local_c;
    el_c = &local_c;
    B *pb = nullptr;
    try {
        lanza_puntero_c();
    } catch (B *p) {
        pb = p;
    }
    mira(pb == static_cast<B *>(el_c) && (void *)pb != (void *)el_c && pb->b == 22, "throw C*, catch (B *): el puntero ajustado a la segunda base");
    // 7. throw; relanza la MISMA, que coge el de fuera.
    borra();
    r = -1;
    try {
        relanza();
    } catch (int v) {
        r = v;
    }
    mira(r == 40 && dice("1r"), "throw; dentro de un catch: la coge el de fuera");
    // 8. Una excepcion nueva dentro de un catch: la vieja se destruye.
    borra();
    copias = muertes = 0;
    r = -1;
    try {
        lanza_en_catch();
    } catch (int v) {
        r = v;
    }
    mira(r == 105 && dice("c") && muertes == 1, "throw dentro de un catch: la nueva sube y la vieja se destruye");
    // 9. Un try dentro de un catch.
    mira(catch_con_try() == 20, "un try y su catch dentro de otro catch");
    // 10. Despues de todo, se sigue lanzando y cogiendo igual.
    r = 0;
    for (int k = 0; k < 50; k++) {
        try {
            lanza_int(k);
        } catch (int v) {
            r += v;
        }
    }
    mira(r == 1225, "cincuenta seguidas");
    di("tanda4.exe: las excepciones de C++ son las de Windows\r\n");
    ExitProcess(fallos);
}
