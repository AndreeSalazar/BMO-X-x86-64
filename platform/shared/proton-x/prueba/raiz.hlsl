// raiz.hlsl -- la root signature del cubo de P3b2, escrita como la entiende
// dxc: las banderas y UN descriptor de constantes (b0) en la raiz. Compilada
// con `dxc -T rootsig_1_0 -E Raiz` da el blob serializado (RTS0) que D3D12
// espera: el testigo del serializador de la casa (prueba/HACER.txt).
#define Raiz "RootFlags(ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT), CBV(b0)"
