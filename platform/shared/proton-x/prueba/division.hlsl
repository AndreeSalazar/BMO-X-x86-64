cbuffer B : register(b0) { float4 k; int4 n; };
struct E { float4 pos : SV_Position; float2 uv : TEXCOORD0; };
float4 pixel(E e) : SV_Target {
    int a = (int)(e.uv.x * 1000.0);
    uint ua = (uint)(e.uv.y * 100000.0);
    int q = a / n.x;
    int r = a % n.x;
    uint uq = ua / (uint)n.y;
    uint ur = ua % (uint)n.y;
    // Las cifras de ua en base n.z: dividir dentro de un bucle.
    uint s = 0;
    uint x = ua;
    [loop] for (int i = 0; i < 8 && x != 0u; i++) {
        s += x % (uint)n.z;
        x /= (uint)n.z;
    }
    return float4((float)q, (float)r, (float)(uq + ur * 3u), (float)s + (float)(a / 4));
}
