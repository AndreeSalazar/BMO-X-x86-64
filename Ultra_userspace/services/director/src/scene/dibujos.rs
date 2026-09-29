// **LOS DIBUJOS VECTORIALES DEL SISTEMA** (2026-09-29): solo datos, en la
// rejilla de 24 (cuartos: `0..96`) de `bmo_dibujo::icono`. Los pinta
// `iconos::vector`; el porque de las reglas, en la cabecera de
// `platform/shared/bmo-dibujo/src/icono.rs`.
//
// Los papeles de la paleta (los pone quien pinta, ver `iconos::paleta`):
//
//    0 cuerpo    el color de la clase o de la unidad
//    1 claro     el cuerpo aclarado (la luz de arriba)
//    2 oscuro    el cuerpo oscurecido (la sombra de abajo, las ranuras)
//    3 contorno  casi negro, fijo: recorta la silueta sobre cualquier fondo
//    4 luz       el piloto del disco (verde montado, rojo no)
//    5 brillo    casi blanco: el filo que coge la luz
//    6 ambar     el aviso (solo lectura)
//
// [!] Sin comentarios `//!` a proposito: el banco del anfitrion lo incluye con
// `include!` para verlo en PNG antes de ir al metal.

/// **El disco**: una losa vista un poco desde arriba, con su cara de arriba
/// clara, el frente en degradado, el piloto y dos ranuras.
pub(crate) const DISCO: [Capa; 8] = [
    (Figura::Poligono(&[(17, 22), (79, 22), (92, 44), (4, 44)]), Tinta::Lisa(3)),
    (Figura::Caja { x: 4, y: 40, w: 88, h: 34, r: 7 }, Tinta::Lisa(3)),
    (Figura::Poligono(&[(19, 25), (77, 25), (88, 43), (8, 43)]), Tinta::Degradado(5, 1)),
    (Figura::Caja { x: 7, y: 43, w: 82, h: 28, r: 5 }, Tinta::Degradado(0, 2)),
    (Figura::Trazo { puntos: &[(10, 44), (86, 44)], ancho: 2 }, Tinta::Lisa(1)),
    (Figura::Circulo { x: 19, y: 57, r: 5 }, Tinta::Lisa(4)),
    (Figura::Trazo { puntos: &[(44, 53), (78, 53)], ancho: 3 }, Tinta::Lisa(2)),
    (Figura::Trazo { puntos: &[(44, 61), (78, 61)], ancho: 3 }, Tinta::Lisa(2)),
];

/// **ESTRATOS**: tres capas apiladas, la de arriba la mas clara. Es lo que el
/// volumen ES: estratos que se ponen encima y no se sobreescriben.
pub(crate) const ESTRATOS: [Capa; 6] = [
    (Figura::Poligono(&[(48, 40), (93, 62), (48, 84), (3, 62)]), Tinta::Lisa(3)),
    (Figura::Poligono(&[(48, 44), (86, 62), (48, 80), (10, 62)]), Tinta::Degradado(0, 2)),
    (Figura::Poligono(&[(48, 26), (93, 48), (48, 70), (3, 48)]), Tinta::Lisa(3)),
    (Figura::Poligono(&[(48, 30), (86, 48), (48, 66), (10, 48)]), Tinta::Degradado(1, 0)),
    (Figura::Poligono(&[(48, 12), (93, 34), (48, 56), (3, 34)]), Tinta::Lisa(3)),
    (Figura::Poligono(&[(48, 16), (86, 34), (48, 52), (10, 34)]), Tinta::Degradado(5, 1)),
];

/// **EFI, el arranque**: un chip -- el firmware -- con sus patas.
pub(crate) const CHIP: [Capa; 8] = [
    (Figura::Trazo { puntos: &[(36, 8), (36, 88)], ancho: 6 }, Tinta::Lisa(2)),
    (Figura::Trazo { puntos: &[(60, 8), (60, 88)], ancho: 6 }, Tinta::Lisa(2)),
    (Figura::Trazo { puntos: &[(8, 36), (88, 36)], ancho: 6 }, Tinta::Lisa(2)),
    (Figura::Trazo { puntos: &[(8, 60), (88, 60)], ancho: 6 }, Tinta::Lisa(2)),
    (Figura::Caja { x: 16, y: 16, w: 64, h: 64, r: 11 }, Tinta::Lisa(3)),
    (Figura::Caja { x: 19, y: 19, w: 58, h: 58, r: 9 }, Tinta::Degradado(0, 2)),
    (Figura::Caja { x: 33, y: 33, w: 30, h: 30, r: 5 }, Tinta::Degradado(5, 1)),
    (Figura::Circulo { x: 28, y: 28, r: 3 }, Tinta::Lisa(1)),
];

/// **El candado de SOLO LECTURA**, para poner encima de otro icono (abajo a
/// la derecha): un circulo ambar con el candado oscuro.
pub(crate) const CANDADO: [Capa; 4] = [
    (Figura::Circulo { x: 74, y: 74, r: 22 }, Tinta::Lisa(3)),
    (Figura::Circulo { x: 74, y: 74, r: 19 }, Tinta::Lisa(6)),
    (Figura::Trazo { puntos: &[(67, 72), (67, 66), (70, 61), (74, 60), (78, 61), (81, 66), (81, 72)], ancho: 4 }, Tinta::Lisa(3)),
    (Figura::Caja { x: 62, y: 70, w: 24, h: 17, r: 3 }, Tinta::Lisa(3)),
];

/// **La carpeta**: la solapa detras, el frente delante y mas claro arriba.
pub(crate) const CARPETA: [Capa; 5] = [
    (Figura::Caja { x: 6, y: 14, w: 38, h: 20, r: 6 }, Tinta::Lisa(3)),
    (Figura::Caja { x: 6, y: 22, w: 84, h: 64, r: 8 }, Tinta::Lisa(3)),
    (Figura::Caja { x: 9, y: 17, w: 32, h: 16, r: 4 }, Tinta::Lisa(2)),
    (Figura::Caja { x: 9, y: 25, w: 78, h: 58, r: 6 }, Tinta::Lisa(2)),
    (Figura::Caja { x: 9, y: 34, w: 78, h: 49, r: 6 }, Tinta::Degradado(1, 0)),
];

/// **La hoja**: con la esquina doblada y tres renglones.
pub(crate) const HOJA: [Capa; 7] = [
    (Figura::Caja { x: 14, y: 4, w: 68, h: 88, r: 8 }, Tinta::Lisa(3)),
    (Figura::Caja { x: 17, y: 7, w: 62, h: 82, r: 6 }, Tinta::Degradado(1, 0)),
    (Figura::Poligono(&[(56, 3), (83, 3), (83, 30)]), Tinta::Fondo),
    (Figura::Poligono(&[(55, 6), (79, 30), (61, 30), (55, 24)]), Tinta::Lisa(3)),
    (Figura::Trazo { puntos: &[(30, 48), (66, 48)], ancho: 5 }, Tinta::Lisa(2)),
    (Figura::Trazo { puntos: &[(30, 60), (66, 60)], ancho: 5 }, Tinta::Lisa(2)),
    (Figura::Trazo { puntos: &[(30, 72), (54, 72)], ancho: 5 }, Tinta::Lisa(2)),
];
