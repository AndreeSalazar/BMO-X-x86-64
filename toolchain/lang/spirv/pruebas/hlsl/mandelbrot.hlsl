// mandelbrot, en HLSL: un pixel por invocacion, el mismo calculo que el GLSL.
RWStructuredBuffer<uint> vueltas : register(u0);
cbuffer Ventana : register(b1) { float2 origen; float2 paso; uint ancho; uint tope; };
[numthreads(8, 8, 1)]
void main(uint3 id : SV_DispatchThreadID) {
    uint2 p = id.xy;
    float2 c = origen + float2(p) * paso;
    float2 z = float2(0.0, 0.0);
    uint k = 0;
    while (k < tope && dot(z, z) <= 4.0) {
        z = float2(z.x * z.x - z.y * z.y, 2.0 * z.x * z.y) + c;
        k++;
    }
    vueltas[p.y * ancho + p.x] = k;
}
