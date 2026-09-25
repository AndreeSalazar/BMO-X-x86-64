#version 450
// VERRANO V0 -- el sombreador de PIXEL del cubo: el color de la cara, tal
// cual. Su SASS, a mano, en `src/tuberia.rs`.

layout(location = 0) flat in vec4 color;
layout(location = 0) out vec4 salida;

void main() {
    salida = color;
}
