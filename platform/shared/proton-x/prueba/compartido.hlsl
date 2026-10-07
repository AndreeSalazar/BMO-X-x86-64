// compartido.hlsl -- 18 de la pila A de PLAN_LAS_TRES_GRANDES (07-10): lo
// que dos CS de Cyberpunk pidieron en el metal (06-10): `OperacionD3d(48)`
// (IMad: `mad()` de enteros) e `Instruccion(38)` (`atomicrmw` de LLVM: un
// Interlocked* sobre la memoria COMPARTIDA del grupo). Cada hilo de un grupo
// de 64 suma, mira el minimo y el maximo, enciende su bit y cambia un
// contador `groupshared`; tras la barrera, el hilo 0 escribe lo que quedo, y
// cada hilo lo que le devolvieron sus Interlocked (el valor de ANTES). Lo que
// se juzga no depende del orden de los hilos: lo que quedo; que los "antes"
// de la suma son todos distintos; y que UNO solo vio el cambio sin hacer.
RWStructuredBuffer<uint4> salida : register(u0);
groupshared uint suma;
groupshared uint menor;
groupshared int mayor;
groupshared uint bits;
groupshared uint ultimo;

[numthreads(64, 1, 1)]
void CSMain(uint gi : SV_GroupIndex, uint3 g : SV_GroupID)
{
    if (gi == 0) {
        suma = 0;
        menor = 0xFFFFFFFF;
        mayor = -1000;
        bits = 0;
        ultimo = 0;
    }
    GroupMemoryBarrierWithGroupSync();
    int k = (int)gi;
    uint v = (uint)mad(k, -3, 200);      // IMad
    uint w = mad(gi, 5u, g.x * 1000u + 1u);   // UMad
    uint antes;
    InterlockedAdd(suma, w, antes);
    InterlockedMin(menor, v);
    InterlockedMax(mayor, k - 30);
    InterlockedOr(bits, 1u << (gi & 31));
    uint cambiado;
    InterlockedExchange(ultimo, 7, cambiado);
    GroupMemoryBarrierWithGroupSync();
    salida[g.x * 65 + gi] = uint4(v, w, antes, cambiado == 0 ? 1 : 0);
    if (gi == 0)
        salida[g.x * 65 + 64] = uint4(suma, menor, (uint)mayor, bits);
}
