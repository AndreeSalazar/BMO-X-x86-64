# Objetos en TITAN++ -- capacidades reales y siguiente ampliacion

16-10-2026. El propietario pide ampliar la reutilizacion para MUNDO y otros
programas, tras estudiar su Minecraft. Esto revisa el compilador actual;
no anuncia una implementacion completa de clases Java.

## Lo que ya funciona

| Necesidad | TITAN++ actual | Limite |
| --- | --- | --- |
| datos con nombre | `type`, campos y registros anidados | todos los campos se inicializan |
| construir a partir de piezas | composicion de registros | no hay herencia de clases |
| comportamiento compartido | `trait` y `trait X for T` | contrato completo y firmas comprobadas |
| funcion reutilizable | parametro de trait | no es un tipo para campos, resultados o listas |
| cambiar el mismo valor | `mut` en parametro y llamada, tambien campos anidados | exclusividad conservadora del propietario completo |
| limites de acceso | `mod`, `use`, `pub` | visibilidad de declaraciones; no prometer campos privados por clase |
| propiedad y transferencia | copia, `mut`, `take` | no equivale al modelo de referencias compartidas de Java |

La regla vigente en TITAN_MAESTRO 14.13 es composicion + trait. Su vocabulario
tiene 25 palabras y un techo de 30. Ampliar capacidades no obliga a introducir
`class`, `extends`, `impl` y `self` como cuatro palabras nuevas.

## Prueba concreta: bloques que se pueden romper

[bloques.titan](ejemplos/nivel10/bloques.titan) define `Stone` y `Wood`, ambos
con un componente `Durability`. Comparten la funcion `reduce` y cumplen
`Breakable`; una sola `mine` trabaja con ambos. La madera responde distinto
al mismo golpe, y el llamador observa el cambio en el bloque original.

No se uso un objeto por celda del terreno: este ejemplo estudia comportamiento.
Para millones de bloques, elegir la representacion de datos requiere medir
memoria, construccion de caras y acceso a vecinos.

[composition.rs](emisor-x86_64/tests/composition.rs) fuerza E1 para ejecutar
las llamadas y mutaciones en x86-64, sin resolver todo al compilar. Prueba:
dos implementaciones, componente anidado, fuerza cero, fuerza excesiva y
rechazo T0085 de un tipo que no cumple el contrato.

## Hallazgos y resultado de O1

1. Antes de O1, una funcion propia rechazaba
   `reduce(mut block.durability, force)` con T0077. **Implementado el 16-10:**
   se admite el campo y cadenas como `mut actor.body.state.value`, tambien
   entre modulos y al pasar el componente a un trait. La IR conserva el
   propietario y los campos; el calculo devuelve el cambio al campo correcto,
   y E1 pasa su direccion. Para un trait se conserva el envoltorio existente
   con etiqueta y retorno del componente: no se copia el contenedor entero.
   No se habilitan indices en funciones propias ni `take` parcial. Las
   primitivas de colecciones conservan su camino previo para indices.
2. E1 usa una etiqueta de tipo y un dispatcher de comparaciones y salto
   para el trait. Es codigo nativo, pero tiene coste. La documentacion antigua
   prometia seleccion estatica para E1; se corrigio para describir lo real.
3. Al construir un registro con `[0; 3000]`, E1 agotaba la pila por construir
   la tabla y el registro en temporales antes de copiar al local. Se corrige
   la construccion de registros NUEVOS: campos y registros anidados van a
   su destino; las tablas repetidas compatibles se llenan alli. Se mantienen
   conversiones y comprobaciones PIC. Las reasignaciones conservan su camino
   anterior para evaluar el valor nuevo antes de soltar el viejo. La prueba
   antes daba T0066 y ahora coincide con el calculo. Otras pruebas comprueban
   que la expresion repetida se evalua una sola vez y que un valor decimal
   fuera de limites sigue dando T0074 en la misma linea que el calculo.

## Orden propuesto de ampliacion

- [x] Verificar composicion, contrato y mutacion nativa con dos tipos de bloque.
- [x] O1: prestar campos a funciones propias. La linea de llegada es
      `reduce(mut block.durability, force)` sin copiar el registro completo.
      Debe rechazar prestar el mismo campo dos veces, leer o prestar el
      contenedor mientras su campo esta prestado, y cambiar un valor inmutable.
      Antes de admitir campos hermanos disjuntos se demuestra que no se solapan.
      Comprobador, propiedad, IR, calculo y emisor actualizados; no se agregan
      palabras. Dieciseis pruebas comparan E1 con calculo, o verifican el mismo
      rechazo en los dos. Incluyen campos escalares, componentes con monton,
      tabla de 3000 elementos, limites de tipos, modulos y contrato trait.
      `take` parcial necesita reglas aparte. La camara de MUNDO ya usa O1
      y el propietario confirma jugar con los controles nuevos el 16-10.
      Eso acredita ese uso del componente, no las dieciseis variantes del
      banco en metal; alias, errores y componentes grandes conservan sus
      pruebas del anfitrion y sus limites documentados.
- [ ] O2: especializar llamadas de trait cuando el tipo concreto es conocido;
      comparar instrucciones y bytes contra llamada directa, sin perder T0085.
      Conservar un camino correcto cuando se necesite elegir en ejecucion.
- [ ] O3: definir si hacen falta cuerpos por defecto, extension de contratos
      y acceso mediante metodos. Cada uno requiere sus reglas de ambiguedad
      y visibilidad antes de sumar sintaxis.
      **18-10, cuerpos por defecto hechos** (decision del propietario): una
      fn de `trait` con cuerpo lo da por defecto; se juzga una vez como
      generico del trait (T0073 si mira un campo) y cada tipo que no la
      escribe recibe su copia. Sin palabras nuevas. Calculo y E1 coinciden
      en [por_defecto](ejemplos/nivel10/por_defecto.titan). Faltan la llamada
      con punto (`piedra.golpea(3)`) y la extension de contratos.
- [ ] O4: evaluar colecciones de valores de tipos distintos con un contrato
      comun, solo ante un uso concreto. Definir representacion, propietario,
      coste y despacho; no basta permitir `[Trait]` en el parser.

O1 y O2 responden a obstaculos observados. O3/O4 son propuestas, no capacidades
disponibles ni ampliaciones implementadas de la gramatica. Esta tanda implementa
O1, agrega el ejemplo y sus pruebas; O2 es el siguiente cambio del compilador.

La lectura del juego que motivo este estudio vive en
[MUNDO](../../../Ultra_userspace/apps/mundo/ESTUDIO_MINECRAFT.md).

Lo que cada app del propietario le pide a TITAN++, hasta ILLAPA (el Houdini
propio), y el orden propuesto: [LAS_APPS](LAS_APPS.md).
