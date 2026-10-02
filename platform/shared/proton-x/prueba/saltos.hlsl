cbuffer B : register(b0) { float4 k; };
struct E { float4 pos : SV_Position; float2 uv : TEXCOORD0; };
float4 pixel(E e) : SV_Target {
    float acc = 0;
    [branch] if (e.uv.x < e.uv.y) { acc = e.uv.x * k.x; } else { acc = e.uv.y + k.y; }
    [loop] for (int i = 0; i < 4; i++) {
        acc += k.z;
        [branch] if (acc > k.w) break;
    }
    return float4(acc, e.uv, 1);
}
