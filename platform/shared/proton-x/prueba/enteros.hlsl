cbuffer B : register(b0) { float4 k; int4 n; };
struct E { float4 pos : SV_Position; float2 uv : TEXCOORD0; };
float4 pixel(E e) : SV_Target {
    int a = (int)(e.uv.x * 16.0);
    uint b = (uint)(e.uv.y * 8.0);
    int c = a * n.x + (a << 2) - (int)(b >> 1);
    uint d = (b ^ 0x5u) | ((uint)a & 0xFu);
    int m = min(max(c, n.y), n.z);
    uint u = min(d, (uint)n.w);
    float r = 0;
    [branch] switch (a & 3) {
        case 0: r = (float)c; break;
        case 1: r = (float)d * 0.5; break;
        case 2:
        case 3: r = (float)(m - (int)u); break;
        default: r = -1; break;
    }
    bool par = (b & 1u) == 0u;
    return float4(r, (float)m, (float)u, par ? 1.0 : (float)(c >> 3));
}
