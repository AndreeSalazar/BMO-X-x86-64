// antes.h -- el puente de MSVC a mingw-w64 para compilar las muestras de
// Microsoft SIN tocar su fuente (se incluye delante con -include; E1.0 de
// docs/plan/PLAN_LA_ESCALERA_PROTON_X.md, 05-10). Tres cosas que solo acepta
// MSVC, y ninguna cambia lo que hace la muestra. La cuarta va en la linea de
// ordenes: `-fpermissive`, por `&CD3DX12_RESOURCE_BARRIER::Transition(...)`
// (la direccion de un temporal, que vive hasta el final de la expresion).
#pragma once
#include <windows.h>
#include <rpc.h>
#include <rpcndr.h>
#include <wrl.h>

// MSVC acepta `_uuidof` (un guion) como sinonimo de `__uuidof`.
#define _uuidof __uuidof

// `Microsoft::WRL::Wrappers::FileHandle` (corewrappers.h de MSVC): un HANDLE
// que se cierra solo. La WRL de mingw-w64 no lo trae.
namespace Microsoft { namespace WRL { namespace Wrappers {
class FileHandle {
public:
    explicit FileHandle(HANDLE h = INVALID_HANDLE_VALUE) : h_(h) {}
    ~FileHandle() { if (h_ != INVALID_HANDLE_VALUE && h_ != nullptr) CloseHandle(h_); }
    FileHandle(const FileHandle&) = delete;
    FileHandle& operator=(const FileHandle&) = delete;
    HANDLE Get() const { return h_; }
private:
    HANDLE h_;
};
}}}

// Los metodos COM que DEVUELVEN una estructura. MSVC los escribe `h =
// heap->GetCPUDescriptorHandleForHeapStart()`; DirectX-Headers, con mingw-w64,
// los declara con su ABI de verdad: el valor vuelve por un puntero oculto
// (`...(&h)`), que es exactamente lo que el compilador de Microsoft emite. La
// llamada de maquina es LA MISMA en los dos (rcx = this, rdx = &h, rax = &h):
// el .exe llama a la casa igual que el de MSVC. Las cabeceras van antes de la
// macro para que sus declaraciones no la vean.
#include <d3d12.h>
#include <dxgi1_6.h>
#include <d3dx12.h>
// Los IID para __uuidof con mingw-w64 (DirectX-Headers los trae para eso).
#include <dxguids/dxguids.h>
static D3D12_CPU_DESCRIPTOR_HANDLE bmo_puente_cpu;
static D3D12_GPU_DESCRIPTOR_HANDLE bmo_puente_gpu;
#define GetCPUDescriptorHandleForHeapStart() GetCPUDescriptorHandleForHeapStart(&bmo_puente_cpu)[0]
#define GetGPUDescriptorHandleForHeapStart() GetGPUDescriptorHandleForHeapStart(&bmo_puente_gpu)[0]

// `min` y `max` sueltos: el windows.h de Microsoft los da como macros (sin
// NOMINMAX); el de mingw-w64, en C++, no. Los de la biblioteca de C++.
#include <algorithm>
using std::max;
using std::min;

// Lo que usa `L` como nombre (DirectXMath, la biblioteca de C++), ANTES de la
// macro de abajo: con sus guardas, cuando la muestra los incluya ya estan.
#include <DirectXMath.h>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

// `L#x` (el nombre de una variable como cadena ANCHA): MSVC lo une en
// L"x"; GCC ve el identificador `L` y luego "x". Las muestras de E2 solo lo
// usan para NOMBRAR objetos (`NAME_D3D12_OBJECT`: SetName, que en release
// no hace nada). `L` pasa a ser "una cadena estrecha a ancha": el `.exe`
// llama a SetName con el nombre, como el de MSVC. Un `L"..."` de verdad es
// otro token y no lo ve; `L##q` (la macro TEXT de Windows) tampoco: se pega
// antes de expandir.
struct BmoAncha {};
inline const wchar_t* operator+(BmoAncha, const char* s) {
    static wchar_t buf[8][128];
    static unsigned k = 0;
    wchar_t* w = buf[k++ % 8];
    unsigned i = 0;
    for (; s[i] && i < 127; i++) w[i] = (unsigned char)s[i];
    w[i] = 0;
    return w;
}
#define L BmoAncha{} +
