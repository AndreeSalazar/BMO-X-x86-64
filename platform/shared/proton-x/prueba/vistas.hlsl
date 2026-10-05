// vistas.hlsl -- los sombreadores de vistas.exe (N5.3b y N5.3c de
// docs/plan/PLAN_LAS_TRES_GRANDES.md, 05-10): las vistas en la RAIZ y los
// UAV de TEXTURA y con tipo. Cuentas exactas: enteros, y colores k / 255.

// A: todo en la RAIZ. Un SRV estructurado de 32 bytes por elemento (el paso
// sale de los metadatos: la vista de la raiz no lo lleva), un UAV
// estructurado de 8 y uno crudo.
struct Par {
    float4 a;
    float4 b;
};
StructuredBuffer<Par> pares : register(t0);
RWStructuredBuffer<uint2> sumas : register(u0);
RWByteAddressBuffer crudo : register(u1);

[numthreads(8, 1, 1)]
void CSRaiz(uint i : SV_DispatchThreadID) {
    Par p = pares[i];
    sumas[i] = uint2(p.a.x + p.b.y, p.a.w * 2.0);
    crudo.Store(i * 4, i * 3 + 1);
}

// B: un RWTexture2D de una tabla, con sus medidas de GetDimensions.
RWTexture2D<float4> imagen : register(u0, space1);

[numthreads(4, 4, 1)]
void CSImagen(uint2 xy : SV_DispatchThreadID) {
    uint w, h;
    imagen.GetDimensions(w, h);
    imagen[xy] = float4(xy.x / (float)(w - 1), xy.y / (float)(h - 1), 0.5, 1.0);
}

// C: la misma textura LEIDA como UAV, a un RWBuffer con tipo (la vista es
// R8G8B8A8_UNORM: el elemento se escribe en su formato).
RWTexture2D<float4> fuente : register(u0, space2);
RWBuffer<float4> destino : register(u1, space2);

[numthreads(4, 4, 1)]
void CSLee(uint2 xy : SV_DispatchThreadID) {
    float4 c = fuente[xy];
    destino[xy.y * 16 + xy.x] = c.bgra;
}
