// tanda17.cpp -- el .exe de la TANDA 17 de Cyberpunk (30-09): el RTTI de C++
// de MSVC (DURAS del censo): dynamic_cast (__RTDynamicCast) hacia abajo,
// cruzado entre las bases de una herencia multiple, fallido (NULL) y a una
// referencia (lanza bad_cast); typeid (__RTtypeid), tambien de un puntero
// nulo (lanza bad_typeid); y __unDName (lo que usa type_info::name).
//
// Sin la biblioteca de C++: kernel32, vcruntime140 y el CRT privado. Sale
// con el numero de fallos. En Windows dice lo mismo.
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned long long U64;
#define IMPORTA extern "C" __declspec(dllimport)
IMPORTA HANDLE __stdcall GetStdHandle(DWORD n);
IMPORTA int __stdcall WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void __stdcall ExitProcess(unsigned c);
IMPORTA char *__cdecl __unDName(char *salida, const char *decorado, int largo, void *(*reservar)(U64), void (*soltar)(void *), unsigned short banderas);

// Lo que la biblioteca de C++ pondria y aqui no hay (ver tanda4.cpp).
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

// Lo que diria <typeinfo>: en MSVC, type_info vive en el espacio global.
class type_info {
  public:
    virtual ~type_info();

  private:
    void *datos;
    char nombre[1];
};
namespace std {
using ::type_info;
}

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
    if (!a)
        return false;
    while (*b)
        if (*a++ != *b++)
            return false;
    return *a == 0;
}

struct Base {
    int b = 1;
    virtual ~Base() {}
};
struct Hija : Base {
    int h = 2;
};
struct Otra : Base {
    int o = 3;
};
// Herencia multiple: B no empieza donde el objeto.
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

// Punteros que el compilador no puede seguir: el RTTI de verdad. (Los
// objetos son de `inicio`: sin CRT, nadie construye los globales.)
static Base *volatile p_hija;
static Base *volatile p_otra;
static A *volatile p_a;
static Base *volatile p_nulo = nullptr;

// Como malloc, de un monton propio que solo crece (__unDName pide aqui
// tambien su monton de trabajo: en Windows, sin esta funcion no hace nada).
static char monton[1 << 16];
static U64 gastado;
static void *reservar(U64 n) {
    n = (n + 15) & ~(U64)15;
    if (gastado + n > sizeof monton)
        return nullptr;
    gastado += n;
    return monton + gastado - n;
}
static void soltar(void *) {}

extern "C" void inicio() {
    Hija hija;
    Otra otra;
    C c;
    p_hija = &hija;
    p_otra = &otra;
    p_a = &c;
    // -- dynamic_cast
    mira(dynamic_cast<Hija *>(p_hija) == &hija, "dynamic_cast hacia abajo: Base* a Hija*");
    mira(dynamic_cast<Hija *>(p_otra) == nullptr, "dynamic_cast a otra hija: NULL");
    {
        B *pb = dynamic_cast<B *>(p_a);
        mira(pb == static_cast<B *>(&c) && (void *)pb != (void *)&c && pb->b == 22, "dynamic_cast cruzado: A* a B* en la herencia multiple, ajustado");
        B *volatile vb = pb;
        mira(dynamic_cast<C *>(vb) == &c, "dynamic_cast de la segunda base al objeto entero: B* a C*");
    }
    {
        bool cogida = false;
        try {
            Hija &h = dynamic_cast<Hija &>(*p_otra);
            (void)h;
        } catch (...) {
            cogida = true;
        }
        mira(cogida, "dynamic_cast a una referencia que no es: lanza (bad_cast)");
    }
    // -- typeid
    {
        Base &rh = *p_hija, &ro = *p_otra;
        A &ra = *p_a;
        mira(&typeid(rh) == &typeid(Hija) && &typeid(ro) == &typeid(Otra) && &typeid(ra) == &typeid(C), "typeid del objeto entero, por su referencia a la base");
    }
    {
        bool cogida = false;
        try {
            Base *n = p_nulo;
            const void *t = &typeid(*n);
            (void)t;
        } catch (...) {
            cogida = true;
        }
        mira(cogida, "typeid de un puntero nulo: lanza (bad_typeid)");
    }
    // -- __unDName
    {
        char b[128];
        mira(igual(__unDName(b, "?AVHija@@", 128, reservar, soltar, 0x2800), "class Hija") && igual(__unDName(b, "?AUBase@@", 128, reservar, soltar, 0x2800), "struct Base"), "__unDName de un tipo: class Hija, struct Base");
        mira(igual(__unDName(b, "?AV?$vector@HV?$allocator@H@std@@@std@@", 128, reservar, soltar, 0x2800), "class std::vector<int,class std::allocator<int> >"), "__unDName con plantillas y espacios de nombres");
        mira(igual(__unDName(b, "?que@Hija@@UEBAHXZ", 128, reservar, soltar, 0x1000), "Hija::que") && igual(__unDName(b, "??0Hija@@QEAA@XZ", 128, reservar, soltar, 0x1000), "Hija::Hija"), "__unDName, solo el nombre: un metodo y un constructor");
        char *r = __unDName(nullptr, "?AVHija@@", 0, reservar, soltar, 0x2800);
        mira(r >= monton && r < monton + sizeof monton && igual(r, "class Hija") && !__unDName(b, "?AVHija@@", 128, nullptr, nullptr, 0x2800), "__unDName sin bufer: lo pide a la funcion de reservar; sin ella, NULL");
    }
    di("tanda17.exe: el RTTI de C++ es el de Windows\r\n");
    ExitProcess(fallos);
}
