// instancias.hlsl -- los sombreadores de instancias.exe (N5.13 de
// docs/plan/PLAN_LAS_TRES_GRANDES.md, 05-10): las INSTANCIAS y los buferes
// de vertices de varias ranuras. Cuentas exactas en float: cada borde cae en
// un borde de pixel de un destino de 64 x 64, y cada color es k / 255.

struct Sale {
    float4 pos : SV_Position;
    float4 color : COLOR;
};

// A: la esquina del cuadro POR VERTICE (ranura 0), su sitio y su color POR
// INSTANCIA (ranura 1, cada instancia) y su fila POR INSTANCIA (ranura 2,
// cada DOS instancias); el verde es SV_InstanceID (que cuenta desde 0).
Sale VSInst(float2 esquina : ESQUINA, float2 sitio : SITIO, float4 color : COLOR, float fila : FILA, uint inst : SV_InstanceID) {
    Sale s;
    float2 p = float2(sitio.x, fila) + esquina * 6.0;
    s.pos = float4(p.x / 32.0 - 1.0, 1.0 - p.y / 32.0, 0.5, 1.0);
    s.color = float4(color.r, inst * 16.0 / 255.0, color.b, 1.0);
    return s;
}

// B: SIN bufer de vertices: el rectangulo sale de SV_VertexID (las esquinas
// x de los vertices 1, 4 y 5 y las y de los 2, 3 y 5, en bits), una banda
// por instancia.
Sale VSId(uint id : SV_VertexID, uint inst : SV_InstanceID) {
    Sale s;
    float2 e = float2((0x32u >> id) & 1u, (0x2Cu >> id) & 1u);
    float2 p = float2(4.0 + e.x * 56.0, 50.0 + inst * 6.0 + e.y * 4.0);
    s.pos = float4(p.x / 32.0 - 1.0, 1.0 - p.y / 32.0, 0.5, 1.0);
    s.color = float4(1.0, inst * 100.0 / 255.0, 0.0, 1.0);
    return s;
}

float4 PSColor(Sale s) : SV_Target {
    return s.color;
}
