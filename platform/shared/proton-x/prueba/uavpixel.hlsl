// uavpixel.hlsl -- los sombreadores de uavpixel.exe (05-10, la fila "un UAV
// en un sombreador de DIBUJO" de la 7.2 de
// docs/plan/PLAN_LA_ESCALERA_PROTON_X.md): los UAV escritos desde un
// dibujo. Todo entero: nada que redondear con una GPU de verdad.

RWTexture2D<uint> posiciones : register(u1);
RWByteAddressBuffer contador : register(u2);
RWBuffer<uint> vertices : register(u3);

// C: un cuadro de pantalla completa sin bufer de vertices (las esquinas x de
// los vertices 1, 4 y 5 y las y de los 2, 3 y 5, en bits), y cada vertice
// escribe 100 + su numero en SU elemento.
float4 VSCuadro(uint id : SV_VertexID) : SV_Position {
    vertices[id] = 100u + id;
    float2 e = float2((0x32u >> id) & 1u, (0x2Cu >> id) & 1u);
    return float4(e.x * 2.0 - 1.0, 1.0 - e.y * 2.0, 0.5, 1.0);
}

// A: cada pixel escribe su posicion en SU texel: y en los 16 bits de
// arriba, x en los de abajo.
float4 PSPosicion(float4 p : SV_Position) : SV_Target {
    uint2 xy = uint2(p.xy);
    posiciones[xy] = (xy.y << 16) | xy.x;
    return float4(0.0, 1.0, 0.0, 1.0);
}

// B: cada pixel suma 1 al contador. No lee NADA: todos los pixeles le dan
// las mismas entradas, y aun asi cada uno tiene que correr (y sumar).
float4 PSCuenta() : SV_Target {
    contador.InterlockedAdd(0, 1);
    return float4(1.0, 0.0, 0.0, 1.0);
}

// Solo para el banco (`src/pruebas_uav.rs`), no para el .exe: B con
// `[earlydepthstencil]`, la profundidad ANTES: un pixel tapado no suma.
[earlydepthstencil]
float4 PSCuentaTemprana() : SV_Target {
    contador.InterlockedAdd(0, 1);
    return float4(1.0, 0.0, 0.0, 1.0);
}
