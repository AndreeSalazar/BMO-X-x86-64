// guids.cpp -- los IID de D3D12 y DXGI como SIMBOLOS (`IID_ID3D12Device`...),
// para lo que los nombra sin __uuidof (d3dx12, en HelloTexture). Es lo que en
// MSVC da dxguid.lib: aqui, INITGUID sobre las mismas cabeceras de
// DirectX-Headers con las que se compila la muestra.
#define INITGUID
#include <windows.h>
#include <initguid.h>
#include <d3d12.h>
#include <dxgi1_6.h>
