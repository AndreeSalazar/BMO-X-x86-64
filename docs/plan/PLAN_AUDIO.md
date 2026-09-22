# PLAN AUDIO -- las casillas de su MAESTRO, medidas contra el codigo

> Escrito el **2026-08-25**, cuando `AUDIO_MAESTRO.md` llego a codigo. Lo pide el
> indice de esta carpeta por su propia regla:
>
> > *"El par MAESTRO + PLAN es la simetria de esta carpeta (...) **el dia que un
> > maestro llegue a codigo, su plan va aqui y con este nombre**."*
>
> El **por que** de cada eleccion --y las cinco cosas que ese documento se niega
> a prometer-- vive en [`AUDIO_MAESTRO.md`](../maestro/AUDIO_MAESTRO.md). Aqui
> solo hay casillas.

---

# 0. EL ESTADO, EN UNA TABLA

| paso del maestro | que es | estado |
|---|---|---|
| **0** -- el aparato dice quien es | parsear AudioStreaming | ⚠ **el parser vale; el CENSO miraba donde no era.** Arreglado 25-08 |
| **1** -- `SET_INTERFACE` | poner el alt que trae el endpoint | ⛔ **EL RYZEN LO NEGO el 25-08.** Ver A1 |
| **2** -- el TRB isocrono | mandar silencio | ✅ **escrito 25-08**, sin ejecutar |
| **3** -- WAV | PCM en un sobre. Cero decodificador | ✅ **HECHO 25-08**, 12 pruebas |
| **4** -- el bufer prestado | dos indices, y CERO copias | ✅ **HECHO 25-08**, sin ejecutar |
| **5** -- MP3 | encima del mismo tubo | ⛔ falta, **y va el ultimo** |
| **6** -- el audio se INTEGRA (21-09) | el audifono lo RECLAMA el que enumera; el volumen va por el hilo del bus; el `save` lo cuenta entero | ✅ **HECHO 21-09**, sin ejecutar. Ver A6 |

★★ **Y la fila que importa cambio el 25-08: el camino entero esta escrito.**
Descriptor, endpoint, alt, frecuencia, TRB isocrono y el bucle que alimenta.

⚠⚠ **PERO EL 25-08 POR LA TARDE EL RYZEN CONTESTO, Y DIJO QUE NO** (26-08). El
aparato aparece, los ocho numeros salen, y `el tubo NO esta abierto`. O sea que
**A1 no estaba hecho: estaba escrito.** La diferencia es la que este documento
lleva repitiendo desde su primera linea, y esta vez la pago el.

★ Lo que se encontro leyendo esta en A1. Y lo que se aprendio del proceso vale
mas que el fallo: **la linea que decia POR QUE salia por el cable de serie**, o
sea a un sitio al que el dueno no vuelve. Un fallo que solo se puede diagnosticar
desde Ring 0 es un fallo que no se puede diagnosticar.

---

# 1. LAS CASILLAS

## [!] A0 -- **EL RYZEN CONTESTO, Y EL CENSO MIRABA DONDE NO ERA** (2026-08-25)

La primera foto del audio en metal:

```text
   audio: ningun aparato de reproduccion en los puertos libres
   audio: puertos libres mirados, y ninguno reproduce  =0
```

★★ **CERO. No es que mirara y no encontrara: no llego a mirar nada.** Y esa
distincion --que esa linea existe para dar-- resolvio el fallo sola.

### Dos caminos buscando el mismo aparato, mirando cosas distintas

```text
   el VOLUMEN        recorria SLOTS 1..8   -> lo encontraba desde hace dias
   la REPRODUCCION   recorria PUERTOS      -> no lo veia nunca
```

El audifono **ya estaba enumerado** --por eso el volumen funciona-- asi que tenia
slot, su puerto no figuraba libre, y `direccionar_puerto` no puede re-direccionar
lo que ya tiene direccion. Los dos filtros lo descartaban antes de leer un byte.

> El aparato estaba ahi. El censo de reproduccion miraba donde no estaba.

### Como quedo

Los dos caminos recorren **slots**, y **comparten el lector de descriptores**
(`uaudio::leer_configuracion`). No se escribio un segundo lector:

*** **Dos lectores del mismo descriptor son dos sitios donde ese descriptor se
puede leer distinto.** Es la misma leccion que `bmo-bex-gate` y que `reloc_cabe`,
por tercera vez esta semana.

** Y la linea de diagnostico se conserva con el nombre cambiado --`slots mirados,
y ninguno reproduce`-- porque fue la que lo resolvio. Un `0` ahi seguira
significando *"no llegue a mirar"*, que es otro sitio donde buscar.

## [X] A0b -- el paso 0, tal y como estaba escrito

Al ir a hacer el paso 0 resulto que **estaba hecho y cableado**:
`bmo_uaudio::stream::find_playback` son 509 lineas con **25 pruebas**, y
`dev/usb/audio.rs` ya lo llama y apunta los cuatro numeros por CABINA.

⚠ **Lo unico que le falta es un arranque.** Es la cuarta vez este mes que
aparece codigo escrito, compilado, probado y **sin ejecutar** -- ver `banda`,
`cabina` y `red rx`. La diferencia es que este si esta cableado: solo falta
mirarlo.

**Como se sabe que quedo hecha**: la foto, y esta **predicha** en el maestro:

```text
   audio: interfaz AudioStreaming, alt      =1
   audio: canales                           =2
   audio: bits por muestra                  =16
   audio: bytes por trama (wMaxPacketSize)  =192
   audio: frecuencia elegida                =48000
   audio: el endpoint isocrono es el DCI    =...
```

## [ ] A1 -- `SET_INTERFACE` -- ⛔ **EL RYZEN LO NEGO. Corregido el 26-08, sin ejecutar**

**Que era**: pedirle al aparato el alt setting que trae el endpoint isocrono
(`SET_INTERFACE`, request 0x0B) y configurar ese endpoint en el xHC.

*** **Era el que separaba "escrito" de "suena".** El paso 0 sabia cual era el
alt; el paso 2 sabia encolar una trama. Y **nadie le habia dicho al aparato que
se pusiera en ese alt**, asi que su endpoint no existia.

**Como quedo**: `dev/usb/audio.rs::abrir`, y son tres pasos **en este orden**:

```text
   1. configurar el endpoint en el xHC   el HOST se prepara
   2. SET_INTERFACE (0x0B)               el APARATO empieza su reloj
   3. SET_CUR frecuencia                 solo si declara mas de una
```

★★ **El orden es una decision.** Se prepara el host antes de que el aparato
arranque su reloj: al reves hay una ventana en la que el aparato ya espera datos
en cada microtrama y el xHC no tiene ni anillo donde ponerlos. Con `OUT` eso no
rompe nada --recibe silencio-- pero **cuenta como tramas tarde**, y entonces el
primer numero que se mira al depurar estaria sucio desde antes de empezar.

** Y el paso 3 solo se manda **si hay mas de una frecuencia que elegir**: un
aparato de una sola puede contestar STALL, con razon, y un error que sale en cada
arranque deja de ser un error.

[!] `EP_ISOCH_OUT = 1`, y la tabla del xHCI **no es la del USB**: aqui `1` es
Isoch OUT, `5` Isoch IN y `7` Interrupt IN --el del teclado--. Meter el numero
del USB da un endpoint del tipo equivocado, y eso no falla al configurarlo:
falla al primer TRB.

★ Y desde el 26-08 ese numero **ya no se escribe aqui**: se pide al driver
(`bmo_xhci::EP_TYPE_ISOCH_OUT`). Era una copia de una tabla del controlador, y el
driver la necesita para si mismo -- ver abajo.

### ⛔ LO QUE CONTESTO EL METAL, Y LO QUE SE ENCONTRO (26-08)

```text
   aparato de reproduccion HALLADO
   los ocho numeros estan en F11
   el tubo NO esta abierto: no puede sonar nada todavia
```

`abrir()` solo puede fallar en dos sitios y uno se descarta solo: si el
descriptor se contradijera, la linea `ninguna frecuencia suya cabe en su propio
paquete` habria salido en F11. **Fue `configure_endpoint`.**

*** **Y lo que se encontro leyendo el driver es un `3` que vale para el
teclado**: `configure_endpoint` escribia `CErr = 3` en DW1 del Endpoint Context
para TODOS los endpoints.

> xHCI 6.2.3.5 -- *"CErr ... shall be set to '0' for Isoch endpoints."*

No es estilo. Una transferencia isocrona **no se reintenta** --la muestra llega a
tiempo o no existe-- asi que un contador de reintentos ahi es un campo con un
valor que el hardware declara imposible. Un xHC estricto contesta **Parameter
Error (cc=17)**. Y que el de AMD lo es ya estaba demostrado en el mismo fichero:
con `CH=1` encadenando las etapas de control contestaba Transaction Error donde
QEMU lo toleraba.

** Y el segundo numero clavado, que no da error sino **tramas tarde**: el
`Average TRB Length` estaba en `8`, el tamano de un informe de teclado boot. Para
192 bytes cada milisegundo es declarar **24 veces menos ancho de banda del que se
va a gastar**.

### [!] Y LA PARTE QUE NO ES DEL AUDIO: el `cc` no llegaba a nadie

El codigo de terminacion se escribia con `h.log`. Desde el escritorio, *"el xHC
no configuro"* y *"el aparato no acepto el alt"* se ven igual, y son dos sitios
distintos. Ahora `last_cfg_ep_cc()` lo apunta y `usb/audio.rs` lo pone en CABINA:

```text
    8  ancho de banda    el intervalo no cabe en la agenda periodica
   17  parametro         algun campo del contexto no vale
   19  ya corria         sale si se pide `audio` dos veces seguidas
```

⚠ **Y un aviso para la proxima prueba**: `audio` dos veces seguidas puede dar
`19` legitimamente, porque reconfigura un endpoint que ya corre. Si sale 19,
reiniciar y pedirlo **una sola vez**.

★ Ademas se comprueba el estado del endpoint despues de configurarlo:
**configurado no es corriendo**, y esa distincion ya costo el teclado una vez --
todo verde y ni un evento.

## [X] A2b -- EL SILENCIO, y se pide a proposito

`audio silencio` arma el empuje; `audio calla` lo para. El hilo del bus encola
**ocho tramas por latido** --cuatro para cubrir los 4 ms, mas cuatro de
colchon-- y toca el timbre **una vez**.

*** **Y no se enciende solo al arrancar.** Abrir el tubo es configuracion y es
seguro; empujar tramas es **trafico continuo a 250 latidos por segundo**, y eso
no debe pasar en cada arranque mientras no haya nada que reproducir. Es la regla
de las hojas de metal metida en el codigo.

** El timbre va FUERA del bucle: uno por trama serian 2.000 escrituras MMIO por
segundo para mover 192 bytes cada una -- **el aviso costaria mas que el dato**.

## [X] A2 -- el TRB isocrono -- **ESCRITO el 25-08**, y sin ejecutar

`queue_isoch_out` en `platform/drivers/usb/xhci/src/transferencia.rs`.

**Las cuatro diferencias con su hermano `queue_interrupt_in`**, que es donde se
equivoca quien copia la funcion de al lado:

```text
   1. el tipo es 5 (Isoch), no 1 (Normal)
   2. lleva FRAME ID: en que microtrama quiere el aparato estos bytes
   3. lleva SIA -- *Start Isoch ASAP*
   4. TBC/TLBC a cero, y cero no es "no aplica": es el valor
```

★ **Se usa `SIA` y no un frame id calculado**, y es una decision con precio:
calcularlo exige leer el `MFINDEX` y acertar antes de que el reloj avance, y
fallar el numero se **oye como un clic**. El maestro ya eligio por escrito:

> *"Primero que suene sin huecos. Un audio puntual con 40 ms de retardo es audio;
> uno con 5 ms y clics, no."*

** Y trae los dos contadores que el maestro pedia: `isoch_encoladas` --que tiene
que subir sola-- y **`isoch_tarde`**, que se apunta en el bucle de eventos
cuando el xHC contesta `Missed Service Error` (CC 10) o `Isoch Buffer Overrun`
(CC 31).

> **`tramas tarde` es la cifra de toda la pagina de audio.** Un audio que va bien
> y uno que chasquea se distinguen por ese contador y por nada mas -- a oido son
> *"suena raro"* y *"suena bien"*, que no es un diagnostico.

## [X] A3 -- WAV -- **HECHO el 25-08**, `platform/shared/bmo-sonido`

** Y el fichero que esta maquina va a tocar el dia que emita una muestra ya
esta en el arbol, con su formato escrito: [`activos/sonido/LEEME.md`](../../activos/sonido/LEEME.md).

12 pruebas. Y **no es un formato de audio**: es PCM en un sobre, o sea
exactamente lo que come el endpoint. Cero decodificador.

### *** El fallo que este crate existe para impedir

Se lee mucho que *"un WAV son 44 bytes de cabecera"*. Es cierto en el caso comun
y **falso en general**: RIFF es una lista de trozos, y entre `fmt ` y `data`
puede haber un `LIST` con el titulo de la cancion.

> Un lector que salte 44 bytes a ciegas **entrega los metadatos como si fueran
> muestras**. Y eso no da un error: **da ruido blanco a todo volumen, en un
> oido.**

Por eso recorre los trozos, cuenta el byte de relleno de los impares, y comprueba
en `u64` que cada largo quepa -- que lo escribe el fichero, o sea otro.

### Y no convierte nada, a proposito

`cabe_en` contesta **si** o **no** con el motivo, y **no existe un `Casi`**: esa
variante habria sido la puerta por la que entra el resampler, que la parte 8 del
maestro rechaza por escrito.

## [X] A4 -- el bufer prestado -- **HECHO el 25-08, ANTES de MP3**

La parte 4 del maestro avisaba: *"si esto se hace en el paso 3, nace bien. Si se
hace despues, **hay que deshacer una copia**"*. Se hizo antes.

```text
   MAL   `audio_escribir(&muestras)` -> el kernel copia 192 bytes a su anillo.
         Mil veces por segundo, mil cruces de puerta y mil copias
   BIEN  la app pide un bloque, lo llena de PCM y lo OFRECE.
         **La app escribe donde el aparato va a leer**
```

### *** Y AQUI HAY ALGO QUE SOLO SE VE DESPUES DE SMAP

Desde el 25-08 Ring 0 **no puede tocar memoria de Ring 3**. Un diseno que hiciera
al kernel **leer** las muestras del bufer de la app estaria muerto desde esa
misma manana: `#PF` en la primera trama.

★★ **Este no lee nada.** El TRB isocrono lleva una direccion **FISICA**, y quien
va a buscar los bytes es **el xHC por DMA** -- no el CPU. El kernel solo traduce
una VA a su fisica **una vez**, al ofrecer.

> **El que lee no es el CPU, asi que SMAP no tiene nada que decir.**

[!] Y lo hace posible que `KIND_MEMORIA` entregue marcos **contiguos**: `Bloque`
guarda una `fisica` y los bytes van seguidos detras. Con paginas sueltas haria
falta un TRB por pagina y el corte no caeria en la frontera de una trama.

### Los dos numeros que cruzan, y ninguno mas

```text
   escrito   lo mueve la APP:   "he llenado hasta aqui"
   leido     lo mueve el TUBO:  "voy por alla"
```

** `escrito` se comprueba contra el tamano del bloque, y el tamano **lo dice el
bloque, no la app**: preguntarselo a ella seria dejar que declare un tamano que
no tiene. Y `fisica_de` busca en **sus** bloques y en ninguno mas, que es lo que
impide ofrecer la memoria de otro.

### *** MEDIA TRAMA NO SE MANDA

Si hay menos bytes de los que pide una trama, **no se entrega lo que hay
rellenando el resto**: sale una trama de silencio y se cuenta. Inventar muestras
no se ve -- **se oye**.

### Y el tercer contador, que separa dos culpas

```text
   tarde    el xHC no llego a su cita       -> el problema es del BUS
   huecos   nadie escribio la trama         -> el problema es de la APP
```

★ Los dos se oyen igual: un clic. **Sin separarlos, un audio que chasquea manda a
mirar el driver cuando la mitad de las veces el que llega tarde es quien
produce.** Los dos salen en `audio`, con su etiqueta al lado.

### Y se suelta al morir

`revoke_all` suelta el prestamo. Sin eso, **el aparato seguiria leyendo por DMA
marcos de un proceso que ya no existe** -- que es peor que un fallo: es un ruido
que no para y que no tiene dueno a quien pedirle que pare.

## ⛔ A5 -- MP3, y **por que va el ultimo aunque sea lo que se pidio**

El dueno lo pidio por nombre, y la respuesta honesta es el orden, no un no.

```text
   un .wav   ->  PCM              ->  el endpoint
   un .mp3   ->  DECODER -> PCM   ->  el MISMO endpoint
```

★★ **El decodificador no toca nada de lo anterior: entrega PCM al mismo sitio.**
Por eso puede ir el ultimo sin que nada se rehaga -- y por eso ir primero seria
caro:

> Empezar por aqui dejaria **un decodificador en verde y sin ejecutar** mientras
> no hay donde soltar las muestras. Es la cicatriz de los nueve tests de coma
> flotante del frontend de C, otra vez.

**Que trae**: `minimp3` es **un solo fichero**, o sea unity build por diseno --
como la amalgamation de SQLite. Va en Ring 3.

⚠ **Y hay un bloqueante que hay que mirar antes de traerlo**: `minimp3` usa coma
flotante, y el frontend de C de esta casa ya tiene historia ahi. Comprobarlo es
media tarde y se hace **antes** de traer 2.000 lineas, no despues.

[!] `bmo-sonido` ya **reconoce** un MP3 y contesta que no sabe tocarlo. Eso no es
soporte: es la diferencia entre *"esto no es audio"* y *"esto es un MP3 y aqui
todavia no hay decodificador"* -- dos respuestas que mandan a sitios distintos.

---

# 2. EL ORDEN, y por que es ese

```
   [X] A0  el aparato dice quien es    hecho y cableado. FALTA LA FOTO
   [X] A2  el TRB isocrono             escrito, sin ejecutar
   [X] A3  WAV                         hecho, 12 pruebas
   --------------------------------------------------------------------------
   [ ] A1  SET_INTERFACE               EL METAL LO NEGO; corregido 26-08
   [X] A2b el silencio, a peticion     hecho, sin ejecutar
   [X] A4  el bufer prestado           hecho ANTES de MP3, que era el aviso
   --------------------------------------------------------------------------
   A5  MP3                             lo unico que queda, y sin rehacer nada
```

★★ **Lo que separa hoy de que suene ya no es codigo: es un ARRANQUE.** Todo el
camino --descriptor, endpoint, alt, frecuencia, TRB isocrono y el bucle que
alimenta-- esta escrito y **nada de ello ha corrido nunca**. El primer numero que
lo dira es `encoladas` subiendo con `tarde` en cero.

★ **A1 va primero de lo que queda y no es discutible**: con A0, A2 y A3 hechos,
lo unico que impide que salga un sonido es que **nadie le ha dicho al aparato que
se ponga en el alt que trae el endpoint.**

---

# 3. LA PRUEBA QUE PIDIO EL DUENO, y en que orden llega

```text
   1. el silencio        ceros en bucle. EL SILENCIO NO PUEDE SONAR MAL
   2. un tono            una onda generada, sin fichero
   3. un .wav            leer y dar. Cero decodificador
   4. un .mp3            y aqui ya no hay nada nuevo que inventar
```

★★ **El 1 es el equivalente de `net rx`**: si el endpoint no se atasca y
`isoch_encoladas` sube sola con `isoch_tarde` en cero, **el tubo esta vivo** sin
haber arriesgado un solo ruido raro en los oidos del dueno.

---

## [X] A6 -- EL AUDIO SE INTEGRA: reclamado, el volumen por el hilo del bus, y el `save` lo cuenta (2026-09-21)

Lo que el `save` de las 13:52 enseno y lo que se hizo con ello, en tres piezas:

1. **El audifono no se ENCONTRABA desde el 17-09.** Ese dia lo no adoptado
   empezo a configurarse y a DEVOLVER su ranura; `uaudio::buscar()` seguia
   recorriendo ranuras 1..8 y `get_config_descriptor` contestaba `no ep0
   ring` en todas. Y buscarlo costaba **244 ms con las interrupciones
   cerradas** (`latido tarde 244 ms`, `3 ticks`, `tid 6 = musica.ibx`): era
   un syscall (`AUDIO_OP_DEVICES`, `IF=0` por `SFMASK`) mandando
   transferencias bloqueantes, un segundo conductor del xHC fuera del hilo
   del bus. Ahora **el que enumera lo ofrece** con el descriptor en la mano
   (`XhciHal::reclamar` -> `uaudio::reclamar`): si es USB Audio con
   volumen, se queda con su ranura viva (`VEREDICTO_RECLAMADO`, ficha 12),
   se le lee el rango y se guarda su interfaz de reproduccion. `devices()`
   es una lectura de un atomico; `censar` abre el tubo con lo guardado.
   `MAX_CFG` 512 -> 1024, porque un 7.1 se sale de 512, y la ficha "sin
   papeles" dice en que PASO se quedo y cuanto declaro medir.
2. **El volumen va por el hilo del bus.** `AUDIO_OP_VOLUME` DEJA DICHO el
   porcentaje (`pedir_volumen`) y `pump_bus` lo manda en su vuelta
   (`atender`): el syscall no toca el xHC. De paso: un volumen pedido antes
   de enchufar el audifono se aplica al reclamarlo, y al volver a
   enchufarlo se restaura el ultimo. `confirmar` guarda lo que el aparato
   dijo tener y si coincide.
3. **El `save` tiene seccion de audio** (`consumo`, entre usb y prestamos):
   aparatos, la ranura del reclamado, canales, mute, reproduce, rango en dB,
   volumen mandado / lo que vale / lo que tiene / confirmado / pedido, el
   dueno, y el tubo (abierto, armado, frecuencia, trama, max packet,
   encoladas, tarde, huecos, vetos DMA, pendientes). `INFO_AUDIO_*`
   0x82-0x87, sin handle. Todo por `fila`: DATOS.TXT lo lleva.

4. **Y el tubo tambien lo abre el hilo del bus** (misma tarde, a peticion
   del dueno: *"dale con `abrir` por el hilo del bus tambien"*). `censar`
   (el comando `audio` / `op_aparato`) ya no toca el xHC: lee lo reclamado y
   PIDE el tubo (`pedir_tubo`); `pump_bus` lo abre en su vuelta
   (`atender_tubo`), una vez. Y **al reclamar el audifono se pide solo**, asi
   que el tubo esta abierto ANTES de que `musica.ibx` pregunte `tubo(0)`:
   hasta hoy solo lo abria el comando `audio`, y musica sin ese comando
   caia al altavoz, que en esta placa no suena. Al desenchufar, `cerrar`:
   el tubo se cierra con la ranura todavia viva, y `latido` deja de encolar
   tramas a un endpoint muerto (antes nadie lo cerraba). `TUBO` pasa de
   `[escribe] ambos` a `bombeo`: ya no hay dos conductores del xHC en el
   audio. Ninguno.

---

## [X] A7 -- EL PAQUETE DEL EP0, y el pitido que dormia mal (2026-09-21, noche)

El save de las 19:45 dijo `sin papeles (ni el descriptor del aparato)` para
el puerto 1, y eso apunto a lo que faltaba en la enumeracion desde el
principio: **el `Evaluate Context`**. `address_device` supone un paquete de
8 bytes para un aparato Full Speed y se pedian los 18 bytes del descriptor de
golpe. Teclados y ratones tienen paquete de 8 y contestan en tres; un
audifono USB Audio suele declarar 64 y contesta en UNO, que el xHC rechaza
como Babble. Ahora los dos caminos piden 8, leen el byte 7 y, si no
coincide, `bmo_xhci::evaluar_mps0` (input context copiado del de salida, A1,
`TRB_EVAL_CTX`) antes de los 18; la ficha lleva el `cc`. Y el mismo save
enseno que el `latido tarde 244 ms` de las 13:52 era el PITIDO girando en
el syscall, no `buscar()`: `AUDIO_OP_BEEP` ahora duerme con `wait_current`.
Sin metal.

## [X] A8 -- EL ESQUEMA DE WINDOWS, ENTERO (2026-09-21, noche; *"mata el viejo y usa el nuevo"*)

Los dos caminos (`bmo_xhci::address_device` de una pieza, y `pasos.rs` uno
por bombeo) hacen ahora la secuencia de 6.1: Enable Slot; `Address Device`
con **BSR = 1** (la ranura en `Default`, el aparato en la direccion 0, EP0
con paquete supuesto **64** para Full Speed); `GET_DESCRIPTOR(64)` en la
direccion 0 (un aparato de 8 contesta 8 y para: paquete corto, legal; uno de
64, los 18); byte 7 y `Evaluate Context` si no coincide; **segundo reset**
del puerto; `Reset Device` (xHCI 4.6.11, `TRB_RESET_DEV`, para que el xHC
sepa del reset); `Address Device` con BSR = 0; 10 ms para asentar; y
entonces los 18, la cabecera y la configuracion. En `pasos.rs` son nueve
pasos mas (`Reset2`, `Reseteando2`, `Recuperando2`, `ResetDevice`,
`EsperandoResetDevice`, `Direccionar2`, `EsperandoDireccion2`, `Asentando`).
Las pruebas: un teclado de paquete 8 pasa por `address0, get_dev, evaluate,
reset, reset_device, address, get_dev, ...` (12 hechos, en 260-340 ms de
plazos); el de paquete 64 entra SIN evaluate y sin Babble. El esquema viejo
(`mps0_supuesto` 8 para Full Speed y los descriptores tras el SET_ADDRESS)
esta retirado, no aparcado. Sin metal.

# 6. EL ADN: lo que hacen Windows y Linux, contra lo nuestro (2026-09-21, noche)

El dueno, tras el save de las 19:45: *"que tal si estudiar como se hizo
Windows el driver generico, y lo mismo con Linux, para tener ese ADN"*. La
sospecha es correcta, y el `Evaluate Context` que faltaba (A7) es la prueba:
no era un invento, era un paso que los dos anfitriones dan desde hace veinte
anios y aqui no estaba. Lo que sigue es ese ADN escrito como LISTA contra el
codigo de BMO-X. Se lee, no se copia: Linux es GPL y este repo es Apache-2.0,
y Ring 0 esta cerrado a codigo de terceros. La secuencia y los numeros son
del protocolo, no de nadie.

## 6.1 -- Enumerar: la secuencia contra la que los aparatos se PRUEBAN

Un aparato USB se prueba en la fabrica contra Windows. Por eso Linux, en
`drivers/usb/core/hub.c` (`hub_port_init`), tiene DOS esquemas y el que usa
por defecto en USB 2 es el que imita a Windows (el comentario del fichero lo
dice: los aparatos "solo funcionan con el esquema de Windows"):

```text
  ESQUEMA NUEVO (Windows, y Linux por defecto en USB 2)
    reset del puerto
    EP0 con paquete SUPUESTO de 64 (Full Speed), 8 (Low), 64 (High), 512 (Super)
    GET_DESCRIPTOR(aparato, 64 bytes) EN LA DIRECCION 0   <- sin SET_ADDRESS
       un aparato de paquete 8 contesta 8 y para (paquete CORTO: legal)
       uno de 64 contesta los 18 en uno (cabe en 64)
       -> del byte 7 sale bMaxPacketSize0; se reajusta el EP0
    reset del puerto OTRA VEZ
    SET_ADDRESS, y 10 ms para que asiente
    GET_DESCRIPTOR(aparato, 18)
    GET_DESCRIPTOR(configuracion, 9) y despues entera
    SET_CONFIGURATION

  ESQUEMA VIEJO (Linux en USB 3, y como reserva si el nuevo falla)
    reset, SET_ADDRESS, GET_DESCRIPTOR(8) con paquete supuesto 8,
    reajustar el EP0 si el byte 7 no coincide, GET_DESCRIPTOR(18)
```

Los reintentos y los tiempos (`hub.c`): `GET_DESCRIPTOR_TRIES` 2 con 100 ms
entre ellos, `GET_MAXPACKET0_TRIES` 3, `SET_ADDRESS_TRIES` 2, `PORT_RESET_TRIES`
5, `HUB_ROOT_RESET_TIME` 60 ms, debounce de 100 ms estables sobre un plazo de
2 s, `TRSTRCY` 10 ms tras el reset, `USB_QUIRK_DELAY_INIT` para los que piden
mas. Y si el esquema nuevo falla dos veces, prueba el viejo (y al reves con
`old_scheme_first`). Sobre xHCI, "direccion 0" se hace con `Address Device`
con **BSR = 1** (Block Set Address Request, xHCI 4.3.4): el xHC deja la
ranura en `Default` sin mandar `SET_ADDRESS`, y despues un segundo `Address
Device` con BSR = 0. El reajuste del EP0 es `usb_ep0_reinit` ->
`xhci_check_maxpacket` -> `Evaluate Context`.

**BMO-X hoy** (tras A8, la misma noche): el esquema NUEVO, entero, en los
dos caminos. El viejo se retiro. Lo que sigue sin tener: alternar esquemas
entre reintentos (Linux lo hace; aqui el segundo intento corta la corriente,
que es otra medicina) y una tabla de quirks (`USB_QUIRK_DELAY_INIT`,
aparatos que necesitan mas tras `SET_ADDRESS`).

## 6.2 -- Reproducir: lo que hace `snd-usb-audio` (Linux) / `usbaudio.sys` (Windows)

| que | Linux / Windows | BMO-X hoy |
|---|---|---|
| encontrar el aparato | el nucleo enumera y OFRECE al driver de clase por (clase, subclase); el driver dice "es mio" | `reclamar` (21-09): la misma forma |
| elegir formato | recorre TODOS los alt settings de AudioStreaming, elige por (PCM, bits, canales, frecuencia); UAC1 y UAC2 | `find_playback`: UAC1, el primero que sirve; **UAC2 no** (un 7.1 High Speed seria UAC2) |
| tamano de paquete | `bytes_per_interval` con FRACCION acumulada: 44.100 Hz son 44 y 45 muestras alternando (`phase`); sin eso el reloj deriva y se oye | `rate / 1000 * canales * subframe`: exacto a 48.000, TRUNCA a 44.100 |
| cuantas tramas en vuelo | 8-12 URBs de varios paquetes cada uno; el anillo nunca se vacia | el latido encola las de 4 ms; `huecos` cuenta cuando no llega |
| sincronia | endpoint de FEEDBACK (asincrono): el aparato dice cuantas muestras quiere por trama y el host ajusta | `Sync` se PARSEA y no se usa: si el aparato es asincrono, deriva |
| volumen | Feature Unit: GET_MIN/MAX/RES y CUR; mute aparte; por canal si el maestro no vale | igual (`uaudio.rs`) |
| rarezas | tabla de quirks por vid:pid de cientos de filas (retrasos tras SET_INTERFACE, aparatos que mienten el rango, que no aceptan GET_MIN...) | ninguna; el `save` dice el vid:pid y el cc, que es por donde empieza una tabla asi |

## 6.3 -- Que se toma de esto, y en que orden

1. Lo que el siguiente save diga del puerto 1 decide 6.1: si sigue mudo con
   el evaluate puesto, se hace el esquema nuevo (BSR = 1, 64 bytes en la
   direccion 0, segundo reset). Son tres pasos mas en `pasos.rs` y un
   `address_lanzar` con BSR.
2. La fraccion de 44.100 y UAC2 son de A1/A5: cuando SUENE a 48.000.
3. El feedback y los quirks, cuando haya un aparato que los pida: el save
   dira `tarde`/`huecos` con el nombre del aparato, y esa es la fila 1 de la
   tabla.

# 5. Y DESPUES: LA API DE ACCESORIOS EN RUST (idea del dueno, 21-09)

*"Construir una API ULTRA simplificada para los accesorios nuevos que quieran
programar con Rust."* Se apunta con su condicion, que es la regla del 17-09:
**Ring 0 esta cerrado a terceros**. Un accesorio de tercero no es un driver en
el kernel: es un programa de Ring 3 (o la antena) que habla con el kernel por
un contrato pequeno. Y ese contrato ya tiene TRES piezas hechas hoy, sin
nombre de API:

```text
   ver que hay          INFO_AUDIO_*, las fichas del portero      (OP_INFO, sin handle)
   tener derecho        AUDIO claim -> un dueno                    (capability)
   mover datos          el bufer PRESTADO (A4): dos indices, cero copias
```

Y una cuarta que es la forma que tendria el driver mismo: los **once verbos**
de `bmo_uhid::pasos::Metal` --lanzar, llego, devolver, plazo-- que es lo que
un driver necesita del bus y nada mas. La API "ultra simplificada" seria ESE
trait, publicado, con el kernel como unico `Metal` de verdad.

El dueno lo acoto asi (21-09): *"accesorios nuevos, para facilitar a los que
quieren meter algo raro y ya"*. O sea: no un SDK, una PUERTA. Lo que hoy ya
hace el kernel con el audifono es la forma exacta de esa puerta: el que
enumera OFRECE el aparato con sus papeles (`reclamar(slot, vid, pid, cfg)`),
alguien dice "es mio", y a partir de ahi habla con el por control transfers y
un endpoint. Para un tercero eso seria: un `.bex` de Ring 3 que declare
`vid:pid` (o clase) y reciba la oferta por el buzon, con los once verbos
como syscalls sobre SU ranura y nada mas. Cuando toque: despues de que el
audio suene en el metal, no antes.

# 4. LO QUE ESTE PLAN NO PROMETE

Las cinco de la parte 8 del maestro, sin cambiar ninguna: **resampleo**,
**mezclador de varias apps**, **grabar**, **HD Audio** y **latencia baja**.

★ Y la que mas cuesta aceptar es la ultima, asi que va con su frase: *"Primero
que suene sin huecos. Un audio puntual con 40 ms de retardo es audio; uno con 5
ms y clics, no."*
