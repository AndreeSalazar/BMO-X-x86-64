cbuffer B : register(b0) { float4 k; };
struct E { float4 pos : SV_Position; float2 uv : TEXCOORD0; };
float4 pixel(E e) : SV_Target {
    float x = e.uv.x;
    float y = e.uv.y;
    [loop] while (x < k.x) {
        x = x * 2 + k.y;
        [branch] if (x != y) { y = y + 1; } else { y = y - 1; }
    }
    return float4(x, y, 0, 1);
}
