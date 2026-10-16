# Minecraft 26.3 local -- primera lectura para MUNDO

16-10-2026. El propietario pide estudiar su instalacion para construir
compatibilidad nativa en TITAN++ y VERRANO. Esta lectura no ejecuta Java,
no modifica el juego y no incorpora sus recursos al repositorio.

## Lo encontrado en esta PC

- Instalacion: `C:/Users/Salazar/AppData/Roaming/.minecraft/`.
- Version de la captura y del manifiesto: `26.3`, release; tambien esta
  instalada `26.4-snapshot-4`, que no es la referencia de esta tanda.
- Cliente: `versions/26.3/26.3.jar`, 41.483.720 bytes.
- SHA256: `4508d006323f24fa02876310c192d739af56516eb259000ac50f0909a68c9a2d`.
- El manifiesto pide Java 25. Las clases observadas tienen major 69.
- 11.383 clases compiladas, cero fuentes `.java` en ese archivo.
- 1.300 PNG de bloques, 2.883 modelos JSON y 1.288 JSON de estados.
- Mundo `saves/New World`: nueve archivos de terreno `.mca` bajo
  `dimensions/minecraft/overworld/region/`. No estan en la antigua carpeta
  `region/` de la raiz del mundo. Entidades y puntos de interes van aparte.

Estos numeros describen la copia local. No se descargaron paquetes ni se
leyeron cuentas del launcher. El mundo estaba abierto: esta lista no es una
copia consistente de sus chunks; para una futura importacion se cierra el
mundo y se trabaja sobre una copia.

## Que podemos estudiar del codigo

La herramienta propia [study_minecraft.py](tools/study_minecraft.py) lee el
constant pool y las firmas de seis clases, y mide los bytes de su bytecode.
No descompila ni analiza el flujo de esos metodos. Los nombres permiten
ubicar responsabilidades; no demuestran por si solos como funciona cada una.

Las rutas siguientes son entradas internas del JAR, bajo `net/minecraft/`:

| Clase | Metodos declarados | Nombres observados | Trabajo equivalente de MUNDO |
| --- | ---: | --- | --- |
| `world/level/block/Block` | 85 | `shouldRenderFace`, `updateFromNeighbourShapes` | caras y vecinos |
| `world/level/block/state/BlockBehaviour` | 65 | `getCollisionShape`, `tick`, `rotate` | propiedades, colision y reglas |
| `client/Camera` | 49 | `move`, `setRotation`, `setupPerspective` | MC2: posicion, inclinacion y proyeccion |
| `client/player/LocalPlayer` | 113 | `move`, `pick`, `updateAutoJump` | MC2/MC3: apuntado y jugador |
| `client/renderer/LevelRenderer` | 73 | `compileSections`, `prepareChunkRenders`, `render` | construir, seleccionar y enviar trozos |
| `client/renderer/chunk/SectionCompiler` | 7 | `compile` | convertir bloques en geometria |

Los recuentos incluyen constructores, sobrecargas y metodos sinteticos.
No se ha reconstruido la generacion del mundo ni la logica de criaturas.

## Primera pieza concreta: un bloque

En las entradas `assets/minecraft/models/block/` se observo:

| Bloque | Datos del modelo | Necesidad nativa |
| --- | --- | --- |
| piedra y tierra | heredan `cube_all`, una textura para todas las caras | resolver el modelo padre y aplicar UV |
| hierba | dos elementos, suelo de tierra, cara superior, lateral y overlay; cinco caras con indice de tinte | material por cara, tinte y recorte de transparencia |
| tronco | hereda `cube_column`, extremos distintos de los lados | orientacion y dos texturas |
| hojas | heredan `leaves` | estudiar el padre antes de definir su transparencia |
| tablas | heredan `cube_all` | igual recorrido basico que piedra |

Las ocho texturas basicas inspeccionadas miden 16 x 16. No basta pegar la
imagen de hierba en seis caras: el modelo diferencia superficies y tinte.
Los modelos heredados no tienen por que declarar sus elementos localmente;
`elements = 0` en el inventario no significa un bloque invisible.

Recorrido propuesto: estado del bloque -> modelo resuelto -> caras visibles
con UV y material -> vertices de MUNDO -> VERRANO. Es un esquema propuesto,
no una descripcion completa del recorrido interno del cliente Java.

## Que cambia al escribirlo en TITAN++

Se puede guardar el mundo como identificadores de bloques y propiedades,
usar funciones para camara, colision y construccion de caras, y dejar el
dibujo a VERRANO. No hace falta reproducir la jerarquia de clases de Java.
Una sintaxis mas facil no implementa por si misma redstone, luz o generacion:
esas reglas requieren codigo y pruebas aunque se expresen con menos lineas.

MUNDO hoy publica posicion y color por vertice; sus 80 caras visibles son
el limite actual. Aplicar texturas exige un recorrido con UV, materiales,
muestreo y recursos persistentes del backend. Que haya pruebas de texturas
en otras puertas de VERRANO no significa que la lamina actual ya las acepte.

## Metas que salen de esta lectura

- [x] Localizar la instalacion, version y mundo de referencia.
- [x] Inventariar recursos y firmas sin ejecutar Java ni extraer el juego.
- [ ] MC2a de [PLAN_MUNDO](../../../docs/plan/PLAN_MUNDO.md): inclinacion,
      mira y rayo 3D, como siguiente cambio jugable.
- [ ] Resolver un modelo `cube_all` y sus seis caras; probar primero piedra
      y tierra, y rechazar una referencia de textura o padre inexistente.
- [ ] Llevar UV y una textura al recorrido nativo de VERRANO; comparar una
      escena fija antes de agregar la hierba con tinte y overlay.
- [ ] Leer una copia cerrada de un chunk del mundo local, sin reemplazar
      bloques desconocidos silenciosamente ni prometer aun un mundo entero.

Inventario generado en `target/estudio/minecraft/inventory.json` (ignorado
por Git). Para regenerarlo desde la raiz, pasar la ruta `.minecraft` a
`Ultra_userspace/apps/mundo/tools/study_minecraft.py`; admite `--version`,
`--world` y `--out`. Solo escribe el inventario, nunca los archivos de origen.
