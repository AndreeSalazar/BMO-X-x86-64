# VERIFICAR -- banco completo, coste visible

`bmo.ps1` y la ventana VERIFICAR siguen usando `comprobar.ps1`. No hace falta
una bandera nueva. `-Rapido` conserva su significado explicito: omite el banco
de aplicaciones; los guardianes siguen siendo parte del proceso.

## Que cambia

- El reloj de VERIFICAR empieza antes del banco, no despues.
- Build y VERIFICAR guardan tiempos por etapa en `target/verificacion/`.
- El banco guarda stdout, stderr y JSON por corrida (PID y UUID), con tiempo total y por paquete.
  Estos identificadores son del verificador; no identifican un intento del GSP.
- `banco.py` construye primero todos los objetivos por la orden original
  `cargo test --workspace --exclude bmo-kernel --no-fail-fast --no-run`.
  Esto conserva incluso la comprobacion de compilacion de los ejemplos.
- Luego Cargo ejecuta unidades/binarios, integracion y documentacion con dos
  procesos como maximo. Cada uno conserva la seleccion del workspace, entorno,
  directorio y harness administrados por Cargo. No se lanzan ejecutables a mano.
- El presupuesto de threads por binario es la mitad del CPU, salvo que el
  propietario ya haya definido `RUST_TEST_THREADS`. No se modifica el perfil release.

Cargo ejecuta normalmente los binarios de pruebas en secuencia, aunque cada
binario pueda ejecutar varias pruebas a la vez. Las opciones de seleccion se
contrastaron con la [documentacion de Cargo](https://doc.rust-lang.org/cargo/commands/cargo-test.html).
Los logs de cada grupo se concatenan EN EL MISMO ORDEN en ambos canales:
la pareja anuncio/resultado no depende del orden en que terminen los procesos.

## Cobertura antes que velocidad

El workspace se consulta; no hay lista manual de paquetes. `test=false` y
`doctest=false` se respetan. Si excluir un objetivo pudiera perder features,
se vuelve a la orden original completa. Solo se admite el reparto cuando las
features por defecto se conservan por otro root activo, o el paquete excluido
esta completamente aislado. Clases nuevas, harness propio, ejemplos con
pruebas o casos mixtos no representables tambien vuelven al banco original.
Nunca se usa una aprobacion antigua para omitir una comprobacion.

El codigo nativo de Cargo se captura inmediatamente: un fallo sin la palabra
`error` tambien rechaza el banco. Metadata vacia, anuncios sin resultado y
resultados sin propietario se rechazan. Una sola pareja es un array, no un
caracter de una cadena. Se conserva --no-fail-fast y se espera a ambos carriles.

El 17-10 se comprobo otra causa de salida prematura: el texto `$salida` de
`comprobar.ps1` y `contrato.ps1` colisionaba con `$script:salida`, la bandera de
fallo. Una ejecucion anterior completo el banco y termino con codigo cero
ANTES de los guardianes. Los textos ahora tienen nombres propios; el cierre
solo se acepta al llegar a TODO VERIFICADO y al sello correspondiente.

## Autoproteccion

Ocho pruebas Python comprueban seleccion, preservacion de defaults, orden de
canales, limite de concurrencia y propagacion de errores. El banco ademas
corre su autoprueba PowerShell: seis escenarios de datos, inspeccion de AST
contra la colision de variables y ausencia de Python para guardian obligatorio.
Ambas se ejecutan desde comprobar.ps1, incluso con -Rapido si hay Python.

El guardian NVIDIA es obligatorio: sin Python no aprueba el build. Su S8
ademas ejecuta sus catorce pruebas negativas en cada --check. El supervisor
GSP sigue sin unsafe; esta optimizacion no cambia operaciones ni energia GPU.

## Medicion del cambio

El banco original completo 5361 pruebas y 393 anuncios de objetivos. La
referencia anterior termino prematuramente: no es una validacion completa
contra la que anunciar un porcentaje global de mejora.

Se optimiza en dev el frontend TITAN++ y su adaptador PROMETEO. Los asserts y
chequeos de desbordamiento del perfil de pruebas siguen activos. La primera
recompilacion del banco costo 37,49 s; no es tiempo de ejecucion de pruebas.
El objetivo cubo_gira paso de unos 50,8 s entre anuncios a 38,70 s de ejecucion
con las tres pruebas aprobadas. Son medidas con limites distintos, no una
comparacion exacta de ciclos ni una promesa general de FPS.

Validacion del 17-10:

| Medida | Resultado |
| --- | --- |
| Banco con cache de compilacion preparada | 314,21 s, 5361 pruebas aprobadas |
| Equivalencia con el banco original | 393 anuncios en ambos; ninguno perdido ni agregado |
| Carriles, tiempo de pared | construccion 6,47 s; integracion 307,37 s; unidades 166,93 s; documentacion 9,68 s |
| Guardianes y contrato despues de las correcciones | 53,6 s en pasada -Rapido; cierre TODO VERIFICADO |
| Pruebas de los paquetes afectados por las correcciones | titan-lector 87; proton-x-casa 71, incluidos tres casos nuevos |
| Compilacion release x86-64 | Director, TALLER y PROTON-X aprobados |
| Pila TALLER | 41632/65536 bytes |

Los tiempos de carriles se solapan: no se suman para anunciar tiempo total.
Los 53,6 s corresponden a una pasada separada de guardianes, con el banco ya
validado; no se presentan como un build integral ni se omite el coste de
compilacion inicial. El banco previo al ultimo arreglo tenia 5361 pruebas;
las tres nuevas del cerrojo se validaron en el paquete completo afectado.

Dos deudas que antes quedaban tapadas por la salida prematura se corrigieron:
el catalogo generado de TALLER estaba en 47 ejemplos y ahora refleja los 48;
el registro de sombreadores rechazados de PROTON-X tenia un Sync pendiente
que excedia el trinquete. Ahora usa CAS Acquire/Release y una guardia RAII,
sin retornar referencias mutables estaticas. Reentrada o contencion rechazan
la captura diagnostica sin esperar; el motivo del rechazo sigue disponible.
Cambiar la carpeta devuelve `false` si esta ocupada y la app lo informa.
No se subio el limite 62 del guardian de hilos. Los tests cubren exclusion,
reentrada, liberacion al desenrollar la pila y 8000 mutaciones desde ocho hilos.

Los datos generados quedan en target, fuera del commit. Si se interrumpe una
corrida, sus logs parciales no acreditan una aprobacion ni una mejora.
