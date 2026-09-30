// tanda20m.cpp -- la TANDA 20 compilada por MSVC con SUS cabeceras (30-09):
// las ultimas DURAS de msvcp140 como las usa el juego (<< float con
// setprecision y fixed, << const void*, << long long, istream::read, seekg y
// tellg, un ifstream con un nombre ANCHO -- el _Fiopen de wchar_t -- y
// task_continuation_context::use_default, que llama a su constructor
// privado). Es la gemela de tanda20.cpp (clang), que no sirve de oraculo en
// la ABI de C++.
//
// En la consola de "x64 Native Tools Command Prompt for VS":
//   cl /nologo /EHsc /MD /O1 /std:c++17 tanda20m.cpp /Fe:tanda20m.exe
// Dice `bien` o `MAL` por comprobacion (y lo que salio, si no es lo
// esperado) y sale con el numero de fallos.
#include <windows.h>

#include <fstream>
#include <iomanip>
#include <ppltasks.h>
#include <sstream>
#include <string>

static unsigned fallos;

static void di(const char *t) {
    DWORD n = 0, k = 0;
    while (t[n])
        n++;
    WriteFile(GetStdHandle(STD_OUTPUT_HANDLE), t, n, &k, nullptr);
}

static void mira(bool bien, const char *que) {
    if (!bien)
        fallos++;
    di(bien ? "  bien  " : "  MAL   ");
    di(que);
    di("\r\n");
}

static void igual(const std::string &salio, const char *esperado, const char *que) {
    bool ok = salio == esperado;
    mira(ok, que);
    if (!ok) {
        di("        salio \"");
        di(salio.c_str());
        di("\"\r\n");
    }
}

static void con_numeros() {
    std::ostringstream o;
    o << 1.5f << ' ' << std::setprecision(3) << 3.14159f << ' ' << std::fixed << std::setprecision(2) << 2.5f << std::defaultfloat << std::setprecision(6) << ' ' << std::setw(6) << 1.25f;
    igual(o.str(), "1.5 3.14 2.50   1.25", "<< float: %g, setprecision(3), fixed con 2 y setw(6)");
    o.str("");
    o << (const void *)0x1234 << ' ' << (const void *)nullptr;
    igual(o.str(), "0000000000001234 0000000000000000", "<< const void*: 16 cifras en mayusculas, sin 0x");
    o.str("");
    o << -9000000000ll << ' ' << 9223372036854775807ll << ' ' << std::hex << -1ll << std::dec;
    igual(o.str(), "-9000000000 9223372036854775807 ffffffffffffffff", "<< long long: negativo, el mayor, y en hex");
}

static void con_lectura() {
    std::istringstream i("0123456789");
    char b[8] = {};
    i.read(b, 4);
    mira(i.gcount() == 4 && std::string(b) == "0123" && i.good(), "read(4): cuatro, y gcount 4");
    mira((long long)i.tellg() == 4, "tellg: en el 4");
    i.seekg(7, std::ios::beg);
    char c[4] = {};
    i.read(c, 2);
    mira(std::string(c) == "78" && (long long)i.tellg() == 9, "seekg(7, beg) y read(2): \"78\", y tellg 9");
    i.seekg(-3, std::ios::cur);
    mira((long long)i.tellg() == 6, "seekg(-3, cur): el 6");
    char d[8] = {};
    i.read(d, 8);
    mira(i.gcount() == 4 && std::string(d) == "6789" && i.rdstate() == (std::ios::eofbit | std::ios::failbit), "read de mas: los que hay, y eofbit|failbit");
    mira((long long)i.tellg() == -1, "tellg tras un fallo: -1");
    i.seekg(0, std::ios::beg);
    bool sigue = i.rdstate() == std::ios::failbit;
    i.clear();
    i.seekg(0, std::ios::beg);
    mira(sigue && i.good() && (long long)i.tellg() == 0, "seekg quita eofbit pero no failbit; tras clear, vuelve al 0");
    i.seekg(99, std::ios::beg);
    mira(i.rdstate() == std::ios::failbit, "seekg fuera: failbit");
}

static void con_fichero_ancho() {
    wchar_t ruta[MAX_PATH];
    GetModuleFileNameW(nullptr, ruta, MAX_PATH);
    std::ifstream f(ruta, std::ios::in | std::ios::binary | std::ios::ate);
    long long fin = f ? (long long)f.tellg() : 0;
    std::ifstream no(L"no_existe_bmo.xyz");
    mira(fin > 1000 && !no.is_open(), "ifstream de un nombre ancho (_Fiopen de wchar_t): su propio .exe al final, y el que no hay");
}

static void con_contexto() {
    auto c = concurrency::task_continuation_context::use_default();
    unsigned long long primero;
    unsigned char en_linea;
    memcpy(&primero, &c, 8);
    memcpy(&en_linea, (const char *)&c + 8, 1);
    mira(sizeof(c) == 16 && primero == 1 && en_linea == 0, "task_continuation_context::use_default: captura diferida (1) y sin correr en linea");
}

int main() {
    con_numeros();
    con_lectura();
    con_fichero_ancho();
    con_contexto();
    di("tanda20m.exe: las ultimas DURAS, compiladas por MSVC, son las de Windows\r\n");
    return (int)fallos;
}
