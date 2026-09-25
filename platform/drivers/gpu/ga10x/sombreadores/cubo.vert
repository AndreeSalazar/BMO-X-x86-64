#version 450
// VERRANO V0 -- el sombreador de VERTICE del cubo.
//
// Toma la posicion (ya en coordenadas de recorte: las cuentas del juez las
// hace la CPU) y el color de la cara de un buffer. Nada cambia de un
// fotograma a otro salvo los DATOS: el programa se compila UNA vez y viaja
// ya traducido en el BSF (kind SM86). Su SASS, a mano, en `src/tuberia.rs`.

struct Vertice {
    vec4 posicion;
    vec4 color;
};

layout(std430, set = 0, binding = 0) readonly buffer Vertices {
    Vertice v[];
};

layout(location = 0) flat out vec4 color;

void main() {
    gl_Position = v[gl_VertexIndex].posicion;
    color = v[gl_VertexIndex].color;
}
