// escena.hlsl -- los sombreadores de escena.exe (A11, 06-10): una escena 3D
// DURA, la que el propietario pidio ("un test en 3D duro en Windows, y que se
// refleje en BMO-X"). Un terreno de 48 x 48 cuadros con relieve y una
// textura con mips, 64 cubos por instancias, una luz con SOMBRA (un mapa de
// profundidad y SampleCmp), un vidrio con mezcla, todo en HDR (RGBA16F) y un
// tonemap por computo a RGBA8.

cbuffer Camara : register(b0) {
    float4x4 vista_proy;  // la de la camara
    float4x4 luz_proy;    // la de la luz (la del mapa de sombra)
    float4 hacia_luz;     // xyz: hacia la luz; w: intensidad
    float4 tiempo;        // x: el fotograma
};

struct Vertice {
    float3 pos : POSITION;
    float3 nrm : NORMAL;
    float2 uv : TEXCOORD0;
};

struct Instancia {
    float4 donde : INSTANCIA0;  // xyz: el centro; w: el lado
    float4 color : INSTANCIA1;  // rgb: el color; a: 1 si es un cubo, 0 el terreno
};

struct Salida {
    float4 pos : SV_Position;
    float3 mundo : TEXCOORD0;
    float3 nrm : TEXCOORD1;
    float2 uv : TEXCOORD2;
    float4 color : COLOR0;
};

Salida VSEscena(Vertice v, Instancia i) {
    Salida s;
    float3 p = v.pos * i.donde.w + i.donde.xyz;
    s.mundo = p;
    s.pos = mul(vista_proy, float4(p, 1));
    s.nrm = v.nrm;
    s.uv = v.uv;
    s.color = i.color;
    return s;
}

// La sombra: solo la profundidad vista desde la luz.
float4 VSSombra(Vertice v, Instancia i) : SV_Position {
    float3 p = v.pos * i.donde.w + i.donde.xyz;
    return mul(luz_proy, float4(p, 1));
}

Texture2D<float4> tablero : register(t0);
Texture2D<float> sombra : register(t1);
SamplerState lineal : register(s0);
SamplerComparisonState compara : register(s1);

float4 PSEscena(Salida s) : SV_Target {
    float4 base = s.color.a > 0.5 ? float4(s.color.rgb, 1) : tablero.Sample(lineal, s.uv * 6.0);
    // La sombra: donde cae el punto en el mapa de la luz, y SampleCmp con 4
    // vecinos (el PCF de los juegos).
    float4 l = mul(luz_proy, float4(s.mundo, 1));
    float2 t = l.xy / l.w * float2(0.5, -0.5) + 0.5;
    float z = l.z / l.w - 0.002;
    float luz = 0;
    [unroll] for (int k = 0; k < 4; k++) {
        float2 o = float2(k & 1, k >> 1) - 0.5;
        luz += sombra.SampleCmpLevelZero(compara, t + o / 512.0, z);
    }
    luz *= 0.25;
    float d = saturate(dot(normalize(s.nrm), hacia_luz.xyz));
    float3 c = base.rgb * (0.15 + hacia_luz.w * d * luz);
    return float4(c, 1);
}

// El vidrio: un color con alfa, mezclado sobre lo que haya.
float4 PSVidrio(Salida s) : SV_Target {
    return float4(0.2, 0.6, 1.5, 0.35);
}

// El tonemap: de HDR a RGBA8 (Reinhard, y la raiz que hace de gamma).
Texture2D<float4> hdr : register(t2);
RWTexture2D<unorm float4> final : register(u0);

[numthreads(8, 8, 1)]
void CSTono(uint2 p : SV_DispatchThreadID) {
    uint w, h;
    final.GetDimensions(w, h);
    if (p.x >= w || p.y >= h)
        return;
    float3 c = hdr.Load(int3(p, 0)).rgb;
    c = c / (1 + c);
    final[p] = float4(sqrt(c), 1);
}
