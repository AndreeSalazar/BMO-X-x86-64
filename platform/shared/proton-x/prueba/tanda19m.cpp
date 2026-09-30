// tanda19m.cpp -- la TANDA 19 compilada por MSVC con SUS cabeceras (30-09):
// los flujos de msvcp140 como los usa el juego (el codigo inline de
// <sstream>, <fstream>, <iomanip> y <locale>). tanda19.exe (clang) no sirve
// de oraculo aqui: la base virtual de ostream y el locale que vuelve por
// valor son justo donde clang y MSVC tienen que coincidir, y en el Windows
// del propietario no coincidieron. Este lo hace MSVC, como el juego.
//
// En la consola de "x64 Native Tools Command Prompt for VS":
//   cl /nologo /EHsc /MD /O1 /std:c++17 tanda19m.cpp /Fe:tanda19m.exe
// Dice `bien` o `MAL` por comprobacion (y lo que salio, si no es lo
// esperado) y sale con el numero de fallos.
#include <windows.h>

#include <ctime>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <locale>
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

// Con lo que salio, si no es lo esperado.
static void igual(const std::string &salio, const char *esperado, const char *que) {
    bool ok = salio == esperado;
    mira(ok, que);
    if (!ok) {
        di("        salio \"");
        di(salio.c_str());
        di("\"\r\n");
    }
}

static void con_ostream() {
    std::ostringstream o;
    mira(o.good() && o.flags() == (std::ios::skipws | std::ios::dec) && o.fill() == ' ' && o.precision() == 6 && o.width() == 0, "ostringstream: su ios (skipws|dec, relleno ' ', precision 6)");
    o << 42 << ' ' << -7 << ' ' << 4000000000u << ' ' << 123ul << ' ' << 18446744073709551615ull;
    o.write("abc", 3);
    igual(o.str(), "42 -7 4000000000 123 18446744073709551615abc", "<< int, unsigned, unsigned long, unsigned long long; write");
    o.str("");
    o << std::hex << 255 << ' ' << -1 << ' ' << std::showbase << std::uppercase << 255 << std::nouppercase << std::noshowbase << std::dec;
    igual(o.str(), "ff ffffffff 0XFF", "hex, showbase y uppercase (manipuladores de ios_base)");
    o.str("");
    o << std::setw(5) << 7 << std::left << std::setw(4) << 3 << std::internal << std::showpos << std::setw(5) << 9 << std::noshowpos << std::right << std::setfill('*') << std::setw(3) << 1 << std::setfill(' ') << 5;
    igual(o.str(), "    7" "3   " "+   9" "**1" "5", "setw, left, internal con showpos, setfill; y el ancho se gasta");
    o.str("");
    o << 1 << std::endl;
    o.put('x');
    o.flush();
    igual(o.str(), "1\nx", "endl (un manipulador de ostream), put y flush");
    o.setstate(std::ios::failbit);
    bool fallo = o.rdstate() == std::ios::failbit;
    o << 5;
    o.clear();
    mira(fallo && o.rdstate() == 0 && o.str() == "1\nx", "setstate(failbit): no escribe; clear() lo quita");
    std::locale l = o.getloc();
    mira(o.widen('z') == 'z' && l.name() == "C", "widen y getloc: el locale \"C\"");
}

static void sin_streambuf() {
    {
        std::ostream nada(nullptr);
        nada.clear();
        mira(nada.rdstate() == std::ios::badbit, "ostream sin streambuf: badbit, y clear no lo quita");
    }
    mira(true, "ostream sin streambuf: se destruye");
}

static void con_istream() {
    {
        std::istringstream i("hola");
        char b[5] = {};
        i.rdbuf()->sgetn(b, 4);
        mira(i.gcount() == 0 && std::string(b) == "hola" && i.good(), "istringstream: su streambuf se lee");
    }
    std::stringstream io;
    io << 12;
    char b[3] = {};
    io.rdbuf()->sgetn(b, 2);
    mira(std::string(b) == "12" && io.good(), "stringstream (iostream): se escribe y se lee por el mismo streambuf");
}

static void con_cerr() {
    std::cerr << "  (cerr) hola\r\n";
    mira(std::cerr.good() && (std::cerr.flags() & std::ios::unitbuf), "cerr: unitbuf; escribe");
}

static void con_fichero() {
    char ruta[MAX_PATH];
    GetModuleFileNameA(nullptr, ruta, MAX_PATH);
    std::ifstream f(ruta, std::ios::binary);
    char mz[3] = {};
    bool abre = f.is_open() && f.rdbuf()->sgetn(mz, 2) == 2 && std::string(mz) == "MZ";
    std::ifstream no("no_existe_bmo.xyz");
    mira(abre && !no.is_open(), "ifstream (_Fiopen): el propio .exe empieza por MZ; el que no hay no abre");
}

static void con_tiempo() {
    std::tm t = {};
    t.tm_sec = 9;
    t.tm_min = 5;
    t.tm_hour = 13;
    t.tm_mday = 30;
    t.tm_mon = 8;
    t.tm_year = 126;
    t.tm_wday = 3;
    t.tm_yday = 272;
    std::ostringstream o;
    o << std::put_time(&t, "%Y-%m-%d %H:%M:%S %a %b %j %p %x %% %#m %A %B %I");
    igual(o.str(), "2026-09-30 13:05:09 Wed Sep 273 PM 09/30/26 % 9 Wednesday September 01", "put_time (time_put) con el locale \"C\"");
}

static void con_facetas() {
    std::locale l;
    const auto &ct = std::use_facet<std::ctype<char>>(l);
    const auto &cv = std::use_facet<std::codecvt<char, char, std::mbstate_t>>(l);
    mira(ct.tolower('Q') == 'q' && ct.is(std::ctype_base::digit, '7') && cv.always_noconv(), "use_facet: ctype<char> y codecvt<char,char>");
}

int main() {
    con_ostream();
    sin_streambuf();
    con_istream();
    con_cerr();
    con_fichero();
    con_tiempo();
    con_facetas();
    di("tanda19m.exe: los flujos de msvcp140, compilados por MSVC, son los de Windows\r\n");
    return (int)fallos;
}
