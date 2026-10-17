# EL FOCO -- estado vigente y orden de trabajo

**Actualizado el 16-10-2026.** Este bloque decide el trabajo actual. Las
notas de tandas anteriores y el plan acumulado conservan su fecha; no
sustituyen el ultimo resultado. [Indice de documentos](../README.md).

## Ahora: G0, que la RTX arranque de forma fiable

El [plan GSP-CONTROL-BMO](PLAN_GSP_CONTROL_BMO.md) se redefinio el 17-10:
el GSP ejecuta y BMO-X lo GOBIERNA (dar posesion, conceder, pedir, escuchar,
revocar). El estudio del RISC-V y el firmware propio interno quedan APARCADOS.
GCB0/GCB1 ya implementados: contrato, supervisor puro con ocho pruebas y
`gpu control`, incluido en `save`. El resumen indica etapa, datos crudos y
build/intento ausentes; no arma recuperacion ni cambia el arranque.
Siguiente: GCB2 (G0), build e intento reales y las entradas del booter
contra OpenRM 570.144 antes de probar otra secuencia.

MUNDO es el cliente nativo de prueba. La introduccion de Cyberpunk es un
hito demostrado y se conserva como tal; retomarlo no desplaza el bloqueo
de arranque de la GPU ni convierte esa introduccion en un juego jugable.

| Orden | Meta | Evidencia de llegada |
| --- | --- | --- |
| 1 | G0: localizar y corregir el arranque intermitente | GSP activo en las corridas de validacion del plan; un cambio de diagnostico no cierra la meta. |
| 2 | Q0a3: aislar los cuatro pixeles diferentes | Escena comparable, GPU confirmada y juez sin diferencias; conservar C1. |
| 3 | MUNDO: resaltado, controles mantenidos y fisica | Bloque apuntado visible; salto, gravedad y colision comprobados. |
| 4 | TITAN++: O2 y maduracion del modelo | Medir especializacion de traits contra llamada directa y conservar rechazos correctos. |

## Lo cumplido y lo que sigue abierto

| Pieza | Estado vigente | Limite |
| --- | --- | --- |
| MUNDO funcional y guardado | Confirmados por el propietario el 16-10 | No confirma todos los casos de errores o cortes electricos. |
| Mirada relativa y captura del puntero | Implementadas; controles confirmados por el propietario | MC2b aun necesita el resaltado del bloque. |
| C1: buffers prestados a GPU | 325 fotogramas GPU, cero CPU en la tanda 19:37-19:39 | No acredita igualdad perfecta ni arranque fiable. |
| Q0a3: igualdad de imagen | Abierto: 4/230.400 pixeles distintos | Se investiga cuando la GPU este disponible. |
| Captura de pantalla completa | Dos PNG 1080p guardados en la tanda 20:36 | Ventana y recorte conservan su prueba propia. |
| G0: despertar GSP | Abierto: 0x15 en frio a las 23:23; argumentos legibles e intactos antes del booter | Despues aparecen BADF; causa sin demostrar. |
| FPS de MUNDO en la ultima tanda con app (20:36) | Muestra CPU de 19,60; 20 intervalos/1.020 ms | A las 23:23 no hubo lamina observada; no hay FPS nuevos. |
| Lentes GPU preparadas | Vistas a las 23:23: 64 B de shader y 32 B de ordenes; hex/bits contrastados | No es captura de VRAM ni del canal activo; solo esas ventanas. |
| M3a | Cerrado en anfitrion: -30,54 % de instrucciones al ordenar | No mide ciclos ni segundos en el Ryzen. |
| Lente de bytes | 128 bytes vistos, hex y bits coinciden | Rotulos corregidos, EOF y autopsia nueva aun requieren observacion. |
| Cyberpunk | Intro vista el 13-10, unos 16 s/fotograma por CPU | No jugable; dibujo del juego por GPU no acreditado. |

## Una tanda util, sin reinicios en serie

La ampliacion [Mesa/NVIDIA](../../platform/drivers/gpu/ga10x/INSPECCION_NVIDIA.md)
permite estudiar `gpu verrano ordenes` sin GSP. Es una plantilla preparada,
no un envio observado; G0 y Q0a3 conservan su evidencia de metal propia.

La [lente de GPU](../../platform/shared/bmo-lente-gpu/README.md) agrega
`gpu verrano bytes vs 128` con DIRECTOR nuevo, incluso sin GSP. Muestra
codigo preparado en CPU, no una lectura del aparato; observar esa salida
no cierra G0 ni exige reiniciar solo para consultarla.

La [auditoria RTX 3060 / PROTON-X](../../platform/drivers/gpu/ga10x/ESTUDIO_RTX3060_PROTON_X.md)
agrega G0-MODO como hipotesis de protocolo inicial y localiza el limite Z
de PROTON-X/VRN2. Son hallazgos de codigo; no cierran G0 ni Q0a3.

Con kernel y DIRECTOR nuevos: uso normal de MUNDO, `gpu laminas` y `save`,
funcione o falle la RTX. Si falla, comparar `GSP ANTES` con la autopsia
posterior. Si arranca, consultar tambien el juez de pixeles. No repetir
`gpu reintentar` cuando el descargador ya dejo WPR2 protegida.

Fuentes: [parte del 16-10](../metal/METAL_2026-10-16.md),
[diagnostico G0](../../platform/drivers/gpu/ga10x/DESPERTAR_FIABLE.md),
[controles](../../Ultra_userspace/apps/mundo/CONTROL.md).

## Evidencia y presentacion que faltan

- [ ] DOC-VIDEO: grabar una sesion reciente de MUNDO con backend visible,
      mirada, captura del puntero y guardado; no existe esa grabacion nueva.
- [ ] OBS-BUILD: incluir identificacion del commit/build en cada informe;
      falta implementar y comprobar que corresponde al binario desplegado.

La cronologia enlazada ya esta en el [indice](../README.md). Estas dos
metas de evidencia no bloquean el analisis del informe actual ni adelantan
Cyberpunk sobre G0. La portada renovada quedo en `985654ba9`.

<details>
<summary><strong>Notas previas del 16-10, conservadas como historial</strong></summary>

> **Quinta tanda 16-10, 20:36:** capturas 1080p guardadas; el propietario
> confirma controles nuevos. MUNDO: 1.141 fotogramas CPU, cero GPU; muestra
> de ventana 19,60 FPS. G0 vuelve a dar 0x15 EN FRIO y el descargador tambien
> falla: no bajo WPR2. Informe corregido para no insistir con reintentar.
> Nueva foto del GSP ANTES del booter lista en anfitrion; falta metal.
> Prioridad: G0 antes de Q0a3; no reinicios repetidos por rutina.
> [Datos y limite](../metal/METAL_2026-10-16.md).

> **MC2b, anfitrion:** raton relativo, cursor oculto al capturar, Esc suelta,
> clic recaptura y perdida de foco revoca. Modulo chico de TITAN++, sin
> ceder la entrada fisica. Medicion FPS de ventana incluida en `save`.
> Probar en metal con kernel/DIRECTOR/MUNDO nuevos; resaltar el bloque sigue
> pendiente. [Control y limites](../../Ultra_userspace/apps/mundo/CONTROL.md).

> **Cuarta tanda 16-10:** MUNDO SI dibuja en GPU: 325 fotogramas prestados,
> cero CPU. Quedan 4 pixeles distintos en el juez. Captura se quedo sin
> cupo de bloques: 1080p ya usa uno temporal en vez de dos, sin cambiar el
> kernel. Probar Impr Pant, ventana y recorte; traer `save` y PNG. Despues
> aislar los cuatro pixeles. [Evidencia](../metal/METAL_2026-10-16.md).

> **Diagnostico listo en anfitrion:** `gpu laminas` se conecta a la orden y
> a `save`: contadores GPU/CPU por app, etapa y motivo capturado al fallar.
> No dar por acelerado un juego solo porque sea visible. La corrida sigue
> siendo MUNDO + consulta + `save`, con kernel y DIRECTOR reconstruidos.
> [Uso](../../platform/shared/verrano/DIAGNOSTICO.md).

> **Tercera tanda 16-10, 17:06:** GPU despierta y lienzo correcto; MUNDO
> usa CPU porque VRN1 rechazaba 640x360 con Z antes del kernel. Correccion
> en anfitrion: region de las superficies fijas y copia de su medida real.
> Foco siguiente: probar MUNDO, `gpu laminas` y `save` con kernel/DIRECTOR
> reconstruidos; cerrar C1/Q0a3 solo con evidencia. G0 sigue abierto.
> [Causa, prueba y limites](../metal/METAL_2026-10-16.md).

> **Segunda tanda 16-10:** MUNDO funciona con respaldo CPU mientras el booter
> falla con 0x15 pese a llegada fria registrada. Aviso CALIENTE corregido;
> G0 sigue abierto. Guardado confirmado por el propietario, MC5 cerrado.
> MC2a (I/K, mira, rayo 3D y uso de O1) pasa en el anfitrion y espera metal.
> [Resultado](../metal/METAL_2026-10-16.md), [metas](PLAN_MUNDO.md).

> **Foco actual, 16-10: MUNDO nativo en TITAN++ y VERRANO.** El propietario
> confirma jugar y verlo funcionar en el metal: MC1 funcional cerrado.
> MC5 (guardar) pasa en el anfitrion; no se ha probado cerrar y recargar en
> metal. Siguiente: MC2a, inclinacion por teclado, mira y rayo en 3D;
> despues raton relativo y fisica. C1/Q0a3 GPU conserva su meta propia:
> el respaldo CPU no acredita render GPU. [Metas](PLAN_MUNDO.md).

> **Resultado del metal, 16-10:** GSP listo, init repetido correcto, VERRANO
> IGUAL a CPU/D3D12 y consulta de bits vista. El informe cazo offsets
> truncados solo en los rotulos: corregidos en el anfitrion. MUNDO informa
> C1 y Q0a3 sin completar, pese a la GPU despierta. Evidencia y limites:
> [`METAL_2026-10-16.md`](../metal/METAL_2026-10-16.md).

> **16-10, foco pedido ahora: M3a y la lente precisa de bytes.** M3a queda
> cerrado en el anfitrion (-30,54 % de instrucciones en ordenar, sin
> regresiones del banco). La vista cruda de la autopsia (hex y bits, hasta
> 16 bytes capturados desde RIP) compila y pasa la pila; falta verla en el
> Ryzen. El metro imprime ficheros completos con esa misma vista.
> [`Estudio y medidas`](../../toolchain/tools/metro/M3A_Y_LENTE.md).
> Cyberpunk sigue aparcado como se decidio abajo.

> **Continuacion del 16-10:** el DIRECTOR ya tiene `dentro bits OFFSET RUTA`,
> 64 bytes por pagina con hex y bits, y el comando para seguir. El juez puro
> de ventanas pasa sus pruebas; falta la comprobacion en el Ryzen. Hoja de
> esta tanda: [`LENTE_CRUDA_2026-10-16.md`](../metal/LENTE_CRUDA_2026-10-16.md).

</details>

## Plan acumulado y decisiones anteriores

El estado vigente es el bloque superior. Las consignas siguientes conservan
su contexto y no piden repetir una prueba ya registrada.

> Peticion del propietario, **2026-10-10**: *"enfoca en algo, terminar eso y
> luego seguimos [...] me gustaria que enfoques mas en TITAN++ y VERRANO con
> PROTON-X + INTI esos elementos y LAMINA la RAM, el INTI en NAVEGAR si
> eliminar porque ya TITAN++ lo toma, puedes organizar?"*
>
> [`ABIERTO.md`](ABIERTO.md) dice QUE falta; [`EL_ORDEN.md`](EL_ORDEN.md), con
> que CRITERIO. Esta pagina dice QUE SE HACE AHORA: UNA cosa, con su linea de
> llegada, y la cola detras. Lo que no esta aqui espera.

---

# 0. EL FOCO DEL 14-10: LA LAMINA, INTI, VERRANO Y LA LENTE -- de lo FACIL a lo DIFICIL

> El propietario, **2026-10-14**, despues de que Cyberpunk 2077 arrancara:
> *"es hora de esto, a completar en orden de MAS facil y mas dificil [...]
> RAM = convertir como LAMINA ABSOLUTA, LAMINA ese mismo para administrar y
> llamar, no quiero tener que lidiar como hizo C con C++ [...] y INTI y
> VERRANO [...] convertir mi Kernel que LEA PRECISO binarios 0 y 1"*.

**Cyberpunk se APARCA, no se abandona.** Arranco y pinta su intro: eso ya es
la prueba (README, `METAL_2026-10-13.md`). Lo que le falta es una cola de
piezas sueltas contra DXIL real (el back buffer, ~13 computos, instancias,
mas de 64 registros); queda entera y en orden en
[`LISTA_DEL_METAL.md`](../metal/LISTA_DEL_METAL.md), con lo medido el 14-10.
Se retoma el dia que se decida, con UNA corrida del metal antes (la lente de
derivadas, que nunca se corrio, y la carpeta `rechazados/`).

**Esperan, sin tocarse:** [`PLAN_LA_EDICION_AMD.md`](PLAN_LA_EDICION_AMD.md)
(pide una GPU de AMD en la mesa) y [`PLAN_EL_VIDEO.md`](PLAN_EL_VIDEO.md).

## 0a. LA LAMINA ABSOLUTA: que quiere decir

Hoy "lamina" es UNA cosa (los vertices de VERRANO, `BVER`) y cada bloque de
RAM que se presta entre dos lados tiene su forma y su trato, escritos a mano.
Contados el 14-10 (A1), son OCHO -- el borrador de esta seccion decia seis y
se habia dejado el canal y el bloque de computo --:

```text
   clase  donde                                          escribe -> lee        juez
   BSUP   bmo-abi .../surface/superficie.rs              app <-> DIRECTOR      Ring 3
   BVER   platform/shared/verrano/src/lamina.rs          app -> VERRANO        Ring 3
   LAMI   platform/shared/bmo-antena/src/lamina.rs       DIRECTOR -> NAVEGAR   Ring 3
   BGPU   platform/shared/bmo-pase-gpu/src/buzon.rs      escritorio <-> kernel Ring 0
   BRED   platform/shared/bmo-puerta-red/src/buzon.rs    proceso <-> kernel    Ring 0
   BMOC   platform/shared/bmo-channel/src/lib.rs         Ring 3 <-> Ring 0     Ring 0
   VE5C   platform/drivers/gpu/ga10x/.../completa.rs     app -> kernel, 3060   Ring 0
   CAB2   platform/services/cabina-core                  kernel -> CABINA      (sin uso hoy)
```

La tabla vive en el codigo (`bmo_lamina::censo::CENSO`), con la herramienta
de LA_RAM de cada una, su trato y su juez; esta es su copia para leer.

Ocho tratos para la misma idea. La LAMINA ABSOLUTA es que sean UNO: la
manera de que un programa tenga RAM, la PRESTE y la LLAME. Las reglas:

```text
   1  PEDIR ES TENER EN LA MANO   no hay malloc ni free sueltos: una lamina se
                                  pide con su medida y su CLASE, y para usarla
                                  hay que tenerla (la idea de MONTON.md, un
                                  piso mas arriba)
   2  DE QUIEN ES, SIEMPRE        cada lamina tiene UN propietario; el kernel la
                                  recoge cuando muere (como hoy `reap_dead`).
                                  Sin new/delete, sin destructores, sin quien
                                  libera dos veces: lo que no se le puede
                                  hacer a la RAM, no se escribe
   3  UN TRATO, UNA VEZ           la cabecera (magia, version, clase, medida,
                                  secuencia) y el SELLO por ranura del BVER --
                                  el que ya cazo el medio fotograma -- viven en
                                  UN crate; cada bloque de la tabla es una
                                  CLASE de ese trato, no un fichero aparte
   4  UN DATO, UN SITIO           las seis reglas de la LAMINA QUIROFANO (1e):
                                  se lee donde esta, se copia solo lo que la
                                  tabla dice
```

## 0a2. EL MAPA DE GUERRA: por que la lamina se administra SOLA

> El propietario, 14-10: *"la RAM se convierte ZONA DE MAPA DE GUERRA /
> QUIROFANO EXTREMO porque alli es donde se administra todo [...] LAMINA es
> inteligente por si sola y el kernel ya sabe que hacer y el CPU ya
> administraria bien TODO en TIEMPO REAL"*.

Leidos los documentos, la idea ya estaba en tres sitios y le faltaba juntarse:

```text
   LA_RAM.md (09-08)     el QUIROFANO: la RAM guarda lo de la operacion en
                         curso, no la app. Siete reglas (malloc no miente, lo
                         entregado es entero, copiar se cuenta, el destino es
                         una capability) y la Parte IX: SEIS herramientas, y de
                         cada transporte hay que poder decir cual es y por que
   MONTON.md (20-08)     pedir es TENERLO EN LA MANO; el estado vive dentro
                         del bloque, no en una global escondida
   1e de esta pagina     la LAMINA QUIROFANO: un dato, un sitio; una palabra
                         para saber si hay algo nuevo; nunca medio dato
```

Lo que la hace "inteligente por si sola" es que un bloque dice QUE ES en su
palabra 0, y que esa palabra esta en un MAPA: el censo. El que toma el bloque
no adivina, busca:

```text
   palabra 0  ->  censo::por_magia  ->  la clase: quien escribe, quien lee,
                                        que herramienta de LA_RAM, que trato,
                                        y si la lee Ring 0 (copiar una vez y
                                        juzgar la copia) o Ring 3
```

El DIRECTOR ya lo hacia a mano con dos clases (`scene/surface.rs`: `BVER` a
VERRANO, lo demas como superficie). Con el censo lo puede hacer el kernel con
las ocho, y lo que hoy decide cada puerta por su cuenta pasa a estar ESCRITO.
Eso es el mapa de guerra: cada bloque de la RAM compartida en el mapa, con
su bando (quien escribe), su frente (quien lee) y sus reglas de combate (el
trato). Un bloque que no esta en el mapa no existe para nadie: el guardian
`laminas` para el build.

Y el TIEMPO REAL sale del trato, no de la buena voluntad: el lector mira UNA
palabra (la secuencia) para saber si hay algo nuevo, no se bloquea nunca, y
lo que da por bueno es un dato entero y el ULTIMO publicado. El 14-10 la
prueba castigada del trato encontro que eso ultimo no estaba garantizado
(una de cada ~60 corridas, un fotograma iba hacia atras): el arreglo es una
palabra mas al final de cada lectura (`trato::vigente`), y ya esta en BVER.

## 0b. LA ESCALERA, de lo mas facil a lo mas dificil

Cada escalon entra con su prueba en el anfitrion; lo que pide el metal lo
dice, y el metal se junta en UNA corrida por escalon.

**El estado, de un vistazo (15-10).** Una pieza se TACHA entera cuando esta
hecha en el anfitrion Y vista en el Ryzen (o no necesita el Ryzen). Las que
solo esperan al Ryzen llevan dos subcasillas: `en el anfitrion` (tachada) y
`en el Ryzen` (abierta); lo que hay que mirar, y donde se anota que funciono,
esta en la HOJA 6b.

```text
   pieza  que                                  anfitrion   Ryzen
   A1     el censo de laminas con juez         HECHO       no hace falta
   A2     el trato uno                         HECHO       no hace falta
   A3     las clases al trato                  HECHO       no hace falta
   A4     TITAN++ pide RAM solo con laminas    HECHO       VISTO 15-10
   A5     la lente lee x86                     HECHO       no hace falta
   A6     el .bex dice que instrucciones usa   HECHO       no hace falta
   B1     verificar una vez y apartarse        HECHO       falta (6b: B1)
   B2     la autopsia dice la instruccion      HECHO       VISTO 15-10
   B3     el cubo en SU ventana, por la 3060   HECHO       falta (6b: B3)
   B4     el perfil, UNA fuente; LLVM obedece  HECHO       falta (6b: B4)
   C1     la lamina PRESTADA a la 3060         HECHO       falta (6b: C1)
   C2     S3: el banco duerme en el sello       HECHO       falta (6b: C2)
          S4: la app en su nucleo              BLOQUEADO   (ver C2)
   C3     el volcado por la 3060, por defecto HECHO     falta (6b: C3)
          las ventanas pegadas por la 3060     DECIDIR     (ver C3)
   C4     los contadores del Zen 3 (LB4)   HECHO       VISTO 15-10
          INTI y TITAN++ se cuentan (LB5)  HECHO       sin prueba propia en el metal
   D1     V4: cubo.bsf lo emite la casa    HECHO       falta (6b: D1)
          V3: la matriz en el cbuffer      HECHO (TITAN++, las 2 tarjetas); V3b, el
                                           escritorio dibujandolo, abierto
   D2                                          --          --
```

**Escalon 1 -- solo el anfitrion (facil)**

- [x] **A1 -- el CENSO de laminas con juez.** La tabla de 0a, en el codigo:
      un guardian que falla si aparece un bloque prestado nuevo que no este
      en ella. **Como se sabe:** se agrega una magia de mentira y el build
      cae. **HECHO el 14-10:** `bmo_lamina::censo` (las ocho clases, con su
      herramienta, trato y juez; `por_magia` y `conoce`) y el guardian
      `toolchain/tools/laminas/laminas.py`: las 37 constantes de magia del
      arbol son una clase del censo o un formato que no se presta, con su
      motivo; y ninguna fila del censo esta muerta. Saboteado con
      `MAGIA_FALSA`: cae con su fichero y su linea. La de la antena, que es
      texto, recibio su magia (`bmo_antena::lamina::MAGIA`, `LAMI`).
- [x] **A2 -- el TRATO uno** (`bmo-lamina`, puro, `no_std`): la cabecera
      comun y el sello por ranura, sacados de `verrano/src/lamina.rs`.
      `BVER` pasa a usarlo SIN MOVER UN BYTE. **Como se sabe:** la prueba
      de dos hilos de la lamina, igual; y la lamina que publica
      `cubogira.bex` es la misma byte a byte. **HECHO el 14-10:**
      `bmo_lamina::trato` (crear, abrir, las dos ranuras, empezar, acabar,
      mirar, intacto y `vigente`); BVER lo usa y sus campos son los de antes
      (un `assert` en tiempo de compilacion). La castigada del trato cazo
      que un lector podia dar por publicado un fotograma POSTERIOR y leer
      despues uno anterior; `uno_posterior_en_la_misma_ranura_es_rota` lo
      reproduce en VERRANO y `vigente` lo arregla (0 de 100 corridas
      castigadas). Las pruebas de TITAN++ que leen lo que escribe su
      emisor (`lamina`, `dos_cubos`, `mundo`, `navegar`), igual. **Falta el
      metal:** `gpu verrano banco inti` con `cubogira.bex`, `IGUAL al juez`.
- [x] **A3 -- las demas clases al trato**, una por commit, de la mas suelta a
      la mas atada. **Lo que se encontro al leerlas, y cambia la casilla:**
      lo UNICO que cumplen las ocho es la palabra 0 (la magia). La palabra 1
      cada una la puso a su manera antes del trato -- version entera (BVER,
      BGPU), media version (BRED: la otra mitad son las casillas), una medida
      (BSUP, VE5C), la mitad alta de una magia de 64 bits (BMOC), texto
      (LAMI) --. Igualar los BYTES tocaria el kernel, las cabeceras de C, el
      emisor de TITAN++ y el metal a la vez, por nada que se gane hoy: NO se
      movio un byte. **HECHO el 14-10, lo que si:** `censo::Version` dice la
      forma de la palabra 1 de cada clase y `censo::reconocer` es LA PUERTA
      UNICA -- dos palabras, la clase o por que no --. Cada clase esta atada a
      su fila en compilacion (`const _: () = assert!(...)`: si su version y la
      del censo se separan, no compila; saboteado en VERRANO, no compila): la
      antena, `bmo-abi` (BSUP), `cabina-core` (CAB2, declarada y sin uso),
      `bmo-pase-gpu`, `bmo-puerta-red`, `bmo-channel` y el DIRECTOR (VE5C:
      `bmo-gpu-ga10x` no tiene dependencias a proposito). Y la usan: el
      DIRECTOR toma lo que le ofrecen por la puerta unica (una clase que no
      es para el se devuelve con su nombre: `[lamina] tid N ofrecio un bloque
      BGPU`), el escritorio y el proceso dejan de usar un buzon BGPU o BRED
      con otra version, y BMOC, cuya magia nadie leia, se puede reconocer
      (`Channel::reconocido`). Los lados del KERNEL no cambian (copian una vez
      y juzgan la copia). **Como se sabe:** 268 pruebas de los ocho crates,
      el userspace entero y ESTUDIO para su objetivo, el kernel con
      `cargo check`, y los guardianes como estaban. **Falta el metal:** la
      ventana, el pase de la 3060 (`gpu pase`) y la red, como siempre.
- [x] **A4 -- TITAN++ pide RAM SOLO con laminas.** `director.lamina` deja de
      ser "la de vertices": una lamina con medida y clase, la que el programa
      tiene en la mano para administrar y llamar. **Como se sabe:** un
      ejemplo de TITAN++ que pide dos laminas, publica en una y lee la otra;
      y los NO del compilador (usarla sin tenerla, pasarse de su medida).
      Partida en dos el 14-10 (el propietario: *"es TITAN++ tomando llamadas
      a la RAM"*):
  - [x] **A4a -- LA PUERTA DE LA RAM, en el emisor.** Cinco sitios de E1
        pedian su bloque al kernel cada uno a su manera (el monton, la
        ventana, la lamina de VERRANO, lo tenido y el fichero). Ahora los
        cinco llaman a UNA subrutina, `Helper::PideLamina`
        (`emisor-x86_64/src/e1/ram.rs`), emitida una vez por programa.
        **Como se sabe:** `tests/puerta_ram.rs`, dos jueces que no se hablan:
        ningun fichero del emisor salvo `ram.rs` nombra la operacion de pedir
        memoria (saboteado en `director.rs`: cae); y corriendo en el
        emulador, cada peticion de RAM de un programa sale de la MISMA
        instruccion (el emulador apunta ahora el `rip` de cada llamada,
        `ObservedSyscall::rip`). **HECHO el 14-10.** **Encontrado por el
        camino, y ARREGLADO el 14-10:** el emulador daba CUATRO peticiones
        por proceso (`MEMORIA_MAX_PETICIONES`, que decia ser espejo del
        kernel) y el kernel da OCHO desde el 20-09 (`MEM_OP_SOLTAR`). Peor: el
        ejemplo `memoria_C.c` (`c/memc.bex`) comprobaba que la quinta
        devolviera 0, asi que en el Ryzen decia FALLO. Ahora el emulador da
        ocho, `memoria_C.c` y la sonda de C lo comprueban con ocho, y un juez
        (`bmo-lower`, `emu/espejo.rs`) lee el fuente del kernel y compara las
        dos cifras (saboteado a cuatro: cae). **Queda abierto, del kernel:**
        `vmm/verde.rs` justifica el hueco de `MEMORIA_VA_BASE` con "64 MiB x
        cuatro peticiones"; con ocho son 512 MiB, y como la VA de un bloque
        soltado no se reusa, un proceso que pide y suelta en bucle avanza su
        cursor sin techo. Hay que medirlo antes de tocarlo.
  - [x] **A4b -- la LAMINA en el lenguaje** (el propietario, 14-10: *"si
        dale con A4b asi"*). `lamina` como TIPO, sin palabra nueva (como
        `byte`): el nivel 15 de `toolchain/lang/titan/GRAMATICA.md`.
        `let mut l = lamina(4096)` pide por la puerta de la RAM; `l[i] =
        byte(x)`, `l[i]` y `len(l)` como una `[byte]` de medida fija y fuera
        del monton; se presta (`mut`) o se entrega (`take`: su asa se MUEVE y
        quien la entrego la olvida); se SUELTA sola al morir su propietario
        (`MEM_OP_SOLTAR`) por cualquier camino. NO se copia (T0077: `let b =
        a`, `print`, una tabla o un registro, un parametro sin modo, una fn
        que la devuelve), no crece (T0077 en `push`), y una `gpu fn` no la
        pide. **HECHO el 14-10:** el frontend (`src/lamina.rs`, el tipo, la
        clase y `Value::Lamina`), el emisor (`e1/lamina.rs`, y la vuelta
        `Helper::SueltaLamina` en `e1/ram.rs`) y el emulador, que ahora
        modela `MEM_OP_SOLTAR` como el kernel. **Como se sabe:**
        `emisor-x86_64/tests/la_lamina.rs` -- prestada, entregada y soltada
        UNA vez cada una (saboteado: quien la entrega tambien la soltaria, 4
        llamadas en vez de 2, cae; sin soltar, cae); veinte laminas en un
        bucle con un tope de ocho a la vez; el `break`, el `continue` y la
        vuelta temprana; T0072 y T0060 al correr; los catorce NO --; y los
        ejemplos del nivel 15 en los dos bancos y en la TAB del TALLER.
        **Lo que queda fuera, dicho:** `director.ofrece(l)` -- prestarla a
        otro proceso -- espera a que haya quien la tome (dos apps que
        hablen); hasta entonces una lamina es del programa que la pidio, y
        tampoco se devuelve de una fn. **Falta el metal:** un `.bex` del
        nivel 15 corriendo en el Ryzen.
  - [x] **en el Ryzen** -- FUNCIONO el 15-10 (hoja 6b, fila A4). Era: un `.bex` del nivel 15 de TITAN++ corriendo (hoja 6b, fila A4). Cuando se vea bien: se anota en la hoja
        y se tachan esta casilla y la de A4 (A4a y A4b ya lo estan).

- [x] **A5 -- LB3a: la LENTE lee x86** (`PLAN_LA_LENTE_BINARIA.md`, LB3): un
      decodificador del SUBCONJUNTO que emiten INTI y TITAN++ (prefijos,
      opcode, ModRM, SIB, desplazamiento), crate puro. Lo que no esta en la
      tabla se dice "fuera de la tabla" con sus bytes, nunca se adivina.
      **Como se sabe:** ida y vuelta -- cada codificacion que escribe el
      emisor, la lente la lee igual --, sobre TODOS los ejemplos de los dos.
      **HECHO el 14-10:** `platform/shared/bmo-lente-x86` (`no_std`, sin
      dependencias, compila para el objetivo del kernel): prefijos `66 F2 F3
      F0`, REX, VEX de dos y tres bytes, los mapas `0F`, `0F 38` y `0F 3A`,
      ModRM, SIB, `rip`+disp e inmediatos; la tabla (`tabla.rs`, del SDM y
      del APM, recortada a lo que se emite); el texto en Intel; y lo que
      puede venir despues de cada una (`Sigue`). **El oraculo fue mas alla
      de INTI y TITAN++:** el `metro` del emisor corre sus 63 programas --
      C, C++, COBOL, Ada, INTI y TITAN++ -- con una sonda que apunta cada
      paso, y la lente lee cada instruccion que corrio y su siguiente tiene
      que ser uno de sus sucesores (eso juzga el LARGO, y el DESPLAZAMIENTO de
      cada salto). Las 35.208 instrucciones distintas que corrieron, leidas;
      saboteada (sin la fila de `mov r, r/m`: 579 quejas; un inmediato de 3
      bytes: 472), cae, y el `metro` es guardian del build. Y la ida y
      vuelta con cada codificador de `bmo_lower::x86`, con los 16 registros.
      **Lo que NO cubre, dicho:** el codigo que no corre en el banco (lo
      leera B1 siguiendo los saltos desde la entrada).
- [x] **A6 -- LB6: el `.bex` dice que instrucciones usa.** El emisor de INTI
      y el de TITAN++ apuntan la extension de cada instruccion (SSE2, AVX2,
      FMA, BMI1, POPCNT...), el enlazador hace la union, y la puerta
      (`bmo-bex-gate`, al lado de `xcr0`) la compara con el CPUID. **Como se
      sabe:** un CPUID de mentira sin AVX2 y un `.bex` que lo usa: NO, con
      su nombre.
      **HECHO el 14-10**, con dos cambios sobre lo escrito, dichos:
      - **El juicio no esta en `bmo-bex-gate` sino en `admitir`**, al lado
        del de la memoria y antes de la primera reserva: es donde el kernel
        ya lee el anexo de REQUISITOS, y la clase nueva va en ese anexo
        (`CLASE_INSTRUCCIONES = 0x09`, una mascara, obligatoria) y no en la
        cabecera -- el formato no cambia y un `.bex` viejo sigue entrando.
      - **SSE2 no se declara:** es x86-64 base. El catalogo
        (`bmo-carga-juicio/src/instrucciones.rs`) tiene 16 extensiones con su
        hoja, registro y bit de CPUID: SSE3, SSSE3, SSE4.1, SSE4.2, POPCNT,
        LZCNT, BMI1, BMI2, AVX, AVX2, FMA, RDRAND, RDSEED, RDTSCP, OSXSAVE,
        MONITOR. El estado (`xcr0`) sigue siendo otra pregunta.
      Quien lo escribe es la TABLA: `intrinsics.toml` lleva una columna `cpu`
      por fila, `sem-asm` la lee (un nombre desconocido tira la carga), y el
      emisor de C (y con el C++) y el de INTI la apuntan al escribir los
      bytes; `Escritor::instrucciones` la junta en una sola linea, el `.bo` la
      devuelve (`Object::instrucciones`) y `bmo-enlazar` hace la union. TITAN++
      no escribe hoy ninguna instruccion fuera de x86-64 base. El rechazo dice
      `falta en esta CPU: AVX2` en la bitacora y en `run`. **Tres jueces:** el
      CPUID de mentira sin AVX2 (`bmo-carga-juicio`); la LENTE lee los bytes
      de cada fila de `intrinsics.toml` y lo que piden tiene que ser
      EXACTAMENTE su columna `cpu` (cada fila de la lente sabe su extension:
      `pide`); y el `metro`, con `bits_C.c` en su banco: cada instruccion que
      corre pide lo que pide y el `.bex` lo tiene que declarar. Saboteados (la
      columna de `popcnt` borrada, la de `rdrand` cambiada, el emisor de C sin
      apuntarla), caen los tres con el nombre.

**Escalon 2 -- anfitrion y UNA corrida del metal (medio)**

- [ ] **B1 -- LB7 + LB8: verificar una vez y apartarse.** Al admitir un
      `.bex`, la lente (A5) decodifica su codigo desde la entrada y comprueba
      que no miente la cabecera (A6); lo demostrado se apunta por su hash y
      no se vuelve a leer. **Como se sabe:** en el anfitrion, un `.bex` con
      una instruccion no declarada: NO; en el metal, el segundo `run` del
      mismo `.bex` no paga la lente.
      **HECHO EN EL ANFITRION el 14-10; FALTA LA MITAD DEL METAL** (por eso
      la casilla sigue abierta). `bmo_lente_x86::recorre` lee el codigo como
      lo correra la CPU: desde la ENTRADA, por saltos y llamadas (un `call`
      indirecto vuelve; un `jmp` indirecto, no), y desde cada RELOCACION que
      apunta al codigo -- los punteros a funcion que el programa guarda, que
      escribio el enlazador. No necesita memoria propia: quien llama da el
      mapa de lo leido y la pila. En el kernel, `task/lente.rs` lo corre en
      `admitir` sobre las paginas ya mapeadas y con el hash ya comprobado:
      una extension NO declarada es un NO (con su sitio, y `run` lo pinta);
      lo que no sabe leer o un salto que sale del codigo se dice "NO
      DEMOSTRADO" y ENTRA -- un limite de la lente no es una falta del
      programa. Lo leido se recuerda por el hash de la region de codigo (32
      `.bex`), y la bitacora dice `lente: ya leido` cuando se ahorra.
      **Como se sabe:** el `metro` recorre sus 64 programas en cada build y
      todos quedan DEMOSTRADOS (cobertura 98-100 %; `cuentas.cpp` tiene una
      vtable) y todo lo que corrio lo alcanzo el recorrido; DOOM, compilado
      del commit fijado, DEMOSTRADO en 0,1 s con el 84 % del codigo
      alcanzado (lo demas: unas 57 funciones a las que nada llama, y punteros
      tomados con `lea`); y `bmo-metro --recorre x.bex` lo dice de cualquier
      `.bex`. Saboteado (el emisor de C sin declarar `popcnt`): NO, con
      BMI1 en `0xa4`. **Lo que NO se siembra:** un `lea [rip+x]` que cae en
      el codigo -- TITAN++ guarda DATOS ahi, y probarlo leyo `cubo_gira`
      como instrucciones. **Lo que falta:** lanzar el mismo `.bex` dos veces
      en el Ryzen y ver `lente: ya leido` la segunda.
      **CORREGIDO el 14-10, con B3 (antes de llegar al metal):** pasando el
      recorrido por los `.bex` de Rust (el DIRECTOR, ESTUDIO, PROTON-X...),
      el DIRECTOR salia con un NO -- `tzcnt rax, rsi` sin BMI1 declarado --
      y B1 lo habria dejado fuera al arrancar. Era falso: `tzcnt` es `rep
      bsf`, una CPU sin BMI1 ignora el `F3` y ejecuta `bsf`, y por eso LLVM
      lo emite en x86-64 base. Ahora el NO es solo para lo que sin su
      extension da `#UD` (`Fila::sin_ella_falla`); `tzcnt`/`lzcnt` se
      cuentan aparte (`degradan`). Los diez `.bex` de Rust y DOOM: 0 NO.
      [!] En los de Rust la lente se para pronto (LLVM usa `nopw cs:` y
      `test al, imm8`, que su tabla no tiene) y lee del 3 al 70 % de su
      codigo: si la tabla crece, que vuelvan a pasar por `--recorre` ANTES
      de llegar al Ryzen.
  - [x] **en el anfitrion** -- HECHO el 14-10, corregido el 14-10 con B3 (lo de arriba).
  - [ ] **en el Ryzen** -- `run` dos veces del mismo `.bex`: la segunda, `lente: ya leido` (hoja 6b, fila B1). Cuando se vea bien: se anota en la hoja
        y se tachan esta casilla y la de B1.

- [x] **B2 -- LB3b: la autopsia de Ring 3 dice la instruccion.** El capitulo
      7 de `save`: los bytes en RIP decodificados y explicados, de que
      programa y que funcion. **Como se sabe:** un programa que falla a
      proposito, y el informe dice su instruccion y su registro malo.
      **HECHO EN EL ANFITRION el 14-10; FALTA VERLO EN EL RYZEN.** La
      autopsia (`core/autopsy.rs`) tiene un renglon mas, `instr`, justo
      detras del `rip`: la instruccion leida por la lente y, si tocaba
      memoria, cada registro de su direccion con lo que valia al fallar --
      `instr     movsxd rax, dword [rax]  rax=0xdead0190`. Si el `rip` no se
      lee (un salto a nulo), lee el `call` de antes del retorno
      (`bmo_lente_x86::el_call_antes`), que dice de que tabla salio el nulo.
      Lo que la lente no tiene en su tabla se dice, no se adivina. La lente
      gano `Instr::memoria()` (base, indice, escala, desplazamiento, `rip`).
      **Como se sabe:** el emulador gano una TRAMPA (un rango que, si el
      programa lo toca, para la maquina como un `#PF`, con el `rip` y los
      registros de antes); `tests/autopsia.rs` del banco de C compila un
      programa que lee `p[100]` de `0xDEAD0000`, y la lente, con esos
      registros, da EXACTAMENTE la direccion tocada (`0xDEAD0190`) y el
      registro que la llevaba. Mas las pruebas de la lente: el caso de
      Cyberpunk (`mov eax, [rdi+0x18c0]` -> `rdi`), SIB, `r13`, `rsp`,
      `rip`, sin base. "De que funcion" sigue siendo el `+desplazamiento` del
      renglon `rip` con `--map`, como antes. **Lo que falta:** un `.bex` que
      falle a proposito en el Ryzen y leer `instr` en su `datos/fallos.txt`:
      es `c/autopsia.bex` (`toolchain/lang/c/examples/autopsia_C.c`, 15-10).
  - [x] **en el anfitrion** -- HECHO el 14-10 (lo de arriba).
  - [x] **en el Ryzen** -- FUNCIONO el 15-10 (hoja 6b, fila B2). Era: un `.bex` que falle a proposito y su renglon `instr` en `datos/fallos.txt` (hoja 6b, fila B2). Cuando se vea bien: se anota en la hoja
        y se tachan esta casilla y la de B2.

- [ ] **B3 -- Q0a1..Q0a4: el cubo de TITAN++ en SU ventana, por la 3060**
      (seccion 5): VERRANO dibuja la lamina dentro de la ventana, y el
      DIRECTOR no nombra la 3060 (1c). **Como se sabe:** lo de la seccion 5.
      **14-10, revisado: TODO lo del anfitrion esta hecho y sigue verde; solo
      falta el metal.** Q0a1..Q0a4 se escribieron el 10-10 y el 12-10 (ver la
      seccion 5); hoy, despues de A4..B2: 665 pruebas en verde de TITAN++,
      ga10x, PROTON-X sm86 y VERRANO (la ventana y la lamina de `cubo_gira`,
      los 360 fotogramas, el juez de la CPU a otra medida, `pruebas_medida`);
      el DIRECTOR compila; el guardian `la-3060` (S3) limpio -- el DIRECTOR
      no nombra la 3060 --; `titan/cubogira.bex` sale de `ejemplos.ps1`; y
      las puertas nuevas lo dejan entrar: `cubo_gira` declara lo que usa (A6)
      y la lente lo DEMUESTRA entero (B1, en el `metro`). Un parrafo de Q0a2
      decia que a VRN1 le faltaba el color de fondo: ya lo lleva, corregido.
      **Lo que falta, en el Ryzen** (la seccion 6, filas Q0a2..Q0a4): `run
      titan/cubogira.bex` desde Ejecutar -- gira EN SU VENTANA, se mueve, se
      tapa --, la consola dice `[Q0a3] el juez ... 640x360: la 3060 da lo
      MISMO`, y con el cubo girando `gpu verrano banco inti enram` IGUAL.
  - [x] **en el anfitrion** -- HECHO el 10-10 y el 12-10, revisado el 14-10 (lo de arriba).
  - [ ] **en el Ryzen** -- el cubo de TITAN++ en SU ventana por la 3060 (hoja 6b, fila B3; y las filas Q0a2..Q0a4 de la 6). Cuando se vea bien: se anota en la hoja
        y se tachan esta casilla y la de B3.

- [ ] **B4 -- LB10 + LB12: el perfil del Ryzen, UNA fuente, comprobado al
      arrancar.** Lo leen el emisor, la puerta y la lente; si la CPU no es la
      del perfil, lo generico de x86-64 y se dice.
      **HECHO EN EL ANFITRION el 14/15-10, con lo que pidio el propietario
      al darlo:** *"que ese LLVM de rust TENGA OBLIGACION que tengas
      control ... yo decido por motivos"*. **Falta solo ver en el Ryzen la
      linea del arranque** (`cpu: es la CPU del perfil` y la cuenta de las
      decididas que tiene).
      - **La decision, en el perfil.** La tabla de extensiones de
        `PERFIL/CPU.txt` gana dos columnas que escribe el propietario:
        `mano` (lo que la casa puede usar: `intrinsics.toml`, `asm!`,
        `global_asm!`) y `llvm` (lo que el compilador elige solo). Del dia:
        lo que BMO-X ya usaba; LLVM, nada fuera de x86-64 base.
      - **UNA fuente.** El catalogo de A6 lleva los nombres del perfil
        (`CR4.OSXSAVE`, `MONITOR/MWAIT`), el censo del kernel gana SSE3 y
        SSSE3, y `perfil-campos` ata las reglas (`llvm: si` pide `mano:
        si`; `mano: si` pide `esperado: si`; todo el catalogo decidido) y
        genera `bmo-carga-juicio/src/decidido.rs` (`--apply`). Lo leen: el
        EMISOR (`sem-asm` no carga una fila con `mano: no`: saboteado AVX,
        "[avx_funde4] pide AVX ... no la permite"); la LENTE (`bex-link`,
        abajo); y el KERNEL al arrancar.
      - **LLVM obedece, en dos sitios.** Antes de compilar, el guardian
        `llvm-obedece` cierra sus tres puertas -- la CPU del objetivo
        (`x86-64`, nunca `native` ni `znver3`), los `+feature` y los
        `#[target_feature]` -- contra la columna `llvm` (saboteado `+avx2`,
        `znver3` y un `target_feature`: cae las tres). Y despues, en el
        BINARIO: `bex-link` barre TODO `.text` del ELF con la lente --
        cada instruccion, la misma que `objdump`: 1.062.403 en el DIRECTOR,
        578.457 en PROTON-X -- y declara en el `.bex` lo que pide, o PARA
        el build si algo no esta decidido o no se sabe leer (saboteado
        RDRAND: para en `process_prng+0x91`). Para leerlo todo la lente
        aprendio lo que LLVM escribe: `cs:`/`fs:`/`gs:`, `test al, imm`,
        `shld`/`shrd`, `roundsd`, `mov sreg`, `fxsave64` y el x87 del crt de
        PROTON-X.
      - **Lo que salio a la luz:** PROTON-X usa SSE4.1 (`roundsd`, asm de
        la casa), RDRAND y CR4.OSXSAVE; ahora su `.bex` lo DECLARA. Y
        `usage.rs` decia que SSE4.1 no lo pedia nadie: corregido.
      - **LB12, el arranque:** `profile::carear_extensiones()`, al lado del
        careo de la topologia: si la CPU no es la del perfil (familia/modelo)
        lo grita y queda lo generico de x86-64 -- un `.bex` que pida lo que
        no hay no entra (A6) --; y cada extension DECIDIDA que el silicio no
        tiene, con su nombre.
      - **Lo que NO esta:** la otra mitad de LB10 -- "cual cuesta menos en
        el Zen 3" (latencias) para que el emisor elija -- no existe todavia;
        hoy el perfil decide QUE se puede, no CUAL conviene.
  - [x] **en el anfitrion** -- HECHO el 14/15-10 (lo de arriba).
  - [ ] **en el Ryzen** -- la linea del arranque `cpu: es la CPU del perfil` (hoja 6b, fila B4). Cuando se vea bien: se anota en la hoja
        y se tachan esta casilla y la de B4.

**Escalon 3 -- Ring 0 y el metal (dificil)**

- [ ] **C1 -- S1: la lamina PRESTADA a la 3060** (`PLAN_VERRANO.md`): la
      IOMMU, solo lectura, con la palabra de VUELTA (la ranura en vuelo) en
      el trato de A2.
      **HECHO EN EL ANFITRION el 15-10**, en tres trozos:
      - **La palabra de VUELTA** (`bmo-lamina/src/trato.rs`): el lector
        RESERVA la ranura publicada (`reservar`/`soltar`) y el escritor que
        la respeta (`empezar_si_libre`) no la pisa: si esta en vuelo, no
        publica ese fotograma (Dekker: sello impar, valla, mira la VUELTA).
        La prueba castigada: 20000 fotogramas, la ranura reservada NUNCA
        cambia; y saboteada (el escritor que no mira) MUERDE.
      - **Los escritores la respetan:** la `Lamina` de VERRANO
        (`publica` -> `Ocupada`, `respeta_vuelta`) y el emisor E1 de TITAN++
        (`mfence` + la VUELTA antes de escribir; prueba
        `titan_respects_the_slot_in_flight`).
      - **La 3060 la lee PRESTADA:** VRN1 con el bit 10 del estado lleva 16 B
        de descriptor en vez de los vertices (`ga10x/trabajos/prestada.rs`).
        En Ring 0 (`gpu_trabajo/cubo.rs`, `en_frio`) el kernel comprueba que
        la VA es de un prestamo TOMADO por el escritorio (`fisica_tomada`;
        si no, `LaminaNoTomada`, 29), la mapea en la IOMMU de la 3060 SOLO
        LECTURA en la ultima ranura de la ventana de texturas, dibuja,
        espera la valla y la desmapea -- todo bajo el cerrojo de GR: si la
        app muere no queda nada mapeado --. Con el anillo, NO
        (`LaminaNoSePresta`, 30). El escritorio (`gspcubo/laminas.rs`) la
        usa si la app respeta la VUELTA; si el kernel la rechaza lo dice UNA
        vez y vuelve a copiar.
      - **Lo que NO esta:** S1 decia "desmapear en `reap_dead`"; no hace
        falta: el prestamo dura un dibujo, no la vida de la app.
  - [x] **en el anfitrion** -- HECHO el 15-10 (lo de arriba; banco 5349 verdes).
  - [ ] **en el Ryzen** -- el cubo de TITAN++ en su ventana con la lamina PRESTADA (hoja 6b, fila C1). Cuando se vea bien: se anota en la hoja
        y se tachan esta casilla y la de C1.
- [ ] **C2 -- S3 + S4: el ritmo por el sello y la app en su nucleo.** El
      92 % de la pared de S0 era esperar: aqui se gana.
      **S3 HECHO EN EL ANFITRION el 15-10:**
      - **El futex COMPARTIDO** (`bmo_futex::llave_compartida`, ops 0x46 y
        0x47): la llave es la FISICA de la palabra. El de antes llevaba el
        proceso en la llave, y dos procesos que comparten una lamina la ven
        en direcciones distintas: no se encontraban nunca.
      - **La lamina con TIMBRE:** palabra 13 `DUERME` (el escritorio), 14-15
        el TSC de la publicacion (la app), bit 3 de la 11 `ESTADO_TIMBRE`. La
        app mira DUERME despues de publicar y solo llama al kernel si alguien
        duerme: con el escritorio despierto no cuesta ni una llamada. La
        prueba (un kernel de mentira que mira bajo su cerrojo, como el de
        verdad): 3000 fotogramas, ningun despertar perdido; sin timbre, el
        escritorio se queda dormido (muerde).
      - **TITAN++ lo toca** (E1: `mfence`, DUERME, y el despertar en la
        palabra de la secuencia); prueba con sabotaje.
      - **`gpu verrano banco inti` duerme en el sello** si la app lo toca, y
        dice `C2, el sello: ... de publicado a dibujado X us de media, Y us
        la peor`. Una app sin timbre (INTI) sigue con el `wait` de 1 ms.
      - **Lo que NO esta de S3:** "el anillo con 1 o 2 en vuelo" -- se mide
        primero la latencia en el Ryzen y despues se decide --.
  - [x] **en el anfitrion (S3)** -- HECHO el 15-10 (lo de arriba; banco 5353 verdes).
  - [ ] **en el Ryzen (S3)** -- `run titan/cubogira.bex` y `gpu verrano banco inti` (hoja 6b, fila C2). Cuando se vea bien: se anota en la hoja
        y se tachan esta casilla y la de S3.
  - [ ] **S4 -- la app en SU nucleo: BLOQUEADO, y por que** (15-10). Hoy el
        planificador corre Ring 3 SOLO en el BSP (`percpu.rs`: "Ring 3 solo
        corre en el BSP"; una cola, un `SCHED_LOCK`): los AP solo hacen
        fork-join (`crew`, `repartir`: Ring 3 sin syscalls y con plazo de
        5 s). Una app ENTERA en otro nucleo es H1.1 de
        `PLAN_LAS_TRES_GRANDES.md` (colas por nucleo, un LAPIC por nucleo,
        el IPI de replanificar), y su propio orden dice que la fila 7 no
        empieza antes que la 5 (los Sync de PROTON-X: `hilos-casa`, hoy 63
        contra 62). Y `smp all` reinicia el PC (V9/V10 de
        `PLAN_LOS_DOCE_DIRECTORES.md`). **Lo que pide del propietario:**
        decidir si S4 espera a H1.1 o si se hace un paso menor antes (la
        tanda de la app por `repartir` en los obreros, sin mover la app).
- [ ] **C3 -- Q0b: la 3060 COMPONE las ventanas** (la regla 1b): fuera los
      27,6 ms de la CPU copiando la pantalla.
      **Lo que dijo el codigo al mirarlo (15-10):** cada fotograma tiene DOS
      copias. (1) Lienzo -> pantalla (RAM -> VRAM por el PCIe, memoria
      WC): ESA es la de los 27,6 ms (calculados: 8,3 MB a ~300 MB/s), y ya
      tenia camino por la 3060 -- el motor de copia, pasos 1b y 1c de
      PLAN_LA_3060, VISTO en el metal el 25-09: 649 us la entera --, pero
      solo lo armaba `save mode` o `gpu volcado`. (2) Superficie de cada app
      -> lienzo (`pegar_fila`, RAM -> RAM, solo lo que cambio): barata en la
      CPU; ninguna orden de hoy copia un rectangulo a OTRA posicion.
      **C3a HECHO EN EL ANFITRION el 15-10 -- el volcado por la 3060, POR
      DEFECTO:** `bmo::PorDefecto` (pura, 4 pruebas) decide: con la 3060
      lista (GSP_INIT_DONE) y lienzo, la primera vez una copia VERIFICADA
      (1024 muestras) y despues armar; al volver la pantalla de un juego,
      armar otra vez; las caidas solas (`Pantalla::caidas_gpu`) y los
      intentos fallidos cuentan juntos, y a las tres la CPU (L32), dicho UNA
      vez. `gpu volcado off` manda; `gpu apagar` y el reinicio lo sueltan y
      no se rearma sobre un GSP apagado.
  - [x] **en el anfitrion (C3a)** -- HECHO el 15-10 (lo de arriba).
  - [ ] **en el Ryzen (C3a)** -- tras `gpu init`, la consola dice `[C3] la 3060 vuelca el escritorio por defecto` sin teclear `gpu volcado` (hoja 6b, fila C3). Cuando se vea bien: se anota en la hoja
        y se tachan esta casilla y la de C3a.
  - [ ] **C3b -- las VENTANAS pegadas por la 3060: DECIDIR.** Pide una orden
        nueva del motor de copia (origen y destino con su propio desplazamiento
        y su propio paso; hoy `Tanda::caja` usa el mismo en los dos) y prestar
        la superficie de cada app por la IOMMU. Dos caminos, y ninguno es
        gratis: (a) superficie -> LIENZO por la 3060: el lienzo sigue siendo
        la verdad (el cursor, el cromo), pero es RAM -> RAM cruzando el PCIe
        dos veces -- mas lento que la CPU --; (b) superficie -> PANTALLA
        directo: una sola copia, pero el lienzo bajo la ventana queda viejo y
        el cursor y las cajas sucias de encima tienen que ir DETRAS en la misma
        tanda. **Lo que se propone:** medir antes en el Ryzen con C3a armado
        (la fila `gpu volcado` y el tablero de E2) cuanto cuesta de verdad el
        `pegar_fila`; si es menos de 1 ms (lo que dijo PLAN_EL_PIXEL el 09-09),
        C3b no gana nada que se vea, y la regla 1b se cumple con (b) solo para
        las apps que dibuja VERRANO.
- [x] **C4 -- LB4 + LB5: los contadores del Zen 3, y INTI exprime con
      evidencia.** Primero MEDIR que cuenta cada contador en este Ryzen.
      **HECHO EN EL ANFITRION el 15-10:**
      - **LB4, calibrados:** `bmo-contadores` (puro, 8 pruebas): los MSR,
        `PERF_CTL`, el catalogo con su FUENTE (el PPR de la familia 19h y el
        `perf` de Linux) y su CONFIANZA -- ciclos, instrucciones, saltos y
        saltos fallados, firmes; accesos y fallos de la L1D, por confirmar --,
        y el juez. El kernel corre tres bucles que se saben de memoria
        (recta, azar, persecucion por 256 KiB en permutacion) con las
        interrupciones apagadas, UNA vez por arranque, y solo en la CPU del
        perfil. `cpu contadores` pinta cada fila: lo contado, lo esperado y
        si CUENTA lo que dice el papel.
      - **LB5, un programa se cuenta:** `TASK_OP_CONTAR`; el cambio de tarea
        enciende los contadores 0..3 solo para quien empezo, solo en Ring 3.
        TITAN++: `director.contar()` y `director.contado(k)`; INTI:
        `invoca(mi_tarea, op_contar, contar_empieza, 0, 0)` y
        `invoca_valor(mi_tarea, op_contar, contar_lee, k, 0)`, sin tocar su
        compilador. En los dos, lo contado es lineal en las vueltas de un
        bucle (el emulador cuenta instrucciones con su metro).
      - **Lo que NO esta:** que la LENTE diga en castellano "este bucle
        falla la L1 una de cada 3" sobre un programa entero (pide los
        numeros del metal primero); y que el emisor ELIJA con ellos (LB10,
        la otra mitad).
  - [x] **en el anfitrion** -- HECHO el 15-10 (lo de arriba; banco 5364 verdes).
  - [x] **en el Ryzen** -- FUNCIONO el 15-10 (hoja 6b, fila C4). Era: `cpu contadores`: 8 de 8 filas CUENTAN (hoja 6b, fila C4). Cuando se vea bien: se anota en la hoja
        y se tachan esta casilla y la de C4.

**Escalon 4 -- lo mas duro**

- [ ] **D1 -- V3 + V4 de VERRANO**: las constantes en un buffer, y que
      `cubo.bsf` deje el SASS escrito a mano: lo emite el emisor de la casa,
      cuya entrada es el Programa de BMO-X. **No SPIR-V al estilo de Linux**
      (el propietario, 11-10 y 14-10): VERRANO OCUPA el sitio de Vulkan y de
      D3D12, no va debajo. Lo de la casa habla VERRANO directo; Vulkan y
      DX12 son EXTRANJEROS que se traducen a VERRANO (V5, PROTON-X), y
      SPIR-V o DXIL solo son lo que trae un juego ajeno.
      **HECHO EN EL ANFITRION el 15-10:**
      - **V4, `cubo.bsf` sin SASS a mano:** el sobre que toma el escritorio
        es desde hoy el del cubo de TITAN++ (`ejemplos/nivel11/cubo`):
        PROMETEO lo emite desde el Programa de la casa, el pegamento de
        VERRANO lo pega a la tuberia, el juez dice PERFECTO con los registros
        de la tuberia. Su fuente es el MAPA de la casa, no SPIR-V. Lo fabrica
        y lo ata `tests/cubo.rs` de TITAN++ (`BSF_FIJAR=1`); el banco de
        ga10x lo mira como la puerta. Los programas a mano se quedan (la
        prueba sin LDG y el corpus del juez), juzgados.
      - **V3, las constantes:** una gpu fn que dibuja recibe un SEGUNDO
        valor, su cbuffer (cada campo una fila de 16 B, hasta 16). PROMETEO
        lo baja a `Op::Constantes`; la 3060 lo lee de c[3] en el oraculo y
        del DATOS en la tuberia (el pegamento ya lo sabia); la CPU, por
        `rdx`. `ejemplos/nivel11/cubo3`: los vertices SIN transformar y la
        `wvp` dan, en las dos tarjetas, la casa y el calculo, la posicion de
        recorte de la tanda BIT A BIT -- la de la huella de D3D12 --; cambiar
        el orden de la suma la rompe.
      - **Lo que NO esta:** V3b, que el ESCRITORIO dibuje el cubo de V3 por
        la 3060 (el contrato de `gspcubo/sm86.rs` con cbuffer y los DATOS
        con sus filas delante); `LOAD_CONSTANT_BUFFER` (0x238c) dentro de las
        ordenes, que era la otra forma de M2; y el FMA (la pregunta de V3):
        el emisor no funde, y asi da los bits de D3D.
  - [x] **en el anfitrion** -- HECHO el 15-10 (lo de arriba; banco 5368 verdes).
  - [ ] **en el Ryzen** -- `gpu verrano` con el `cubo.bsf` de TITAN++ (hoja 6b, fila D1). Cuando se vea bien: se anota en la hoja
        y se tachan esta casilla y la de D1.
- [ ] **D2 -- Cyberpunk, retomado** con todo lo de arriba debajo.

---

# 1. EL MAPA: que hace cada uno

```text
   TITAN++    EL QUE ADMINISTRA. Las apps se escriben en TITAN++, y TITAN++
              conecta la CPU (por INTI) y la GPU (por VERRANO). Sus gpu fn
              cuentan en la 3060 o en la CPU, bit a bit lo mismo (LB4)
   INTI       LA PUERTA A LA CPU, y nada mas. Se queda lo de CPU: cpu.inti,
              pulso.inti y el emisor. Pierde TODO lo de app, pieza a pieza,
              el dia que su relevo en TITAN++ existe (3.3 de
              PLAN_INTI_SAMURAI); desde el 10-10 tambien bico.inti y
              png.inti (seccion 1d), y musica.inti con el sonido (Q2)
   VERRANO    LA PUERTA A LA GPU: la API de dibujo. Detras, la RTX 3060 12G
              (o la CPU, la misma API)
   LAMINA     LA RAM COMPARTIDA: quien cuenta PUBLICA en un bloque de RAM, y
              quien dibuja lo LEE ahi, sin copias ni syscalls por byte (la de
              VERRANO para vertices; la de la antena para paginas)
   PROTON-X   LOS JUEGOS DE WINDOWS: traduce D3D12 y DXIL a la casa, y lo que
              la puerta deja, a la 3060 (la receta)
```

## 1b. LA REGLA DEL PROPIETARIO (10-10): las apps VIVEN en la GPU

> *"TODAS las APP tienen que vivir en GPU no solo en CPU, en CPU solo
> calcula [...] que TITAN++ tome TODOS pero que algunos se quedan con CPU y
> otros que ejecutan sean SIEMPRE en GPU."*

```text
   TITAN++ administra TODAS las apps (INTI pierde lo de app)
   la CPU     CALCULA: la logica, las cuentas que no son de celdas, lo que
              lee de fuera. Nada de pintar pixeles a mano para la pantalla
   la 3060    EJECUTA lo que se ve: componer cada ventana en la pantalla,
              dibujar lo de VERRANO (el cubo, en su ventana), y las gpu fn
              de celdas. SIEMPRE, salvo que no este despierta (G0): entonces
              la CPU, la RESERVA de la ley L32, con los mismos bits, y lo dice
```

Lo que pide, en piezas (van en la cola, seccion 4):
la VENTANA de cada app compuesta por la 3060 (hoy el DIRECTOR copia con la
CPU); el cubo de TITAN++ dibujado por la 3060 DENTRO de su ventana (hoy, a
pantalla completa con `gpu verrano banco inti`); y las gpu fn al correr en la
3060 (LB8, la puerta de computo, Ring 0). F1..F5 no se paran por esto: la
ventana de TITAN++ es la misma superficie que la 3060 compondra.

## 1c. LA SEGUNDA REGLA (10-10): el DIRECTOR no negocia con la GPU

> *"sigue que mas falta para que el director negocie con GPU pero ojo no
> mismo GPU sino TITAN++ con VERRANO eso"*

```text
   la app (TITAN++)   CALCULA y PUBLICA en su lamina (RAM compartida):
                      `director.lamina` y `director.publica`; abre su ventana
                      (`director.ventana`) y dice su medida
   el DIRECTOR        COMPONE: toma la lamina de cada app y se la da a
                      VERRANO -- un `Frame` a una `Image` (`Backend::draw`),
                      la Image = los pixeles de la ventana de esa app --.
                      No nombra la 3060, ni un registro, ni una orden: S3 del
                      guardian `la-3060` ya lo juzga en cada build
   VERRANO            LA API: decide quien dibuja -- la 3060 si esta
                      despierta (G0), si no la CPU con los mismos bits (L32)
   el driver          ga10x, DETRAS de VERRANO (y de `gspcubo/sm86.rs`, la
                      unica puerta del escritorio que lo toma)
```

Asi Q0a2 y Q0a3 (seccion 5) se escriben CONTRA VERRANO, no contra el
aparato: el DIRECTOR pide "este Frame en esta Image de 640x360" y es la
puerta (`sm86.rs`) la que habla con la 3060. Si un dia hay otra GPU, el
DIRECTOR no cambia.

## 1d. INTI, MAS DELGADO (10-10): bico y png tambien salen

> *"me hice pregunta se pueden degradar mas? [...] png, bico y otros (no
> quites el CPU porque es INTI)"*

Si: D3 de PLAN_INTI_SAMURAI se reescribe. `bico.inti` (BMP y QOI a BICO) y
`png.inti` (escribe un PNG) son HERRAMIENTAS, no la puerta a la CPU: salen
de INTI con su relevo en TITAN++, y no antes (la regla 3.1: quitar antes
solo pierde su prueba). Lo que pide cada relevo:

```text
   pieza            su relevo en TITAN++                       estado
   bico.inti        Ultra_userspace/apps/bico (`run            FUERA, 10-10 (corte
                    titan/bico.bex`): `director.fichero` lee,  4e): los mismos
                    y `crea`/`escribe`/`cierra` escriben       bytes, los mismos
                                                               ficheros rotos
   png.inti         Ultra_userspace/apps/png (`run             FUERA, 10-10 (4e):
                    titan/png.bex`): el CRC y el Adler sin     el mismo fichero,
                    operaciones de bits                        byte a byte
   musica.inti      Ultra_userspace/apps/musica (`run apps/    FUERA del build el
                    musica.bex`): `use sonido`, las VOCES del   15-10 (Q2, lo pidio
                    orquestador -- la onda en una lamina, y el  el propietario);
                    kernel la mezcla --; las mismas notas, los  su fuente y su
                    mismos accidentes y las mismas rotas que el banco se quedan
                    de INTI, y sus muestras una a una           hasta el Ryzen
   cpu.inti,        SE QUEDAN: son la puerta de TITAN++ a la CPU (y con
   pulso.inti       ellas el monton, mientras `cpu.inti` lo use)
```

**Falta el metal:** `run titan/bico.bex` (deja `datos/fotob.bic` y
`datos/fotoq.bic`, que el escritorio pinta) y `run titan/png.bex` (deja
`datos/hola.png`, que el visor abre). [!] Un cambio con INTI: el codigo 2
de bico (mas de 256 KiB) es ahora el 1 -- `director.fichero` dice si lo
leyo entero, no por que no --.

## 1e. LA LAMINA QUIROFANO (10-10): un dato, un sitio, en tiempo real

> *"educar a lamina (ram) que sea ultra educado [...] la CPU siempre
> orquesta [...] que sea en streaming y no copien para otro para que sea en
> tiempo real siempre [...] que lamina o ram sea quirofano extremo, que la
> CPU al tener la GPU ahi siempre disponible sea exprimido"*

Las reglas -- cada una con su juez, no con su buena intencion --:

```text
   1  UN DATO, UN SITIO   lo que la app cuenta se escribe UNA vez, en su
                          lamina; quien lo usa lo LEE ahi (el kernel lee el
                          paquete en su sitio, por el physmap). Copiar es la
                          excepcion, y cada copia que queda esta en la tabla
   2  SE MIRA UNA PALABRA la secuencia: sin publicacion nueva, NO se copia
                          nada (`Lamina::leer_si_nueva`, su prueba)
   3  NUNCA MEDIO         el sello por ranura: lo que se dibuja es siempre un
                          fotograma ENTERO; uno pillado a medias se reintenta
   4  SIN CERROJO         una app colgada deja su ultimo fotograma, y ya: nadie
                          espera a nadie (el escritorio no se para por una app)
   5  LA CPU ORQUESTA     decide CUANDO y QUE (el plazo del fotograma, el
                          paquete); la 3060 EJECUTA lo que se ve (limpiar,
                          dibujar, agrandar). Lo que la 3060 puede hacer, no
                          lo hace la CPU: la limpieza del destino ya es suya
   6  AL RITMO, NO A LA   el programa duerme hasta el PLAZO del fotograma
      SIESTA              siguiente (`director.espera`), no "trabajo + 16 ms"
```

**Las copias por fotograma del cubo (lamina -> 3060), antes y ahora:**

```text
   app -> lamina           1 (n x 32 B)   la publicacion: es el dato
   lamina -> vi -> de_inti 2 por CADA mirada de 1 ms   ->  1 por publicacion
   de_inti -> paquete      1 (n x 32 B)   queda; el siguiente corte: la
                                          lamina directo al paquete
   programas -> paquete    ~700 B por fotograma: queda (se pueden escribir
                           una vez: la caja del paquete no cambia)
   paquete -> VRAM         n x 32 B por BAR0 de 4 en 4: queda (el anillo y
                           el prestamo persistente, S1 de PLAN_VERRANO)
   limpiar el destino      enram: 3,6 MB por la CPU  ->  la 3060 (VRN1)
   leer de vuelta          el juicio final: 460.800 puertas -> la RAM (enram)
```

**EL CUELLO DE BOTELLA, con numeros** (de las medidas del metal, no de
memoria) y lo que se hizo:

```text
   1  el fotograma era trabajo + 16 ms de siesta    HECHO: el plazo
      + despertar tarde: 18,7 ms, el 92 % esperar   (`tests/ritmo.rs`)
      (S0 de PLAN_VERRANO)
   2  el kernel despertaba tarde (~3,5 ms medio):   HECHO: quien despierta
      la del mismo rango esperaba el quantum        entra si la otra ya gasto
      entero de la otra                             un tic (`bmo_orquesta::
                                                    turno`). Queda: el tic de
                                                    1 ms (un reloj sin tic)
   3  DOOM por la 3060: comprobar 560 us contra     HECHO: al cargar y uno de
      679 de dibujar (METAL_2026-09-25)             cada 32 (`im::toca_mirar`)
   4  la pantalla la copia la CPU: 27,6 ms          QUEDA: el motor de copia
      entera; el PCIe en Gen1 de Gen3               por defecto (649 us), el
                                                    page flip (LA_RAM 8) y el
                                                    enlace en Gen3
   5  las puertas largas con las interrupciones     HECHO en el anfitrion
      cerradas y la 3060 esperada girando dentro    (10-10): EL RESPIRO
      (`latido tarde 1632 ms`, METAL_2026-10-08)    (`ring0/dev/respiro.rs`,
                                                    neutro). Esperando a
      La de 1632 ms era "op 4", no la 3060 (esa     la 3060, las interrup-
      es la 53): lo mas probable, cerrar un         ciones se abren un
      fichero (FAT32 y el FLUSH con IF=0). Y el     instante en cada vuelta
      `yield_current` de la espera no cedia nada:   (sti/pause/cli) si la
      dentro de un syscall solo movia `current`     tarea TIENE la 3060 y no
      (quitado el mismo dia; la puerta larga sale   hay cerrojos tomados; la
      a nombre de quien LLAMO)                      pila del trap se re-
                                                    publica al volver. La
                                                    puerta es UNA (las
                                                    ordenes de la IOMMU y
                                                    las estaciones de la
                                                    muerte que devuelven el
                                                    lienzo); quien esta
                                                    dentro es intocable y
                                                    muere al salir. La
                                                    puerta larga mide solo
                                                    lo CERRADO. `RESPIRO`
                                                    es el interruptor.
                                                    Y op 4, el mismo dia: el
                                                    FLUSH del cierre de un
                                                    fichero va EN VUELO (el
                                                    hilo del disco lo
                                                    aterriza con su IRQ); la
                                                    escritura sigue dentro y
                                                    se MIDE (`guardar` y
                                                    `vaciar` en `save`).
                                                    QUEDA: el metal
   6  el camino frio del cubo, frecuente: la        HECHO en el anfitrion
      huella del caliente lleva el numero de        (10-10): el caliente
      vertices, que cambia al girar                 reescribe las ORDENES
                                                    (cientos de bytes, sin
                                                    releer) y la cuenta sale
                                                    de la huella. La prueba:
                                                    frio con 6 triangulos +
                                                    caliente con 4 = frio con
                                                    4, palabra a palabra
                                                    (saboteado: cae)
```

**Falta el metal** para los HECHOS: la lista entera, con sus ordenes, en
la seccion 6.

# 2. LO QUE YA FUNCIONA EN EL METAL (no se toca: se usa)

```text
   09-10 18:24  TITAN++ -> LAMINA -> VERRANO -> 3060: cubogira.bex publica y
                la 3060 dibuja 360 y 3600 fotogramas IGUAL al juez (LB7)
   09-10        G0: el GSP despierta, 2 de 10 con el paso 1
   09-10 22:41  Freedoom se juega en el Ryzen con la 3060 despierta (L0)
   28-09        BMOX-12 por PROTON-X y la receta: la 3060 lo dibuja
   06-10        los 92 jueces de DX12, 92 de 92 en Windows
```

**Sobre el "cuello de botella" (para que no confunda):** la 3060 NO es lo
lento. Dibuja el cubo en 0,87 ms; lo que se lleva el tiempo es todo lo de
alrededor -- la CPU preparando, copiando y esperando --: en S0 de
[`PLAN_VERRANO.md`](PLAN_VERRANO.md), el dibujo es el 7 % de la pared y el
resto el 92 %. DOOM no sufre porque a la 3060 le pide poquisimo: agrandar
320x200. Un juego grande si sufrira, y lo que hay que adelgazar es la CPU de
alrededor, no la 3060.

# 3. EL FOCO DE AHORA: TITAN++ TOMA NAVEGAR

**Por que este:** es el corte de INTI que el propietario pidio (NAVEGAR fuera
de INTI), usa las cuatro piezas (TITAN++ administra, la LAMINA de la antena
en la RAM, la ventana del DIRECTOR, INTI solo CPU), y deja a TITAN++ con lo
que toda app necesita: una ventana, sus pixeles y su entrada.

**Lo que hay hoy, sin adornos:** TITAN++ todavia NO puede tomar NAVEGAR. Le
faltan la ventana (TA2 de [`PLAN_LA_TINTA.md`](PLAN_LA_TINTA.md)), los bytes
(TA1) y la entrada (TA3); su `director` solo sabe la lamina de VERRANO. Por
eso `navegar.inti` (386 lineas) NO se borra antes: se borra en el MISMO
commit en que el de TITAN++ lo sustituye. Asi nunca hay un dia sin NAVEGAR.

- [x] **F1 -- la VENTANA en TITAN++** (TA2): `director.ventana(ancho, alto)`
      pide su superficie y la ofrece, una fila de pixeles se escribe, y
      `director.presenta()` la entrega; con `screen = true` en el Titan.toml
      y su linea en el certificado. **Como se sabe:** un ejemplo de nivel 11
      lanzado por un escritorio de mentira en el anfitrion pinta un
      degradado y el escritorio lee los mismos pixeles, bit a bit; y sus NO
      (sin `screen`, sin `use director`, desde una gpu fn).
      **HECHO en el anfitrion el 10-10:** `director.ventana(ancho, alto)`,
      `pixel`, `rect`, `fila` y `presenta` (`emisor-x86_64/src/e1/ventana.rs`,
      la superficie BSUP del ABI por sus nombres, con buzon de 64 ranuras);
      el ejemplo `nivel11/ventana` (un degradado y una barra que la cruza) se
      despliega como `titan/ventana.bex`; `tests/ventana.rs`: la cabecera,
      cada pixel recortado, la secuencia, las medidas que dice que no, los
      NO del compilador y el certificado (la PANTALLA). Saboteado el recorte
      de la x: cae. El metro, igual: el cubo que gira no paga nada.
- [x] **F2 -- los BYTES** (TA1): un tipo de 8 bits sin signo y tablas grandes
      en un bloque pedido. **Como se sabe:** su codigo T, su ejemplo y su
      nivel en la GRAMATICA; una tabla de millones de celdas sin copia.
      **HECHO el 10-10 por otro camino, el que NAVEGAR necesita:** los bytes
      se LEEN donde estan, sin copiarlos a una tabla -- `director.toma()` (lo
      que ofrece el antenista), `director.fichero(ruta)` (uno entero, hasta
      256 KiB), `director.medida()` y `director.byte(i)` (`entrada.rs`) --; y
      la LETRA de BMO-X en la ventana: `director.letra` y `director.texto`
      (`letra.rs`, la tabla de `fontgen` detras de su subrutina). El tipo
      de 8 bits de TA1 sigue abierto en `PLAN_LA_TINTA` (lo pide el lienzo,
      no NAVEGAR).
- [x] **F3 -- la ENTRADA** (TA3): el raton y las teclas que el DIRECTOR le
      manda a su ventana. **Como se sabe:** el escritorio de mentira manda un
      clic y una tecla, y el programa los ve. **HECHO el 10-10:**
      `director.evento()` (0 nada, 1 tecla pulsada, 2 soltada, 3 letra, 4
      raton, 5 la ventana cambio), `codigo()`, `raton_x()`, `raton_y()`,
      `botones()` y `se_ve()`; `tests/entrada.rs`: los cinco eventos del
      buzon con sus datos, y el buzon vacio.
- [x] **F4 -- NAVEGAR en TITAN++**: toma la lamina que le OFRECE el
      antenista (o la del disco, `datos/ejemplo.lam`), la pinta en su ventana
      y devuelve clics y teclas, como la version 3 de `navegar.inti`.
      **Como se sabe:** en el anfitrion, la de example.com pintada por el de
      TITAN++ y por el de INTI da los MISMOS pixeles.
      **HECHO el 10-10:** `Ultra_userspace/apps/navegar/` (un paquete:
      `src/main.titan` y `src/lamina.titan`, el port linea a linea de
      `navegar.inti` y de `runtime/lamina/lamina.inti`). Las NUEVE pruebas
      que tenia el de INTI, contra el de TITAN++ (`emisor-x86_64/tests/
      navegar.rs`), pasan a la primera; y los dos, corridos en el mismo
      emulador sobre el mensaje, example.com, la flecha abajo y una lamina
      rechazada, dan la MISMA ventana canal a canal (cuatro huellas FNV
      iguales, fijadas en la prueba).
- [x] **F5 -- el corte 4c de INTI**: `navegar.inti` fuera del build y del
      arbol, el icono y `run apps/navegar...` apuntan al de TITAN++, y el
      perfil `pleno` sale con el. **Como se sabe:** el build sin
      `navegar.ibx`; y en el metal, del propietario, NAVEGAR abre desde su
      icono y pinta example.com.
      **HECHO en el arbol el 10-10:** fuera `navegar.inti` y la sonda
      `sondas/ventana.inti` (su relevo, `titan/ventana.bex`), con sus dos
      pruebas; el build despliega `apps/navegar.bex` de TITAN++ con su icono
      (`bmo-pack` y el juez de carga, de acuerdo); el ANTENISTA ofrece la
      pagina a `navegar.bex`; el metro y la medida, al dia. El perfil
      `pleno` NO sale todavia: lo usan las sondas del censo de INTI (sale
      con el paso 5 de la escalera de INTI). **Falta el metal:** NAVEGAR
      desde su icono, con y sin antena.

Cada paso entra con sus pruebas en el anfitrion; el metal, UNA vez, al final
(F5).

# 4. LA COLA: lo que va despues, en este orden

```text
   Q0  LA REGLA 1b: (a) el cubo de TITAN++ en SU ventana, dibujado por la
       3060 desde su lamina; (b) cada ventana compuesta por la 3060; (c) las
       gpu fn al correr en la 3060 (LB8, Ring 0: se decide con el
       propietario). Desglosada abajo (seccion 5)
   Q1  el juez de E8f/E8g en el metal: un .exe de DX12 con un cbuffer
       indexado, SampleLevel y Load en una 2D de una mip, que la puerta mande
       a la 3060; su tabla en Windows y despues en el Ryzen
       (PLAN_LA_LENGUA_DE_LA_3060)
   Q2  HECHA EN EL ANFITRION el 15-10: el sonido en TITAN++ (`use sonido`,
       las voces del orquestador) y el corte 4d de INTI (musica.ibx fuera
       del build y del disco; `apps/musica.bex` es el reproductor). Ver la
       fila Q2 de la hoja 6b: OIDO en el Ryzen el 15-10
   Q2b HECHA el 10-10: TITAN++ escribe byte a byte y el corte 4e de INTI
       (bico.inti y png.inti a TITAN++, seccion 1d)
   Q3  DOOM por la 3060 cerrado (D2c, la linea [perf]) y su paleta (D3)
   Q4  las texturas con mips en la VRAM, Ring 0: lo que deja a Cyberpunk
       mandar a la 3060 sus SampleLevel de verdad
   Q5  el Sample normal de un pixel (la mip de su cuadro)
   Q6  adelgazar la CPU de alrededor del dibujo (S3 y S4 de PLAN_VERRANO)
```

- [x] **R1 -- RESOLUCION** (10-10, el propietario: *"una app simple para
      configurar hasta cuantas Resolucion que quiero y encima control en
      ellas para cualquier ventana"*): `Ultra_userspace/apps/resolucion/`,
      en TITAN++. Ocho medidas, de 640x360 a 2560x1440; las flechas, las
      cifras 1-8 o el raton eligen, Enter la GUARDA en
      `datos/resol.txt` ("ANCHO ALTO") con `director.guarda` (TA4,
      nuevo), Esc cierra. Quien la LEE: cada ventana de TITAN++ que la
      quiera, con su `medida.titan` -- `titan/ventana.bex` ya se abre con
      ella --; y despues Q0a, la medida del dibujo de la 3060 por VERRANO.
      Pruebas: `emisor-x86_64/tests/resolucion.rs` (5) y `entrada.rs`.
      **Falta el metal:** `apps/resol.bex` desde su icono, Enter, y
      `run titan/ventana.bex` con la medida nueva. [!] La medida la elige
      cada app al abrirse: el DIRECTOR no agranda una ventana ya abierta
      (eso seria otra pieza: la ventana que cambia de medida, R-APP).

- [x] **R2 -- Alt+Tab MANDA en la pantalla** (10-10, el propietario: *"que
      SIEMPRE dominen en la pantalla asi sea con pantalla completa en GPU y
      CPU porque se parpadea"*). Parpadeaba por dos caminos: una app a
      pantalla completa compuesta por la CPU se pegaba encima de la tarjeta
      (no miraba lo que tenia delante), y la 3060 -- DOOM con
      `SUP_A_LA_3060`, PROTON-X con `SUP_LA_3060_DIRECTA` -- escribe la
      pantalla ENTERA cada fotograma. Ahora (`bmo_foco::encima`, con sus
      pruebas): la caja del conmutador no la pisa ninguna app; con el
      abierto la 3060 se queda QUIETA (ni un fotograma, y a la receta
      directa se le APARTA la pantalla: dibuja en su RAM, y el juego sigue
      corriendo); al soltar Alt se vuelca lo devuelto y la 3060 VUELVE con
      un fotograma. **Falta el metal:** Alt+Tab con DOOM por la 3060
      (`gpu doom`), con Cyberpunk por PROTON-X y con una app de CPU a
      pantalla completa: la tarjeta quieta, sin parpadeo, y al soltar el
      juego vuelve. [!] Que el juego se MUEVA detras de la tarjeta (como en
      Windows) pide que la 3060 componga la tarjeta encima: Q0b.

- [x] **R3 -- las ventanas YA NO SE PISAN** (10-10, el propietario: *"al
      estar con una ventana con cualquier app choca, se mezclan o uno
      predomina"*). El DIRECTOR no tenia apilado entre apps: las pegaba en el
      orden de su hueco (la que cambiaba despues tapaba a la otra, estuviera
      delante o no), siempre ENCIMA de las ventanas del sistema, el clic iba
      a la primera de la mesa, y el CROMO de una de detras (borde, titulo y
      fondo, pintado entero al perder el foco o al soltar Alt) caia encima de
      la de delante. Ahora hay UN apilado, la lista del foco
      (`foco::apilado`; la geometria pura y probada en `bmo_foco::encima`):
      cada app se pega sin los trozos que le tapan las de delante -- apps y
      del sistema --, de atras hacia delante; si una de detras repinta su
      cromo, lo de delante que pisa se devuelve; el clic, el realce y el
      borde vivo van a la que se VE; y una del sistema esta tapada solo si
      algo de DELANTE la pisa. Una app quieta destapada por otra que se movio
      se repega. **Falta el metal:** dos apps solapadas (NAVEGAR y
      RESOLUCION, o DOOM en ventana) y una del sistema (CABINA) encima y
      debajo, con clics. [!] En el modo Puntero (Alt+M) el foco sigue al
      raton, y con el el apilado: pasar por encima trae delante. [!] Una app
      que pone la 3060 a pantalla completa sigue escribiendo la pantalla
      entera: una ventana del sistema delante de ella pide Q0b.

**Lo del propietario, en cada arranque:** apagar (no reiniciar), `gpu init`
y `save`: cada `SALIDA.TXT` es una fila de G0 hasta diez. Y una decision que
espera sin prisa: el CUBO de E8g (la division de la 3060, la de la casa, o
esperar).

# 5. Q0, DESGLOSADA: lo que dice el codigo hoy (10-10)

F1..F5 estan HECHOS en el anfitrion (TITAN++ abre ventanas, lee lo que le
ofrecen, recibe teclas, escribe la letra, y NAVEGAR es suyo con los mismos
pixeles; INTI perdio la ventana, la lamina, la entrada y la letra: cortes 4c
y 4b). Lo que la regla 1b pide despues, leido en el DIRECTOR:

```text
   el aparato de la 3060   `gspcubo/sm86.rs`: la tuberia FIJA del estudio --
                           1280x720 y su fondo, o es "no valido" --, dibuja en
                           la PANTALLA (en RAM solo con `bmox12`, `enram`) y lee
                           de vuelta para el juez. VERRANO (`Backend::draw`,
                           un `Frame` a una `Image`) ya es la API correcta
   el compositor           el DIRECTOR pega cada ventana COPIANDO con la CPU
                           (el volcado de cada fotograma); la 3060 solo toma la
                           pantalla entera (DOOM con `SUP_A_LA_3060`, PROTON-X
                           con `SUP_LA_3060_DIRECTA`)
   la memoria de la app    la 3060 lee lo PRESTADO por la IOMMU si sus marcos
                           van seguidos (`loan::fisica_tomada`, lo de DOOM)
```

- [ ] **Q0a1 -- el aparato con viewport y fondo LIBRES**: la tuberia ya
      lleva `Destino { fila, ancho, alto }`; el aparato deja de exigir
      1280x720 y el fondo del estudio para un fotograma de VERRANO. **Como se
      sabe:** el banco con 640x360 IGUAL a la CPU de VERRANO (la reserva, los
      mismos bits), en el metal.
      **10-10, el DRIVER, hecho en el anfitrion:** `cubo::Ventana` lleva su
      medida y las ordenes sacan de ella el destino de color, el recorte y
      el viewport (`x * w/2 + w/2`, `y * -h/2 + h/2`); `Destino::valido`
      acepta cualquier medida de 1 a 4096 por lado que quepa en el mapa (8
      MiB: 1920x1080 cabe); con Z, solo 1280x720 (`tuberia::cabe`: la sombra
      y el bufer de Z miden eso), en el lector de VRN1/VRN2 y en el escritor
      de la receta. La puerta de los juegos (PROTON-X) se queda en 1280x720
      HASTA VERLO en el metal, y lo dice. Falta el aparato del DIRECTOR (con
      Q0a2, que es quien lo usa) y el metal.
      **12-10, la PUERTA DE LOS JUEGOS, abierta:** el metal de Cyberpunk (con
      su ventana ya en 1280x720, LI2j) dijo que 18 de 34 lotes los paraba la
      medida, y eran sus render targets propios (256x1, 256x256).
      `puerta::medida` (proton-x-sm86) deja pasar cualquier medida que el
      destino sepa -- filas de 128 en 128 B, dentro del mapa, desde una
      pagina -- y con la Z solo 1280x720; el censo dice cada NO por su
      nombre (`NoMedida`). Pruebas: `pruebas_medida.rs` (256x1, 256x256,
      640x360, 1920x1080 por la receta, y los NO). **Falta el metal.**
- [ ] **Q0a2 -- el destino EN RAM para la lamina** (como `enram` de
      `bmox12`): la 3060 dibuja en un bloque, no en la pantalla, sin leer de
      vuelta. **Como se sabe:** `gpu verrano banco inti enram` IGUAL al juez.
      **10-10, hecho en el anfitrion:** VRN1 LLEVA el color de la limpieza
      del destino (+20 = 1, +24 el pixel, la marca de la receta) y la 3060
      lo limpia -- la CPU deja de llenar 3,6 MB por fotograma en `enram` --;
      `enram` ya no pide `bmox12`: los vertices de la lamina van DIRECTO al
      VRN1 (`escribir_paquete_dibujo_de`, sin bufer), con el destino en RAM
      de la medida del fotograma; y el banco lee la lamina UNA vez por
      publicacion (`Lamina::leer_si_nueva`), no en cada mirada de 1 ms. El
      juicio final del banco con `enram` lee de la RAM, no por 460.800
      puertas. **Falta el metal:** `run titan/cubogira.bex` y, mientras
      corre, `gpu verrano banco inti enram` (IGUAL a D3D12).
      **Por VERRANO (1c):** la Image de `Backend::draw` con su medida; la
      puerta `sm86.rs` la traduce al `Destino` de Q0a1. ~~Le falta al
      paquete VRN1 el COLOR de fondo~~ -- **ya no (revisado el 14-10):** VRN1
      crecio, +20 = 1 marca que +24 es el pixel de la limpieza
      (`trabajos/tuberia.rs`), como dice el parrafo de arriba.
- [ ] **Q0a3 -- el DIRECTOR dibuja la lamina DENTRO de la ventana de su
      app** (por VERRANO, 1c: el DIRECTOR pide, la puerta dibuja): si el tid de una ventana tiene lamina, cada fotograma NUEVO de
      la lamina lo dibuja el aparato en los pixeles de esa ventana (Ring 0:
      la ventana prestada a la 3060 por la IOMMU, con sus marcos seguidos);
      sin la 3060, la CPU de VERRANO, y lo dice. **Como se sabe:** en el
      metal, `run titan/cubogira.bex` en una ventana que se mueve y se tapa,
      y su huella IGUAL al juez.
      **10-10, hecho en el anfitrion y SIN Ring 0:** la superficie de la app
      no le sirve a la 3060 (sus pixeles empiezan en +32: no van alineados a
      pagina), asi que la 3060 dibuja en un bloque DEL ESCRITORIO de la
      medida de la ventana y la ventana se compone desde ahi
      (`Surface::fuente`). `gspcubo::laminas`, en cada vuelta y al lado de
      `presentar_apps`: la lamina, su ventana (el mismo tid), una palabra si
      no hay nada nuevo, y con un fotograma nuevo `Backend::draw` en el
      `Aparato` -- abierto UNA vez, `en_la_imagen`: la `Image` ES el bloque,
      nada se copia de vuelta --. El DIRECTOR sigue sin nombrar la 3060
      fuera de la puerta. Si algo falla se dice UNA vez por la consola y la
      app se queda con lo que pinte ella. [!] Ventanas de ancho multiplo de
      32 y hasta 1280x720; el huella contra D3D12 solo existe a 1280x720
      (a otra medida, el juez es la CPU de VERRANO). **10-10, escrito:**
      `Cpu::juzgar` (el recuento pixel a pixel, `el_juicio_a_otra_medida`)
      y `laminas` juzga el primer fotograma de cada app y uno de cada 1024;
      la consola dice `[Q0a3] el juez ... la 3060 da lo MISMO` o cuantos
      pixeles da DISTINTO.
- [ ] **Q0a4 -- el cubo de TITAN++ pide su ventana**: `cubo_gira` abre
      `director.ventana` ademas de su lamina (TITAN++ ya sabe las dos).
      **10-10, hecho en el anfitrion:** 640x360 (16:9 como su camara, filas
      de 128 bytes), con su fondo y una linea de quien la pinta hasta que la
      3060 la tome. Y una trampa resuelta: ofrecer la lamina al MISMO
      escritorio SUSTITUYE la oferta de la ventana si aun no la tomo
      (`loan::offer`); `director.lamina`, con ventana, espera a que el
      escritorio la TOME (`SUP_TOMADA`, hasta medio segundo). La prueba
      `tests/lamina.rs`: la ventana primero, y los 360 fotogramas bit a bit.
      **Falta el metal (Q0a3 y Q0a4):** `run titan/cubogira.bex` desde
      Ejecutar: el cubo gira EN SU VENTANA, se mueve y se tapa.
- [ ] **Q0b -- la 3060 COMPONE**: el volcado de las ventanas por el motor de
      copia (o un programa de computo como `imagen`), no por la CPU.
- [ ] **Q0c -- las gpu fn al correr en la 3060** = LB8 de
      PLAN_LAS_LIBRERIAS (la puerta de computo, Ring 0, con el propietario;
      pide G0).

# 6b. LA HOJA DE LA ESCALERA (15-10): donde se anota que FUNCIONO

Una fila por pieza de la escalera (seccion 0b) que espera al Ryzen. Se prueba
en la misma sesion que la seccion 6, con un `save` al final. En `resultado`
se escribe `FUNCIONO dd-mm` (y la foto o el `SALIDA.TXT` que lo dice) o `NO:`
y lo que salio; con un FUNCIONO se tachan la subcasilla `en el Ryzen` y la
pieza entera en la 0b, y su fila de la tabla de estado pasa a `VISTO dd-mm`.

```text
   pieza  QUE SE MIRA                  COMO                              SI SALE MAL                 resultado
   A4     la lamina del nivel 15       `run titan/suelta.bex`: escribe   la novena lamina dice NO    FUNCIONO 15-10
          (TITAN++ pide RAM por la     `20 laminas de 4096 bytes, una    (no se soltaban), o un
          puerta y la suelta)          por vuelta: 190`; y `run          fallo de pagina
                                       titan/laminas.bex`: `100 50 3 7`,
                                       `200`, `497`, `350`
   B1     la lente al admitir, UNA     `run titan/cubogira.bex` dos      un `.bex` bueno rechazado;  (sin probar)
          vez                          veces; en la bitacora: la 1a      o la 2a vez la lente lee
                                       `lente: DEMOSTRADO`, la 2a        otra vez (sin `ya leido`)
                                       `lente: ya leido`
   B2     la autopsia dice la          `run c/autopsia.bex` (lee de      sin renglon `instr`, o una  FUNCIONO 15-10
          instruccion                  0xDEAD0000 a proposito); en       instruccion que no es la
                                       `datos/fallos.txt` el renglon     del `rip`, o la consola
                                       `instr`: la instruccion y su      dice `no fallo`
                                       registro, que dan 0xDEAD0190
   B3     el cubo en SU ventana        las filas Q0a2..Q0a4 de la        lo de esas filas            (sin probar)
                                       seccion 6
   B4     el perfil al arrancar        en la bitacora del arranque:      `NO es la del perfil` en    (sin probar)
                                       `cpu: es la CPU del perfil` y     el Ryzen, o una DECIDIDA
                                       `extensiones decididas que el     que el silicio no tiene
                                       silicio tiene` = 11
   D1     el cubo.bsf de TITAN++       `gpu verrano` (y `gpu verrano     el juez de la puerta dice   (sin probar)
          (V4)                         banco`): IGUAL en el 0, el 30 y   que NO al abrir, o DISTINTO
                                       el 60, con el sobre que ya no es  en algun fotograma: se
                                       SASS a mano                       vuelve al de antes con git
   RX     ver un .bex por dentro       F12 > DATOS > apps, clic derecho  no sale "ver por dentro",  (sin probar)
          (KA-1 de PLAN_EL_KERNEL_     sobre ecce/nucleo.bex > "ver por  o la puerta dice NO a un
          DE_LA_APP)                   dentro" (o `dentro apps/ecce/     .bex que corre, o la lente
                                       nucleo.bex`
                                       en Ejecutar): regiones, anexos,   no DEMOSTRADO
                                       lente DEMOSTRADO, el manifiesto
                                       y "el kernel lo ADMITIRIA"; al
                                       final, las 16 primeras
                                       instrucciones DICHAS EN
                                       CASTELLANO (y en un `.ibx` de
                                       INTI, sus katanas por nombre)
   B2c    la autopsia lo dice en       `run c/autopsia.bex` y `fallo`:   sin renglon `dice`, o una  (sin probar)
          castellano                   debajo de `instr`, el renglon     frase que no es lo que
          (lente en castellano)        `dice  lee 4 bytes de la         hace la instruccion
                                       memoria en [rax] con signo y lo
                                       deja en rax`
   KA1    ECCE es una CARPETA          el escritorio: UN icono `ecce`;   dos ECCE, o ninguno, o la   (sin probar)
          (PLAN_EL_KERNEL_DE_LA_APP)   abrirlo; la ventana, la barra y   ventana dice `nucleo`, o
                                       Alt+Tab dicen ECCE. F12 > DATOS > el `ecce.bex` viejo sigue
                                       apps, clic derecho sobre `ecce` > en `apps\`
                                       "ver la app por dentro": NUCLEO
                                       ... ENTRA y "la app ENTERA entra"
   LX1    un .exe de Windows por       copiar un .exe (el BMOX-12 de     no sale nada, o la lista    (sin probar)
          dentro (PLAN_PROTON_X, 4d)   EPICX) a DATOS y `dentro X.exe`   no es la de `dumpbin
                                       (o clic derecho > "ver por        /imports`, o una llamada a
                                       dentro"): PROTON-X lo correria,   Windows sin su nombre
                                       lo que pide DLL a DLL, la lente
                                       y "llama a kernel32.dll!..."
   LX2    lo que FALTA con el         lanzar un .exe al que la casa le  sin el bloque "DESDE DONDE  (sin probar)
          binario al lado              falte algo (el censo dice cual):  las pide", o un sitio sin
          (PLAN_PROTON_X, 4d)          debajo de "no arranca: faltan",   las instrucciones de antes
                                       por cada una, el sitio, la
                                       funcion del juego y lo de antes
                                       en castellano
   MESA   la mesa de ECCE (PL9b de     ECCE con una nota larga (mas de   Alt+rueda no hace nada, o  (sin probar)
          PLAN_ECCE)                   una hoja): Alt + rueda abajo: la  el clic abre otra hoja, o
                                       mesa; otra vez: los nodos; un     Esc en la mesa CIERRA ECCE
                                       clic en un nodo abre esa hoja con (debe solo volver)
                                       el cursor al principio
   GUION  ECCE en modo guion           una nota con "INT. CASA - NOCHE"  todo pegado a la izquierda, (sin probar)
          (PL2..PL4 de PLAN_ECCE)      y un personaje con su dialogo:    o la escena no sale en oro,
                                       ECCE abre en GUION (la escena en  o Alt+G no cambia de modo
                                       oro, el personaje a 22, el
                                       dialogo a 10); Alt+G a notas
   COPIA  seleccionar y copiar en      Mayus + flechas, Alt+C, Alt+V;    Alt+C no copia, o Ctrl+C   (sin probar)
          ECCE                         Alt+X y Alt+Z lo devuelve          (que es del escritorio)
                                                                          cierra sin querer: es lo
                                                                          suyo, ver keys/windows.rs
   FD     escribir un guion como en    ECCE en guion (con un INT.):      Tab no cambia la linea, o  (sin probar)
          Final Draft (PL3 de          "ana" + Tab = @ANA (la @ tenue);  Enter tras el personaje
          PLAN_ECCE)                   Enter baja al dialogo (a 10); otro deja una linea en blanco,
                                       Enter deja el blanco; Tab en el   o Alt+Z no lo deshace
                                       blanco del dialogo = "(|)"
   FOU    abrir un .fou y guardar      Alt+F en ECCE: la cabecera dice   el explorador no abre ECCE, (sin probar)
          como guion (PL3)             datos/guion.fou; F12 > DATOS >    o abre las notas, o guarda
                                       ese fichero: abre ECCE con el     en datos/notas.txt
                                       guion, y lo escrito se guarda ahi
   RATON  el raton en la hoja (PL3)    un clic en una letra pone el      el cursor cae en otra       (sin probar)
                                       cursor ahi; arrastrar selecciona; linea (la de arriba o
                                       Alt+X corta lo arrastrado         abajo), o arrastrar no
                                                                         selecciona
   PAG    las paginas del oficio       un guion de mas de una pagina en  sin raya, o el (MAS) y el  (sin probar)
          (PL4 de PLAN_ECCE)           ECCE: donde acaba la pagina, la    (CONT.) no salen al partir
                                       raya con "pag 2"; un dialogo       un dialogo, o el pie no
                                       partido lleva "(MAS)" y "NOMBRE   dice "pag N de M"
                                       (CONT.)"; Alt + rueda: la mesa
                                       muestra esas paginas
   PDF    el guion en PDF (PL6 de      ECCE con un guion: Alt+P, la      no sale datos/guion.pdf, o  (sin probar)
          PLAN_ECCE)                   cabecera dice "PDF: datos/        un lector de PDF no lo
                                       guion.pdf"; copiarlo al PC y      abre, o las columnas no
                                       abrirlo: Courier, la escena a     son las de un guion
                                       1,5", el personaje a 3,7", el
                                       dialogo a 2,5", "2." arriba
   LETRA  la cabeza y el pie de ECCE   abrir ECCE: "ECCE" en negrita     la de 8 x 16 de siempre (el (sin probar)
          con la letra de la casa      con el borde suave, "notas" y la  build no dejo letra.blt), o
          (LE2 de PL18)                ruta finas, el pie igual; teclear letras cortadas, o ECCE va
                                       no va mas lento que antes         a tirones al teclear
   IC     la cara ELEGANTE de ECCE     el icono de ECCE en el escritorio sale el cuadro con la E,  (sin probar)
          (32 px, borde suave, de      a 32 con el borde suave (la hoja  o con bordes en escalera,
          su `icono.svg`)              y la pluma de oro), sobre la foto o un halo oscuro alrededor
                                       de fondo y marcado (la pastilla)
   EL0    lo que TOCA cada programa    abrir ECCE, TINTA y MUNDO,       la tabla no sale, o todo    (sin probar)
          (PLAN_EL_TOQUE.md)           usarlos un poco y CERRARLOS;      a cero, o un fallo al
                                       `save`: capitulo 5, la tabla      cerrar una app (el
                                       "lo que TOCO de lo que tiene"     enterrador cuenta antes
                                       (d.bex en vivo, las otras al      de desmontar)
                                       cerrarse) -- y la foto
   PL1    ECCE, el bloc de notas        su icono (o `run apps/ecce.bex`): no abre, las letras no    (sin probar)
          (PLAN_ECCE.md)               escribir con tildes y ene, Enter,  salen, Alt+Z no deshace,
                                       flechas, Alt+Z y Alt+Y; Esc;      o al volver a abrir la
                                       abrirlo otra vez: la nota sigue   nota no esta
                                       ahi (datos/notas.txt)
   Q2     el sonido de TITAN++         `run apps/musica.bex`: dice       no suena (el tubo sin      FUNCIONO 15-10
                                       `voces   `, cada nota y `fin     armar: mirar `audio` en
                                       0`, y SUENA el tema por el        CABINA), o suena a trozos
                                       audifono USB (la misma melodia    (el banco que se reescribe
                                       que tocaba `inti/musica.ibx`)     antes de acabar)
   C4     los contadores del Zen 3     `cpu contadores`: cada fila con   una fila `cuenta OTRA       FUNCIONO 15-10
                                       lo contado y `CUENTA lo que dice  cosa` o `no conto NADA`
                                       el PPR`; al final `8 de 8`. Las   (apuntar CUAL: ese evento
                                       dos de la L1D son las que el      o su umask no es el de
                                       metal confirma o no               este Ryzen)
   C3     el volcado por la 3060, por  `gpu init` (o el arranque con la  `[C3] ... fallo tres         (sin probar)
          defecto (C3a)                3060 lista): `[C3] la 3060        veces`, o la pantalla con
                                       vuelca el escritorio por          basura o a trozos, o `gpu
                                       defecto` en la consola y la fila  volcado` dice menos de 1024
                                       `volcado` con 1024 de 1024; el    muestras
                                       raton y las ventanas se mueven
                                       igual; `gpu volcado off` lo
                                       devuelve y no vuelve solo
   C2     el banco duerme en el sello  `run titan/cubogira.bex` y        el banco se queda           (sin probar)
          (S3)                         `gpu verrano banco inti`: la      parado (2 s y `callada`),
                                       linea `C2, el sello: la app TOCA  o `NO toca el timbre`, o la
                                       EL TIMBRE, N veces dormido` y     latencia peor que con el
                                       la latencia publicado-dibujado;   `wait` de 1 ms
                                       el juez, IGUAL
   C1     la lamina PRESTADA a la      `run titan/cubogira.bex`: en la   `[C1] la 3060 no tomo la    (sin probar)
          3060, sin copia              consola `[C1] la lamina va        lamina prestada` (y el
                                       PRESTADA a la 3060: sin copia`,   porque 29/30 en la
                                       el juez `la 3060 da lo MISMO`,    bitacora), un fallo de la
                                       y el cubo gira igual que antes    IOMMU, o el juez DISTINTO
```

** 15-10, 08:24-08:34 (la primera sesion de la hoja; los informes de `save`
08:30, 08:32 y 08:34, y cinco capturas):

```text
   A4  FUNCIONO  `20 laminas de 4096 bytes, una por vuelta: 190`, y
                 `100 50 3 7`, `200`, `497`, `350`
   C4  FUNCIONO  `8 de 8 filas cuentan lo que dice el papel`; las dos de la
                 L1D, CONFIRMADAS: accesos 129195 y fallos 104523 en 100000
                 vueltas, dentro de lo esperado
   B2  FUNCIONO  AUTOPSIA: `instr  movsxd rax, dword [rax]  rax=0xdead0190`,
                 la direccion tocada 0xdead0190 -- exacto como en el
                 anfitrion; `recursos todo devuelto`
   Q2  FUNCIONO  `musica.bex` escribio TODAS las notas y el kernel toco
                 (`tocadas 1`, el medidor con pico 0.0 dB y rms -8.2 dB). El
                 propietario: *"funciona pero lo corte porque es molesto lo
                 repetitivo"* -- se OYO; el `^C frenado` fue suyo. Lo que
                 queda es de gusto, no de cadena: la onda cuadrada suena
                 a pitido (lo que viene, en PLAN_EL_SONIDO.md)
   B1  sin ver   `cubogira.bex` corrio dos veces (admitido las dos), pero la
                 linea `lente: ...` vive en la bitacora (F11), no en `save`
   B4  sin ver   la linea `cpu: es la CPU del perfil` vive en la bitacora del
                 arranque (F11)
   C1 C2 C3 D1 B3  sin probar. El save dijo `GSP_INIT_DONE NO llego en
                 10 s`, PERO ERA MENTIRA DEL RESUMEN (investigado el 15-10):
                 `save mode` ya lo habia visto llegar -- por eso la fila de
                 abajo, `GET_GSP_STATIC_INFO CONTESTADA`, y los objetos NV_OK
                 --; el `gpu init` tecleado DESPUES no puede volver a correr
                 el secuenciador (`IOMMU_NO_SEC_YA`) y guardaba su resumen
                 vacio ENCIMA del bueno (las huellas: `1 tramos`, sin `dijo:`,
                 BAR1 despues 0). Arreglado en `commands/gspinit.rs`: una vez
                 por arranque, `gpu init` repetido lo dice y no toca nada.
                 La proxima sesion: NO teclear `gpu init` si save mode ya lo
                 hizo (o teclearlo: ahora contesta "ya llego")
                 Y el "gsp NO desperto" de tantos arranques (el propietario,
                 15-10: "tengo que repetir muchas veces el reinicio") NO es
                 la tarjeta caliente: es el 0x15 del booter en arranques
                 FRIOS, el fallo abierto G0 de PLAN_LA_3060 (sus datos, en
                 `platform/drivers/gpu/ga10x/EL_0x15.md`: 5 de 7 el 29-09, y
                 tambien con apagado completo). El `reboot` del escritorio
                 ya apaga el GSP antes (boot_ul); el de la pantalla de fallo
                 y el del shell serie, no. Y la fila `al llegar` pintaba
                 `0x80000000` en el WPR2 de una tarjeta FRIA: era la bandera
                 de "foto tomada", no el registro (arreglado en
                 `gspsalud.rs`, 15-10)
   y ademas      tinta, mundo y resol abrieron su ventana (mundo ofrecio su
                 lamina); ni un aviso en el anillo salvo la caida buscada
```

## 6b1. LA SESION, EN ORDEN (15-10): que se trae y que foto se saca

El propietario (15-10): *"dime los elementos que traere en BMO-X para darle
el check en orden y preciso"*. Las once filas de arriba en UNA sesion, en
este orden (cada paso deja listo el siguiente), con una foto por paso.

```text
   SE TRAE   el USB con el build de HOY (`desplegar.ps1`), la 3060 puesta,
             el audifono USB, el raton y el movil para las fotos (y un
             video corto con sonido para Q2)

   paso  fila   QUE SE TECLEA                      LA FOTO DEBE MOSTRAR (*) 
    1    B4     (nada: arrancar)                   la bitacora del arranque:
                                                   `cpu: es la CPU del perfil` y
                                                   `extensiones ... = 11`
    2    C4     `cpu contadores`                   las 8 filas y `8 de 8`
    3    A4     `run titan/suelta.bex`             `20 laminas ... : 190`
                `run titan/laminas.bex`            `100 50 3 7`, `200`, `497`, `350`
    4    C3     `gpu init`                         `[C3] la 3060 vuelca el
                `gpu volcado`                      escritorio por defecto` y
                                                   `1024 de 1024`; mover el raton
                                                   y una ventana
    5    C1     `run titan/cubogira.bex` (1a vez)  el cubo en SU ventana; en la
         B3     desde Ejecutar                     consola `[C1] ... PRESTADA` y
         B1                                        `[Q0a3] ... da lo MISMO`; en la
                                                   bitacora `lente: DEMOSTRADO`
    6    B1     `run titan/cubogira.bex` (2a vez)  la bitacora: `lente: ya leido`
    7    C2     `gpu verrano banco inti`           `C2, el sello: la app TOCA EL
                                                   TIMBRE` y la latencia
         B3     `gpu verrano banco inti enram`     el juez IGUAL (Q0a2)
    8    D1     `gpu verrano`                      IGUAL en el 0, el 30 y el 60
                `gpu verrano banco`
    9    Q2     `run apps/musica.bex`              `voces   `, las notas y
                (con el audifono)                  `fin     0` -- y el VIDEO
   10    B2     `run c/autopsia.bex`               que la app cae (y NO dice
                                                   `no fallo`)
   11    --     `save`                             nada: se trae la carpeta
                                                   `informe\` entera (SESION.TXT,
                                                   AUTOPSIA.TXT con el renglon
                                                   `instr`, ...)

   (*) Si la consola corre muy rapido, vale `save` y el SESION.TXT en vez de
       la foto. Lo que salga MAL se fotografia igual: un NO con su foto vale
       tanto como un FUNCIONO.
```

Con lo traido se escribe la columna `resultado` de cada fila y se tachan las
subcasillas `en el Ryzen`. Si sobra tiempo, la seccion 6 de abajo (R2, R3,
DOOM, ESTUDIO, MUNDO, TINTA) va en la misma sesion, despues del paso 10 y
antes del `save`.

# 6. QUE FALTA COMPROBAR EN EL METAL (10-10)

Todo esto esta HECHO y probado en el anfitrion; ninguno se ha visto aun en
el Ryzen con la 3060. Una sesion, en este orden, y un `save` al final: cada
linea dice que se mira y que seria un NO.

```text
   QUE                         COMO                            SI SALE MAL
   R2  Alt+Tab domina          Alt+Tab con una app normal, con  la lista detras
       la pantalla             DOOM por la 3060 a pantalla      de la app, o
                               completa y con la CPU            parpadeo
   R3  las ventanas se         dos apps encima una de otra;     una se mezcla
       apilan                  clic en la de atras: sube        con la otra
   R1  la resolucion           `run apps/resol.bex`, una   la ventana no
                               medida, y abrir otra app         toma la medida
   4e  bico y png en TITAN++   `run titan/bico.bex` y           pixeles mal o
                               `run titan/png.bex`              el fichero no
                                                                se escribe
   Q0a2 la 3060 dibuja EN RAM  `gpu verrano banco inti enram`   huella distinta
                               con `run titan/cubogira.bex`     del juez
   Q0a3 el cubo EN SU VENTANA  `run titan/cubogira.bex` desde   la ventana se
   Q0a4                        Ejecutar: gira, se mueve, se     queda con su
                               tapa con otra; en la consola,    linea de espera;
                               `[Q0a3] el juez ... 640x360:     o el juez dice
                               la 3060 da lo MISMO`             DISTINTO
   1   el plazo del fotograma  la linea de fotograma del cubo:  ~18 ms otra vez
                               ~16,7 ms de pared, poco esperar
   2   el turno al despertar   la misma linea: despertar tarde  ~3,5 ms medio
                               < 1 ms
   3   DOOM mira 1 de 32       `gpu doom`, la linea `[perf]`    ~560 us de
                               de la 3060                       comprobar
   6   el cubo en caliente     `gpu verrano banco inti` con el  `preparar` de
                               cubo girando: casi todos los     ~5 ms en los
                               fotogramas en caliente           que giran
   M1  DOS CUBOS que se tapan  `run titan/doscubos.bex`: el de  se ven caras de
       (V2, la profundidad)    atras cruza POR DETRAS del de    dentro, o el de
                               delante; la consola: el juez     atras encima;
                               `... la 3060 da lo MISMO`        juez DISTINTO
   VE2 ESTUDIO, la luz que se  `smp all` y `run sys/estudio.bex` la caja sin luz, o
       recuerda (estilo          desde Ejecutar: la caja de       la consola con
       RenderMan, VERRANO)       Cornell se limpia sola; a d w s  1 parte con
                                 mueven; la consola: los ms y     `smp all`; un
                                 las PARTES (los nucleos); se     panico; con `c`
                                 ve LIMPIA (VE5, completada) y    manchas o
                                 `c` la deja cruda, con grano     bordes borrados
   ** 11-10, 04:10-04:16 (METAL_2026-10-11): VE5 VISTO (`gpu completa`,
   76800 de 76800 en las 3 pasadas); VEL VISTO en 1 nucleo (estudio, ~95 ms,
   "completada en la 3060", sin #GP); R1 VISTO (`resol.bex`); MC1 NO (la
   ventana sin dibujo: el por que, ahora, en la caja); `smp all` reinicia.
   VEL ESTUDIO a toda          `gpu lienzo`, `smp all`, `run    un #GP al
       velocidad (SSE en Ring    sys/estudio.bex`: la consola     abrir (la pila),
       3 y la 3060 completa)     dice "completada en la 3060" y   "la 3060 no
                                 los ms por fotograma; `g` lo     completa", o
                                 pasa a la CPU: la diferencia     los mismos ms
   VE5 COMPLETAR por la 3060   `gpu completa` (tras `gpu         un pixel
       (el filtro de ESTUDIO     lienzo`): tres lineas "76800 de  distinto, la
       VIVO, juzgado pixel a     76800 pixeles iguales al juez"   3060 sin pagar,
       pixel)                    y, arriba a la derecha, la luz   o un Xid
                                 con grano y COMPLETADA
   MC1 MUNDO, los bloques      `run apps/mundo.bex`: el suelo   ver la lista de
       (V2b, el recorte de     hasta abajo; w s a d q e r f     la seccion 3 de
       cerca)                  andan; x quita, c pone tablas;   PLAN_MUNDO
                               el juez de `laminas`
   TB1 TINTA, el lienzo        desde su icono: pintar, la goma  ver la lista de
                               (derecho), s guarda el PNG       5b2 de PLAN_LA_
                                                                TINTA
   5   EL RESPIRO              `gpu doom` y el cubo, moviendo   pantalla azul,
                               el raton; en `save`: `respiro`   latido tarde
                               y `apartadas` SUBEN, `latido     con la 3060,
                               tarde` sin la 3060 de culpable   el raton a
                                                                tirones
   5   la puerta larga, de     cualquier `save` con una puerta  op 4 de otro
       quien la llamo          larga: el nombre es el de quien  (el disco) =
                               llamo, y si es op 4 es el disco  el siguiente
   op4 el cierre, partido      escribir un fichero (`save` ya   `vaciar` en 0
                               lo hace) y otro `save`: las      con `guardar`
                               filas `guardar` (DENTRO de la    grande = la
                               puerta) y `vaciar` (el FLUSH,    escritura era
                               ya fuera); y la puerta larga     el problema
                               ya no es op 4 por el FLUSH       (abajo)
```

**Mirado y NO es parche (10-10):** la purga (`core/purga.rs`) cede con
`yield_current` en bucle, pero corre en el hilo del bus o en el shell de
Ring 0, nunca dentro de un syscall: ahi ceder SI cambia de tarea. Y con el
respiro, la tarea que este dentro de la 3060 no se purga a medias: muere al
salir de su puerta, y la purga la espera cediendo.

**El respiro y el disco (10-10):** el disco tomado es un cerrojo tambien. Una
puerta de la 3060 que tuviera el disco y respirara dejaria al hilo del disco
girando con IF=0 en `tomar_disco` para siempre: el respiro no abre con el
disco tomado (`disk::tomado`).

**Si el respiro sale mal:** `RESPIRO = false` en `ring0/dev/respiro.rs`
y la espera vuelve a girar entera con IF=0, como antes del 10-10
(la puerta unica y la tarea intocable se quedan: no cuestan nada). Con la
pantalla azul, su foto dice donde: si es en el `cli` o justo despues, la
pila re-publicada; si es al matar una app, la tarea intocable.

**Las piezas que aun son parche** (se cambian cuando toque, no se olvidan):

```text
   op 4, la escritura     el FLUSH ya va en vuelo (10-10); los SECTORES del
                          fichero, la FAT y la entrada se siguen escribiendo
                          dentro de la puerta, esperados uno a uno. Si
                          `guardar` sale grande en el metal, la pieza: el
                          guardado entero por el hilo del disco, por trozos,
                          y la puerta solo encola
   el tic de 1 ms         el turno de quien despierta espera a un tic: un
                          reloj sin tic (2 del cuello)
   la copia de pantalla   la CPU, 27,6 ms entera: el motor de copia por
                          defecto, el page flip, el PCIe en Gen3 (4)
```

**Mirado y se queda asi, a proposito (10-10):** los ~700 B de programas que
el DIRECTOR copia al paquete en cada fotograma, y la copia de los vertices de
la lamina al paquete (n x 32 B). Juntas son menos de un microsegundo de CPU
contra un fotograma de 16,7 ms; un paquete persistente con programas
"ya escritos" agregaria estado que puede quedar viejo, para ahorrar nada que
se vea. Lo caro estaba en el kernel (el frio de la 3060, el FLUSH, la espera
con IF=0), y eso es lo que se cambio.
