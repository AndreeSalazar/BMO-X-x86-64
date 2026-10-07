# PLAN EL LIBRETO -- Cyberpunk 2077 entra por VERRANO

> Abierto el **2026-10-07**, despues de ver en la SALIDA de la corrida de
> 68 s que el primer lote de Cyberpunk llego a la puerta de la 3060 y el
> juez dijo que no (`SV_Position en el de pixeles`). El propietario: *"es
> logico que debia empezar en VERRANO para estar listo, PROTON-X
> simplemente prepara y la CPU prepara el libreto, y la GPU ya se prepara
> con todo eso, para arrancar con fuerza"*.
>
> Lo de antes, que este plan NO repite: la API y sus backends en
> [`PLAN_VERRANO.md`](PLAN_VERRANO.md) (la cara comun, VC1 a VC5, es su
> seccion 4); la lengua de la 3060 y el juez del SASS en
> [`PLAN_LA_LENGUA_DE_LA_3060.md`](PLAN_LA_LENGUA_DE_LA_3060.md); la pila
> de DX12 en [`PLAN_LAS_TRES_GRANDES.md`](PLAN_LAS_TRES_GRANDES.md); los
> nucleos y el reinicio de `smp all` en
> [`PLAN_LOS_DOCE_DIRECTORES.md`](PLAN_LOS_DOCE_DIRECTORES.md). Este plan
> dice POR QUE VERRANO va primero, QUE le falta a la 3060 para Cyberpunk y
> EN QUE ORDEN; las casillas que ya viven en otro plan se nombran, no se
> copian.

---

# 0. LA RESPUESTA CORTA: por que VERRANO primero

```text
   sin VERRANO                          con VERRANO
   DX12 -> 3060                         DX12   \
   DX11 -> 3060                         DX11    >  VERRANO  ->  3060
   Vulkan -> 3060                       Vulkan /            ->  la CPU (el juez)
   (y luego cada uno -> una AMD)                            ->  luego, una AMD
   = 3 x 2 traductores, 3 jueces        = 3 + 2 traductores, UN juez
```

Cuatro problemas que VERRANO evita, y por eso va delante:

1. **Una puerta, no muchas.** Cada cara (DX12, DX11, Vulkan) que hable
   con la 3060 por su cuenta es un traductor mas que mantener, y otro mas
   el dia que haya otra tarjeta. Con VERRANO en medio, una cara nueva o una
   tarjeta nueva es UNA pieza, no una por pareja.
2. **El juez en un solo sitio.** La 3060 no se queja: OBEDECE. Un programa
   mal hecho no da error, la cuelga (el Xid 69 del 28-09, VERRANO V0
   colgado en los vertices el 25-09). Todo lo que va a la tarjeta tiene que
   pasar por la misma PUERTA y por el mismo juez antes de que la tarjeta lo
   vea.
3. **La CPU de reserva en un solo sitio.** Lo que la 3060 aun no sabe
   hacer lo pinta la CPU (la trama, que es ademas el juez de las huellas).
   Hoy se decide lote a lote, en el momento de dibujar. Con el libreto se
   decide ANTES, una vez.
4. **Preparar una vez.** Cyberpunk crea sus 1065 PSO en ~1,1 s al cargar.
   Ese es el momento de traducir, juzgar y guardar TODO -- no el primer
   dibujo de cada uno, que es lo que da tirones.

*** Y LO QUE VERRANO NO PIDE, dicho para no bloquear lo que no hace falta:
**la primera imagen de la intro** la puede pintar la CPU (lento pero bien);
**el sonido** ya se abrio; **el reinicio de `smp all`** es del kernel, no de
la 3060. VERRANO es lo que hace falta para JUGAR a buena velocidad, no para
ver el primer cuadro.

---

# 1. LOS CUATRO PAPELES (el modelo del propietario)

```text
   PROTON-X, el ESPIA     lee lo que el juego pide -- cada PSO, cada recurso,
                          cada lista de ordenes -- y lo DICE en VERRANO. No
                          dibuja el mismo
   la CPU, el LIBRETO     al crear cada PSO: lo traduce UNA vez a sus dos
                          destinos (.bex para la CPU, .bsf para la 3060), lo
                          juzga, y apunta quien lo pinta. Al grabar las
                          listas: decide por PASE (no por lote) que va a la
                          3060 y que a la CPU, y lo escribe como un libreto
   la GPU, el ESCENARIO   recibe el paquete entero ya juzgado: programas en
                          su memoria, recursos en su VRAM. De un fotograma a
                          otro solo cambian los DATOS (constantes, vertices)
   el kernel, el TECNICO  prepara (memoria, IOMMU, el canal, el GSP) y se
                          aparta: no esta en el camino de cada dibujo (LI5)
```

---

# 2. LO QUE DIJO EL METAL (la corrida de 68 s, 07-10)

```text
   que                      el numero                     que significa
   PSO                      1065 creados en ~1,1 s;       el lado de la CPU YA
                            143 enlaces, 141 se corren,   recuerda: casi nada se
                            2 compilados, 141 del         compila dos veces
                            recuerdo
   recursos y ordenes       236 recursos, 43              monta el renderizador
                            ExecuteCommandLists
   Present                  0                             aun no muestra un cuadro
   el GSP de la 3060        despierto (la 570.144)        la tarjeta estaba lista
   el primer lote           "va por la CPU: el PSO no     la puerta funciona: dijo
                            va a la 3060: SV_Position     NO a lo que no sabe, y la
                            en el de pixeles"             CPU lo pinto
   el kernel por lote       264 us contra 36 us de la     el cuello de botella es
   (29-09, bmox12)          tarjeta                       el kernel (LI5)
```

**Lo que NO se sabe todavia, y es lo primero (LI0):** solo se vio la
PRIMERA puerta que toco Cyberpunk. No se sabe cuantos de los 143 enlaces
pasarian ni que puerta pesa mas.

---

# 3. LAS PUERTAS DE LA 3060 HOY (sacadas del codigo, no de memoria)

Lo que hoy manda un PSO a la CPU (`platform/shared/proton-x-sm86/src/pso.rs`
y `puerta.rs`; el juez en `platform/drivers/gpu/ga10x/src/sass/juez.rs`):

| la puerta | donde | lo que pide Cyberpunk (a medir en LI0) |
|---|---|---|
| un sombreador de geometria | `pso.rs:132` | pocos, en un motor moderno |
| un valor de sistema en el de vertice (`SV_VertexID`, `SV_InstanceID`) | `pso.rs:115` | probable: dibujos sin bufer, instancias |
| un formato de vertice que no es float de 32 bits | `pso.rs:121` | muy probable: normales y colores en 8 o 16 bits |
| `SV_Position` en el de pixeles | `pso.rs:151` | **SI: el primer lote** |
| varios render targets, o uno que no es el 0, o `SV_Depth` | `pso.rs:155` | **seguro: el G-buffer de un motor diferido** |
| mas de 128 instrucciones (juez R5) o mas de 64 registros | `juez.rs:853` | **la puerta mas dura**: un sombreador de Cyberpunk tiene cientos o miles |
| leer una textura (`TEX`) | `muestreo.rs`: el TIC y el TSC estan, falta la instruccion | **seguro: casi todo el de pixeles** |
| la division de coma flotante | `lib.rs`: se rechaza (la 3060 aproxima) | probable |
| el computo del juego (`Dispatch`) | la 3060 corre computo fijo (`trabajos/blur.rs`), no el de un PSO | **seguro**: posproceso, luces, particulas |
| un PSO por receta y el kernel en cada lote | `puerta.rs`, `gpu_trabajo/cubo.rs` | todo: es la velocidad |

---

# 4. LAS CASILLAS

## 4.0 Medir antes de construir

- [ ] **LI0 -- EL CENSO de la 3060: cuantos PSO de Cyberpunk pasarian, y
      por que no los demas.** Al crear cada PSO, PROTON-X le pregunta a la
      puerta SIN mandar nada a la tarjeta (la traduccion y el juez, en la
      CPU) y apunta TODAS las puertas que no pasa, no solo la primera. Al
      cerrar, una linea en el DIARIO: `# la 3060: de N PSO, P irian; por la
      CPU: SV_Position a, varios RT b, TEX c, largo d, ...` y los 5 PSO mas
      usados con su motivo. Sin Ring 0 y sin la 3060 (corre aunque el GSP
      no despierte). **Como se sabe:** en el banco, los PSO de `prueba/`
      con un censo conocido (uno que pasa, uno por cada puerta: la prueba
      del NO es que cada puerta se cuente); en el metal, la linea del
      DIARIO de Cyberpunk. **Decide el orden de LI2.**

## 4.1 El libreto de los PSO (preparar una vez)

- [ ] **LI1 -- traducir y juzgar al CREAR, no al dibujar.** Es VC1 de
      [`PLAN_VERRANO.md`](PLAN_VERRANO.md) llevado hasta el final: el .bsf
      vivo ya recuerda en ESTRATOS (A9) y el lado de la CPU ya recuerda sus
      enlaces (141 de 143). Falta: hacerlo en `CreateGraphicsPipelineState`
      (dentro de los ~1,1 s de carga, repartido entre los obreros cuando
      `smp all` vuelva), para los 1065; el x86-64 de `nativo` recordado
      tambien; y la decision "3060 o CPU" escrita en el paquete. **Como se
      sabe:** la segunda corrida de Cyberpunk traduce CERO programas (un
      contador en el DIARIO) y su primer dibujo no compila nada.

## 4.2 Abrir las puertas (el orden lo pone LI0; este es el probable)

- [ ] **LI2a -- `SV_Position` en el de pixeles** (N5.9). La 3060 la da en
      un atributo de sistema que el pegamento aun no pone. Es la primera
      que toco Cyberpunk y es acotada. **Como se sabe:** `posicion.dxil`
      (de `prueba/`) pasa la puerta y da los mismos bits en el simulador de
      la 3060 que en la CPU; en el metal, la huella de `bmox12` igual.
- [ ] **LI2b -- programas largos.** Hoy el cuerpo cabe en un hueco de 128
      instrucciones de la tuberia. Un sombreador de Cyberpunk no. El codigo
      tiene que vivir en su propia region de la VRAM (como hace el driver
      de NVIDIA), con el juez leyendo el programa entero, y los registros
      repartidos con un planificador de verdad (hoy 64). **Como se sabe:**
      un programa de 2000 instrucciones del banco da los mismos bits que la
      CPU, juzgado entero.
- [ ] **LI2c -- texturas** (`TEX`; M3 de `PLAN_VERRANO.md`). El TIC y el
      TSC ya se escriben (`muestreo.rs`, T0); falta la instruccion, el pool
      de descriptores en la VRAM y subir la textura. **Como se sabe:** el
      muestreo puntual da los bits de la CPU; el lineal, dentro de la
      tolerancia que diga LI2g.
- [ ] **LI2d -- varios render targets y la profundidad** (N5.8; V2/M1 de
      `PLAN_VERRANO.md`). El G-buffer: hasta 8 destinos y el z-buffer en la
      VRAM. **Como se sabe:** la escena 3D dura (A11) pintada por la 3060
      con la huella de Windows.
- [ ] **LI2e -- la entrada de vertices de verdad**: formatos de 8 y 16 bits
      (UNORM, SNORM, half) y `SV_VertexID` / `SV_InstanceID`. **Como se
      sabe:** las instancias de N5.19 por la 3060, con su huella.
- [ ] **LI2f -- el computo del juego en la 3060.** Un PSO de computo al QMD
      (la 3060 ya corre computo fijo: `trabajos/blur.rs`), con UAV. **Como
      se sabe:** `olas` (A10) por la 3060, con su huella.
- [ ] **LI2g -- DECISION DEL PROPIETARIO: los bits exactos o la tolerancia
      de D3D.** La regla de la casa es "los mismos bits que la CPU". La 3060
      APROXIMA la division, la raiz y el filtrado; D3D lo permite (unos
      ULP). Para un juego: o se acepta la tolerancia de la especificacion en
      esas operaciones (y el juez mide en ULP), o esas operaciones van por
      la CPU para siempre. **Como se sabe:** la decision escrita aqui, y el
      juez con la regla elegida y su prueba del NO.
- [ ] **LI2h -- el sombreador de geometria**, si LI0 dice que Cyberpunk lo
      usa. Si no, se aparca con motivo.

## 4.3 PROTON-X habla VERRANO

- [ ] **LI3 -- el fotograma por pases, no por lotes** (es VC2 de
      `PLAN_VERRANO.md`). `Draw` y `Dispatch` se vuelven fotogramas de
      VERRANO; la trama queda como el backend CPU. **La regla nueva, la
      dificil: un fotograma MIXTO cuesta.** Si un pase va por la CPU en
      medio de pases de la 3060, su render target tiene que ir de la VRAM a
      la RAM y volver: eso puede costar mas que pintarlo. El libreto decide
      por PASE (un render target y lo que lo lee), con el censo de LI0 y lo
      que cuesta cada viaje medido. **Como se sabe:** todos los jueces de
      `prueba/` (hdr, stencil, olas...) dicen lo mismo por la puerta nueva;
      y en el metal, los viajes VRAM <-> RAM de un fotograma, contados en el
      DIARIO.

## 4.4 El libreto del fotograma

- [ ] **LI4 -- grabar una vez, repetir con datos nuevos.** Un juego repite
      casi las mismas listas de ordenes cada fotograma. La primera vez la
      CPU las graba como libreto de VERRANO (que pase, que programa, que
      recursos) y lo deja en la memoria de la 3060; los fotogramas
      siguientes solo suben lo que cambio (constantes, la camara) y dicen
      "otra vez". Es la idea de los *bundles* de D3D12 y de las listas que
      se reusan, hecha por la casa. **Como se sabe:** en `bmox12`, el
      coste de la CPU por fotograma cae (medido) y la huella no cambia.

## 4.5 El kernel fuera del camino

- [ ] **LI5 -- el canal de la 3060 en manos de la app** (es VC4 de
      `PLAN_VERRANO.md`). El GPFIFO y el USERD en memoria de la app detras
      de la IOMMU, y el timbre tocado desde Ring 3; el kernel lo crea por
      RPC al GSP-RM y no vuelve a entrar por dibujo. **Es Ring 0: se decide
      con el propietario.** Pide antes el vigilante (E7: sin el kernel en
      medio, un programa que cuelga la 3060 no lo para nadie) y el GSP que
      despierta siempre (G0; el 07-10 una sesion dio `motivo =44`).
      **Como se sabe:** el coste del kernel por lote cae de ~264 us a casi
      cero, y los fps de `bmox12` suben.

## 4.6 Presentar

- [ ] **LI6 -- el Present en la 3060** (P1 de `PLAN_VERRANO.md`: dos
      superficies y el cambio en el VBLANK). La imagen no vuelve a la RAM
      para verse: la 3060 la muestra. **Como se sabe:** la intro de
      Cyberpunk en la pantalla con su contador de fps en el DIARIO.

---

# 5. EL ORDEN, con lo que ya esta en marcha

```text
   1. el reinicio de `smp all`      V7b + V8: el siguiente CAIDA dice el paso.
                                    Sin esto, ni el censo se reparte
   2. el primer Present POR LA CPU  la intro, lenta pero correcta: prueba que
                                    PROTON-X entiende el fotograma entero
   3. LI0 el censo                  en paralelo con 2: barato, sin Ring 0, sin
                                    la 3060. DECIDE el orden de LI2
   4. LI1 + LI2 (en el orden de LI0) cada puerta abierta es un % de PSO que
                                    deja la CPU, medido
   5. LI3 los pases, LI4 el libreto ahora si: VERRANO entre PROTON-X y la 3060
   6. LI5 el kernel fuera           el ultimo: Ring 0, y pide E7 y G0
   7. LI6 el Present en la 3060
```

*** La razon de ese orden: **cada paso se puede medir solo.** El censo dice
cuanto vale cada puerta antes de abrirla; cada puerta abierta se ve en el
censo siguiente; y lo que toca Ring 0 va al final, cuando todo lo de encima
ya esta probado.

---

# 6. LO QUE PUEDE FALLAR, dicho antes

- **Los programas largos son la puerta mas dura.** LI2b no es un arreglo:
  es pasar de "programas de 64 instrucciones a mano" a "programas de miles
  con un planificador". Si LI0 dice que casi todo es largo, LI2b va primero
  aunque cueste mas.
- **Un fotograma mixto puede ser mas lento que todo por la CPU.** Por eso
  LI3 decide por pase y mide los viajes, y no manda a la 3060 "lo que se
  pueda" sin mirar lo que cuesta volver.
- **La 3060 aproxima.** Si se elige la regla de los bits exactos (LI2g),
  parte del juego se queda en la CPU para siempre. Es una decision, no un
  fallo.
- **Miles de PSO**: la primera partida traduce y juzga todo (tirones al
  cargar, no al jugar); la segunda, nada. Si el traductor cambia, la cache
  vieja no vale: la version va en la clave y se borra sola.
- **El GSP no siempre despierta** (`motivo =44` el 07-10). Mientras pase,
  todo cae a la CPU: lento pero correcto, nunca colgado.
- **El paquete de un juego comprado es SUYO y de esta maquina**: no se
  reparte.

---

# 7. Y LA AMD (porque seria mas facil, y no se pierde nada)

El objetivo del principio era una **RX 9060 XT (RDNA 4)** con Vulkan
([`PLAN_VULKAN.md`](../../platform/drivers/gpu/rdna4/PLAN_VULKAN.md)). Con
una AMD seria mas facil por tres cosas:

| | la 3060 (NVIDIA) | una AMD (RDNA) |
|---|---|---|
| el manual de las instrucciones | no hay: se saca de Mesa (NAK) y del metal | AMD publica el manual de la ISA |
| arrancar la tarjeta | firmware FIRMADO de NVIDIA, el booter y el GSP-RM (60 MiB): un driver de NVIDIA corriendo DENTRO de la tarjeta | tambien firmado (lo carga el PSP: [`PSP_MEDIDO.md`](../../platform/drivers/gpu/rdna4/PSP_MEDIDO.md)), pero el driver abierto (amdgpu) programa la tarjeta directo: no hay otro driver dentro que despertar |
| un driver abierto de referencia | NAK/NVK, joven | RADV y ACO, maduros |

**Y no se tira nada:** esa es justo otra razon para VERRANO. Con la cara
comun en medio, una AMD el dia que llegue es UN backend nuevo debajo de
VERRANO: PROTON-X, el libreto, el censo y el juez de las huellas de la CPU
se quedan como estan. Lo aprendido con la 3060 (el juez del SASS, el .bsf
vivo, la libreta) es la forma; con la AMD cambian los numeros.

---

# 8. ILLAPA, LA ESCUELA DE VERRANO (pedido del propietario, 07-10)

> El propietario, tras subir [`PLAN_ILLAPA.md`](PLAN_ILLAPA.md): *"eso
> ayudaria educar mi VERRANO poco a poco, no?"*. **Si**, y por una razon
> concreta: con Cyberpunk, el JUEGO elige los sombreadores (de miles de
> instrucciones, con todo a la vez); con ILLAPA, **BMO-X los escribe** en
> TITAN++ (`gpu fn`), de la medida que se quiera, uno nuevo por cada cosa que
> se aprende, con el oraculo en el anfitrion. **ILLAPA es la escuela;
> Cyberpunk, el examen.**

## Lo que una casilla de ILLAPA le muestra a VERRANO, y la de aqui que abre

| ILLAPA pide | lo aprende VERRANO | y abre en Cyberpunk |
|---|---|---|
| IL1 `gpu fn` con vecinos y bucles | saltos y bucles en la 3060 (E6 de [`PLAN_LA_LENGUA_DE_LA_3060.md`](PLAN_LA_LENGUA_DE_LA_3060.md)), programas mas largos | **LI2b**, la puerta mas dura |
| IL2-IL4 el solucionador (la rejilla, PBF, lo unificado) | el COMPUTO de verdad en la 3060 (G4 de [`PLAN_EL_CENTAURO.md`](PLAN_EL_CENTAURO.md)), buferes que se leen y escriben (UAV), un pase de computo detras de otro | **LI2f**, el computo del juego |
| IL7 VERRANO en 3D | profundidad, constantes, texturas y MEZCLA | **LI2c**, **LI2d**, y la mezcla (que Cyberpunk tambien pide) |
| LZ1 HDR y el tono | pintar en coma flotante | los render targets de float del juego |
| LZ2-LZ4 materiales, entorno, sombras | el G-buffer, mapas de cubo, mipmaps, varias pasadas | **LI2d**, **LI3** (los pases) |
| la regla de ILLAPA: "de un fotograma al siguiente solo cambian los DATOS" | la misma de VERRANO | **LI4**, el libreto del fotograma |
| D5 de ILLAPA (el `f32` en la CPU) y las aproximaciones de la 3060 | que cuenta como "el mismo resultado" | **LI2g**: es LA MISMA decision; conviene tomarla una vez para los dos |

## [!] El eslabon que falta para que la escuela sirva al examen

Hoy hay **un solo emisor de verdad** hacia la 3060: `proton-x-sm86`, y su
entrada NO es SPIR-V sino el `Programa` de la casa (E3, decision del
propietario del 28-09). Las `gpu fn` de TITAN++ salen como **SPIR-V**
(G2 de `PLAN_EL_CENTAURO.md`); del lado de SPIR-V, `emisor-sm86` solo tiene
el subconjunto (E1), no emite. O sea:

```text
   Cyberpunk:  DXIL / SM5  ->  Programa  ->  proton-x-sm86  ->  juez  ->  3060
   ILLAPA:     gpu fn      ->  SPIR-V    ->  ???
```

Si ILLAPA tuviera su PROPIO emisor de SPIR-V a la 3060, cada cosa se
aprenderia DOS veces (los bucles, las texturas, el computo) y la escuela no
le serviria de nada al examen. Con un lector de SPIR-V al `Programa`, todo lo
que aprenda ILLAPA cae en el MISMO emisor, el MISMO juez y el MISMO .bsf
vivo que usa Cyberpunk.

- [ ] **LI7 -- UNA SOLA LENGUA HACIA LA 3060: el SPIR-V de TITAN++ al
      `Programa` de la casa.** Un lector (el subconjunto de E1 manda lo
      que entra) que convierte el SPIR-V de una `gpu fn` en el `Programa`
      que ya corren el interprete, `nativo` y `proton-x-sm86`. Donde vive
      se decide al hacerlo: el lector de SPIR-V es NEUTRO (el guardian
      `isa`) y el `Programa` vive en `bmo-proton-x`; quiza el `Programa`
      tenga que salir a un sitio neutro primero. **Como se sabe:** las
      `gpu fn` del banco `nivel11` (`mezcla`, `activa`) pasan por SPIR-V ->
      `Programa` -> el emisor -> el simulador de la 3060 y dan los MISMOS
      bits que el oraculo de spirv; la prueba del NO, una `gpu fn` con algo
      fuera del subconjunto, rechazada con motivo y sin emitir. **Va antes
      que IL1**: si no, IL1 nace en un camino aparte.

## El orden de los dos planes juntos

```text
   ahora      smp all, el primer Present por la CPU, LI0 el censo
   la escuela LI7 (el eslabon) -> IL1 (bucles) -> IL2-IL4 (computo) -> IL7
              (3D): cada una con sus gpu fn chicas y el oraculo
   el examen  cada vez que la escuela abre algo, el censo de LI0 se repite
              con Cyberpunk: el % de PSO que deja la CPU dice cuanto sirvio
```
