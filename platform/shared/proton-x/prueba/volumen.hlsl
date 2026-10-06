// volumen.hlsl -- los sombreadores de volumen.exe (06-10): los UAV de una
// textura 3D y de un ARRAY de texturas 2D, escritos y leidos por computo
// (la niebla volumetrica de un juego vive en uno asi; las cascadas de
// sombras, en un array). Todo entero o float exacto.

// u0: un 3D de 8 x 8 x 4 (RGBA32F), entero. u1: el MISMO, con la vista de
// FirstWSlice = 1 y WSize = 2. u2: un array de 3 capas de 4 x 4 con 2 mips
// (R32_UINT), con la vista de MipSlice = 1, FirstArraySlice = 1 y
// ArraySize = 2. u3: un 3D de 2 x 2 x 2 (R32_UINT) para contar. u4: lo que
// se lee, de vuelta.
RWTexture3D<float4> vol : register(u0);
RWTexture3D<float4> medio : register(u1);
RWTexture2DArray<uint> capas : register(u2);
RWTexture3D<uint> cuenta : register(u3);
RWByteAddressBuffer sal : register(u4);

// A y D: cada hilo de 8 x 8 x 4, su texel; y suma 1 en la celda de su
// paridad (256 hilos, 32 por celda).
[numthreads(4, 4, 2)]
void CSEscribe(uint3 id : SV_DispatchThreadID) {
    vol[id] = float4(id.x, id.y, id.z, id.x + 8 * id.y + 64 * id.z);
    uint antes;
    InterlockedAdd(cuenta[id & 1], 1, antes);
}

// B y C: 2 x 2 x 4 hilos; la vista de `medio` y la de `capas` tienen DOS
// rebanadas: lo de z = 2 y 3 cae fuera y no se escribe.
[numthreads(2, 2, 4)]
void CSMedio(uint3 id : SV_DispatchThreadID) {
    medio[id] = float4(-1.0, -2.0, -3.0, 100 + id.z);
    capas[id] = 1000 * id.z + 10 * id.y + id.x + 7;
}

// E: las medidas de las tres vistas y unas lecturas, a `sal`.
[numthreads(1, 1, 1)]
void CSLee() {
    uint w, h, d, w2, h2, d2, w3, h3, e3;
    vol.GetDimensions(w, h, d);
    medio.GetDimensions(w2, h2, d2);
    capas.GetDimensions(w3, h3, e3);
    sal.Store4(0, uint4(w, h, d, w2));
    sal.Store4(16, uint4(h2, d2, w3, h3));
    sal.Store(32, e3);
    sal.Store4(48, asuint(vol[uint3(5, 6, 3)]));
    sal.Store4(64, asuint(medio[uint3(1, 0, 1)]));
    sal.Store4(80, asuint(vol[uint3(0, 0, 9)]));
    sal.Store(96, capas[uint3(1, 1, 1)]);
    sal.Store(100, cuenta[uint3(1, 1, 1)]);
    sal.Store(104, capas[uint3(0, 0, 5)]);
}
