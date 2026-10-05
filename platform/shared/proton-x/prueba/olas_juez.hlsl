// olas_juez.hlsl -- los sombreadores de olas.exe (E2.5 de
// docs/plan/PLAN_LA_ESCALERA_PROTON_X.md, 05-10): las OLAS de verdad.
//
// CSOlas: 2 grupos de 64 hilos (dos olas de 32 cada uno); cada hilo escribe
// N palabras con lo que le dio cada operacion de ola, sobre valores que
// salen de su SV_GroupIndex (i). olas.cpp sabe lo que tiene que salir.
// VSOlas y PSOlas: dibujos cuyos pixeles leen a sus vecinos de cuadro.

#define N 26

RWStructuredBuffer<uint> salida : register(u0);

[numthreads(64, 1, 1)]
void CSOlas(uint i : SV_GroupIndex, uint3 g : SV_GroupID) {
    uint t = (g.x * 64 + i) * N;
    // Un valor por hilo, revuelto (el multiplicador de Knuth).
    uint x = i * 2654435761u + g.x;
    salida[t + 0] = WaveGetLaneCount();
    salida[t + 1] = WaveGetLaneIndex();
    salida[t + 2] = WaveIsFirstLane() ? 1 : 0;
    salida[t + 3] = WaveActiveSum(i);
    salida[t + 4] = WaveActiveProduct(i % 3 == 0 ? 3u : 1u);
    salida[t + 5] = asuint(WaveActiveMin(int(i) - 40));
    salida[t + 6] = WaveActiveMax(x);
    salida[t + 7] = WaveActiveBitAnd(x | 0x0F0F0F0Fu);
    salida[t + 8] = WaveActiveBitOr(x & 0x00FF00FFu);
    salida[t + 9] = WaveActiveBitXor(x);
    salida[t + 10] = WaveActiveCountBits(i % 3 == 0);
    uint4 p = WaveActiveBallot(i % 5 == 0 || i == 33);
    salida[t + 11] = p.x;
    salida[t + 12] = p.y | p.z | p.w;
    salida[t + 13] = WaveReadLaneAt(x, 5);
    salida[t + 14] = WaveReadLaneFirst(x);
    salida[t + 15] = WavePrefixSum(i);
    salida[t + 16] = WavePrefixProduct(i % 4 == 1 ? 3u : 1u);
    salida[t + 17] = WavePrefixCountBits(i % 2 == 1);
    salida[t + 18] = (WaveActiveAllEqual(i / 8) ? 1 : 0) | (WaveActiveAllEqual(g.x) ? 2 : 0) | (WaveActiveAnyTrue(i == 37) ? 4 : 0) | (WaveActiveAllTrue(i < 60) ? 8 : 0);
    // Floats: medios, que se suman EXACTOS en cualquier orden.
    float f = float(i) * 0.5f;
    salida[t + 19] = asuint(WaveActiveSum(f));
    salida[t + 20] = asuint(WavePrefixSum(f));
    salida[t + 21] = asuint(WaveActiveMin(1.0f - f));
    // Con signo y sin el: en la segunda ola, -8..23 (sin signo, -1 seria el mayor).
    salida[t + 22] = asuint(WaveActiveMax(int(i) - 40));
    // Dentro de un si: solo los de la rama son los activos.
    uint r;
    if (i % 3 == 0)
        r = WaveActiveSum(i) * 1000 + WaveReadLaneFirst(i);
    else
        r = WaveActiveCountBits(true) * 1000 + WavePrefixCountBits(true);
    salida[t + 23] = r;
    // El bucle de ESCALARIZAR (el de los juegos): cada vuelta, el primer
    // activo dice un valor y salen los que lo tienen.
    uint v = (i * 7) % 5, vueltas = 0;
    while (true) {
        vueltas++;
        if (WaveReadLaneFirst(v) == v)
            break;
    }
    salida[t + 24] = vueltas;
    // Un bucle del que se sale en vueltas distintas.
    uint s = 0;
    for (uint k = 0; k < i % 4; k++)
        s += WaveActiveCountBits(true);
    salida[t + 25] = s;
}

// Los vertices: 0..5 el destino entero (dos triangulos); 6..8 uno chico que
// cubre SOLO el centro del pixel (2, 2) de 64 x 64.
static const float2 P[9] = {
    float2(-1, 1), float2(1, 1), float2(-1, -1), float2(-1, -1), float2(1, 1), float2(1, -1),
    float2(2.2 / 32 - 1, 1 - 2.2 / 32), float2(2.9 / 32 - 1, 1 - 2.2 / 32), float2(2.2 / 32 - 1, 1 - 2.9 / 32),
};

float4 VSOlas(uint id : SV_VertexID) : SV_Position {
    return float4(P[id], 0, 1);
}

// R el x del vecino de al lado, G el y del de arriba o abajo, B ocho
// pruebas que tienen que salir (255), A los carriles activos de su ola.
float4 PSOlas(float4 pos : SV_Position) : SV_Target {
    uint px = uint(pos.x), py = uint(pos.y);
    uint b = 0;
    if (QuadReadAcrossDiagonal(px) == (px ^ 1) && QuadReadAcrossDiagonal(py) == (py ^ 1))
        b |= 1;
    if (QuadReadLaneAt(px, 0) == (px & ~1u) && QuadReadLaneAt(py, 3) == (py | 1))
        b |= 2;
    if (WaveGetLaneCount() == 32)
        b |= 4;
    uint n = WaveActiveCountBits(true);
    if (WaveActiveSum(1u) == n)
        b |= 8;
    if (WavePrefixCountBits(true) < n)
        b |= 16;
    uint4 m = WaveActiveBallot(true);
    uint yo = WaveGetLaneIndex();
    if (((m.x >> yo) & 1) == 1 && (m.y | m.z | m.w) == 0)
        b |= 32;
    // El primero: el bit mas bajo de la papeleta; el mayor, el mas alto.
    uint f = WaveReadLaneFirst(yo);
    if ((m.x & ((1u << f) - 1)) == 0 && ((m.x >> f) & 1) == 1)
        b |= 64;
    if ((m.x >> WaveActiveMax(yo)) == 1)
        b |= 128;
    return float4(QuadReadAcrossX(px), QuadReadAcrossY(py), b, n) / 255.0;
}
