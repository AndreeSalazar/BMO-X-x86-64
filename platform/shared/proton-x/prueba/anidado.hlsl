cbuffer B : register(b0) { float4 k; };
struct E { float4 pos : SV_Position; float2 uv : TEXCOORD0; };
float4 pixel(E e) : SV_Target {
    float s = 0;
    [loop] for (int i = 0; i < 3; i++) {
        [loop] for (int j = 0; j < 4; j++) {
            [branch] if (j == 1) continue;
            s += e.uv.x * k.x;
            [branch] if (s > k.y) break;
        }
        s = s * 0.5;
    }
    float t = e.uv.y > 0.5 ? s : -s;
    return float4(s, t, 0, 1);
}
