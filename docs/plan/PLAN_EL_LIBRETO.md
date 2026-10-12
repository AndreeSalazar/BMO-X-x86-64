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

- [~] **LI0 -- EL CENSO de la 3060: cuantos PSO de Cyberpunk pasarian, y
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
      **12-10, HECHO en el anfitrion** (`proton-x-sm86/src/censo.rs`,
      `pruebas_censo.rs`: la prueba del NO es un PSO con DOS puertas, donde
      `traducir` dice una y el censo las dos). Cuenta tambien las puertas
      del LOTE (mezcla, Z, stencil, instancias, UAV, olas, derivadas, mips)
      y las del destino (HDR, sin pixeles), y "solo por ella". Encendido con
      el diario. **Falta:** la linea del DIARIO de Cyberpunk en el metal.

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

- [~] **LI2a -- `SV_Position` en el de pixeles** (N5.9). La 3060 la da en
      un atributo de sistema que el pegamento aun no pone. Es la primera
      que toco Cyberpunk y es acotada. **Como se sabe:** `posicion.dxil`
      (de `prueba/`) pasa la puerta y da los mismos bits en el simulador de
      la 3060 que en la CPU; en el metal, la huella de `bmox12` igual.
      **11-10, la mitad de VERRANO HECHA en el anfitrion:** el juez de
      VERRANO (`Draw`, V3a) corre el de pixeles en CADA pixel cuando lee su
      posicion, con la regla de la casa (N5.9: el centro, la z lineal en la
      pantalla, la w de recorte con perspectiva); `posicion.dxil` da lo
      mismo que la prueba de `bmo_proton_x`, y la w con perspectiva
      tambien (saboteada con la media de las w: cae). **Falta la mitad de
      la 3060**, y se dice lo que se sabe y lo que NO, antes de tocarla:
      ```text
         la cabecera     `ImapPositionW` es el bit 191 (raster.rs: nouveau
                         lo pone siempre); los de X, Y y Z, al lado, por
                         CONFIRMAR en NAK (`sph.rs`) -- desde aqui no se
                         llego a su fuente: no se adivina un bit de la SPH
         la lectura      IPA de a[0x70..0x7c] (la posicion, donde el de
                         vertice la escribe); x e y ya son el centro
         la w            la 3060 da 1/w; D3D quiere w: la division EXACTA
                         de la casa (`cociente.rs`, DL10), no un MUFU.RCP
         la z            su IPA contra la cuenta de la CPU: si no da los
                         mismos bits, lo dice el juez de `laminas` en el
                         metal, y se decide (como la raiz, DL10)
      ```
      Instrucciones nuevas para la tarjeta: van al metal con el permiso del
      propietario, como E6.
      **12-10, LA x Y LA y HECHAS** (con el permiso del propietario: *"empezar
      con eso"*). Los bits, de la fuente de NVIDIA y no adivinados: la tabla 9
      de `open-gpu-doc/Shader-Program-Header` pone `ImapSystemValuesB` tras
      CommonWord0..4 (160 bits) e `ImapSystemValuesA` (24): PositionX..W son
      los bits **188..191**. `Carga::Posicion` (pegamento, receta tipo 3,
      validada en Ring 0) pone el IPA de `a[0x70]`/`a[0x74]` y su bit; con el
      origen arriba y el centro en el medio (`raster.rs`, desplazamiento 0
      tambien en la pantalla directa) son `columna + 0.5` y `fila + 0.5`, los
      bits de la CPU. **La z y la w siguen por la CPU** (otro motivo en el
      censo: `SV_Position.z o .w en el de pixeles`): la w es 1/w en la 3060 y
      pide la division exacta; la z, su juicio en el metal. Pruebas:
      `la_x_y_la_y_van_a_la_3060` (el IPA y los bits en el programa pegado, y
      el juez) y `la_z_y_la_w_siguen_por_la_cpu`; `la_posicion_cruza_la_receta`.
      **Falta:** el metal (el censo dira cuantos lotes de Cyberpunk solo leian
      x e y).
- [~] **LI2b -- programas largos.** Hoy el cuerpo cabe en un hueco de 128
      instrucciones de la tuberia. Un sombreador de Cyberpunk no. El codigo
      tiene que vivir en su propia region de la VRAM (como hace el driver
      de NVIDIA), con el juez leyendo el programa entero, y los registros
      repartidos con un planificador de verdad (hoy 64). **Como se sabe:**
      un programa de 2000 instrucciones del banco da los mismos bits que la
      CPU, juzgado entero.
      **12-10, hecho en el anfitrion (el censo del metal: el de vertice de
      129 a 256, el de pixel hasta 4096):**
      - `vram::CODIGO`: 2 x 17 paginas de VRAM detras de los DATOS (libres
        hasta las tablas de GR), mapeadas con el tramo en la PT 336..369
        (`mapear_tramo` las escribe y las relee; el escritorio pregunta si
        son usables con `vram::MAPEADO_BYTES`).
      - `juez::MAX_INSTRUCCIONES` = 4096: el kernel juzga el programa
        ENTERO donde esta. `tuberia::VS/PS` en la region; las ordenes los
        buscan alli (`cubo::hasta_el_dibujo_de` recibe donde esta cada
        programa: el cubo de X5 sigue en sus huecos). `escribir_programa`
        pone a cero solo lo que queda de su ultima pagina.
      - El taller del kernel (estatico) mide 526 KiB; el fin del kernel
        queda en ~17,9 MB, dentro de los 32 MiB que mapea el arranque. En
        PROTON-X el `Pegado` (64 KiB) y el taller van en el monton, y el
        binario no tiene un marco de pila de mas de 19 KiB (medido).
      - Pruebas: `pruebas_largos.rs` (2000 instrucciones: los bits de la CPU
        en los dos ABI, el pegamento y el juez enteros, y el juez en vivo;
        lo de mas de 4096, por la CPU), `tuberia_vram.rs` (el largo en su
        region, el corto deja su pagina limpia, las ordenes apuntan alli).
      **Falta:** el metal (Cyberpunk con `smp all` y `personal 3060`); los
      registros siguen en 64 (lo que pasa de ahi lo dice el emisor); y un
      programa cambiado se reescribe ENTERO por PRAMIN en cada dibujo frio
      (hasta 64 KiB): si el metal lo dice lento, una cache de programas en
      la VRAM (cada uno subido UNA vez) es la pieza siguiente.
- [ ] **LI2c -- texturas** (`TEX`; M3 de `PLAN_VERRANO.md`). El TIC y el
      TSC ya se escriben (`muestreo.rs`, T0); falta la instruccion, el pool
      de descriptores en la VRAM y subir la textura. **Como se sabe:** el
      muestreo puntual da los bits de la CPU; el lineal, dentro de la
      tolerancia que diga LI2g.
      **09-10, decidida (DL17 de PLAN_LAS_LIBRERIAS):** las formas que pide
      el juego -- con nivel, arrays, cubos, 3D y Load --, por capas: E8g de
      PLAN_LA_LENGUA_DE_LA_3060 (lo que dijo `ptxas` de cada una, y el
      orden). Y el LDC con indice de las luces (DL18, E8f), su vecino.
      **10-10, la lengua HECHA en el anfitrion:** `SampleLevel` en 2D, 3D
      y array, y `Load` en 2D y array, con los bits de la casa y juzgadas
      (E8g; `niveles.hlsl` de dxc entero: 59 instrucciones, 40 registros).
      Falta lo de esta casilla que es Ring 0 -- el TIC de bloques con mips,
      las capas, la subida -- y el cubo, que es decision (E8g).
      **11-10, VERRANO ya las lleva (en el anfitrion):** `Texture`,
      `Sampler` y `Resources` en el `Draw`, y el juez interpola lo que deja
      el de vertice CON PERSPECTIVA (lo que no cambia queda exacto; el
      recorte de cerca recorta tambien eso). `textura.hlsl` (dxc) en un
      cuadro cuyos vertices de arriba estan tres veces mas lejos da, pixel a
      pixel, LO MISMO que la casa (`lote::en_cpu`) -- lineal y repetir,
      punto y borde: 0 pixeles distintos --; saboteado (la uv lineal en la
      pantalla, el lineal como punto): cae. La huella de BMOX-12 no se
      movio.
- [ ] **LI2d -- varios render targets y la profundidad** (N5.8; V2/M1 de
      `PLAN_VERRANO.md`). El G-buffer: hasta 8 destinos y el z-buffer en la
      VRAM. **Como se sabe:** la escena 3D dura (A11) pintada por la 3060
      con la huella de Windows.
      **11-10, la mitad de VERRANO HECHA en el anfitrion, y con ella la
      MEZCLA:** el `Draw` de VERRANO (V3a) escribe hasta 8 render targets a
      la vez (la salida k al k) y mezcla cada uno con las reglas de D3D12 --
      los 13 factores que usan los juegos, las 5 operaciones, la mascara y
      el factor de `OMSetBlendFactor` --, en floats sobre lo que ya estaba
      y a 8 bits otra vez; y un dibujo SIN limpiar deja lo de antes, para
      la pasada que suma la luz. Probado: la mezcla da los MISMOS bits que
      la de la casa (`bmo_proton_x::mezcla`) en las 1.690 combinaciones;
      `gbuffer.hlsl` (dxc) pinta sus tres destinos -- el albedo, el sitio de
      su SV_Position y la normal -- pixel a pixel; la luz que se suma en dos
      pasadas, la transparencia y la mascara. Saboteado tres veces (sin
      mezcla, la salida 0 en todos, la resta al reves): cae cada vez.
      **Falta la mitad de la 3060:** varios destinos en la tuberia
      (`pso.rs` aun dice "varios render targets") y su mezcla en el ROP,
      con la regla 4 del juez (el redondeo de la 3060) en el metal.
- [~] **LI2e -- la entrada de vertices de verdad**: formatos de 8 y 16 bits
      (UNORM, SNORM, half) y `SV_VertexID` / `SV_InstanceID`. **Como se
      sabe:** las instancias de N5.19 por la 3060, con su huella.
      *12-10, la mitad de los valores de sistema* (el censo de Cyberpunk: 16
      lotes de 34): `pegamento::Sistema` en la receta (bits 8..10 de
      `componentes`); SV_VertexID es el `vid` del ALD (IMAD.SHL por 1) y la
      puerta escribe los indices SIN el vertice base y copia los vertices
      DESDE el: el `vid` es el de la CPU (`lote::entrada`) y la memoria no se
      mueve. SV_InstanceID, 0 (una instancia). Sin paso, un cuadro sin bufer
      de vertices. Lo probo el banco: `el_cuadro_de_sv_vertexid_va_a_la_3060`
      y `con_vertice_base_el_vid_es_el_de_la_cpu` (puerta, receta, pegado de
      prueba con su juez y el vigia contra la CPU), `sv_vertexid_sale_del_vid`
      y `el_valor_de_sistema_cruza_la_receta`, con sus pruebas del NO. El
      juez cazo un fallo del primer intento (R3: el cuerpo esperaba la
      barrera de unas cargas que no habia). **Falta:** el metal, los formatos
      de 8 y 16 bits y varias instancias.
      *12-10, los FORMATOS* (el censo de Cyberpunk: 12 lotes de 34):
      `ga10x/src/trabajos/formatos.rs` -- UNORM, SNORM, half, enteros de 8,
      16 y 32, R10G10B10A2 y B8G8R8A8, en la libreria de la 3060 --. El
      pegamento carga la palabra que lleva cada componente y la convierte
      con instrucciones de latencia FIJA: sus bits abajo (SHF), el entero
      EXACTO como float por la suma magica (0x4B000000, sin I2F) y la
      division de IEEE por el cociente corregido (q = n/D aproximado, e = n -
      q*D con una FFMA, q + e/D), y max(-1) en SNORM. **Los mismos bits que
      la CPU (`formato_ia::leer`) en TODO el dominio**: los 256 valores de 8
      bits, los 65 536 de 16, los de 10 y 2, con basura alrededor -- mas de
      un millon --; saboteado (sin la correccion del cociente) cae en el
      0x101 de UNORM16. El formato viaja en los bits 16..24 de `componentes`
      de la receta; el vigia le da al simulador lo que lee la CPU. De punta
      a punta: el cubo con su NORMAL en SNORM16 o half y su COLOR en RGBA8 o
      BGRA8 cruza la puerta, la receta, el pegado del kernel (con sus
      conversiones) y su juez, y el vigia lo da igual a la CPU.
      **Falta:** R11G11B10_FLOAT (lo dice el censo), varias instancias y el
      metal.
- [ ] **LI2f -- el computo del juego en la 3060.** Un PSO de computo al QMD
      (la 3060 ya corre computo fijo: `trabajos/blur.rs`), con UAV. **Como
      se sabe:** `olas` (A10) por la 3060, con su huella.
      **11-10, la mitad de VERRANO HECHA en el anfitrion:** la categoria
      COMPUTO (`Compute`, `Dispatch`, `ReadBuffer`/`RwBuffer`: los SRV y los
      UAV) y el juez que despacha con el de la casa (`Programa::despachar`
      de PROMETEO: grupos, memoria compartida, barreras, olas). `computo.hlsl`
      (dxc) en dos grupos: cada hilo, el de su espejo tras la barrera;
      `compartido.hlsl` -- los `Interlocked*` sobre la memoria compartida y el
      `mad` de enteros que pidieron dos CS de Cyberpunk en el metal --: lo
      que quedo, los "antes" distintos, uno solo sin el cambio; y los techos
      de D3D12 (grupos de 1 a 65535, hilos hasta 1024) se dicen. Saboteado
      (un grupo menos, el SRV sin su paso): cae. **Falta la mitad de la
      3060:** el emisor aun rechaza `EscribeUav`, la memoria compartida, los
      atomicos y las barreras, y el PSO de computo no va al QMD.
- [ ] **LI2g -- DECISION DEL PROPIETARIO: los bits exactos o la tolerancia
      de D3D.** La regla de la casa es "los mismos bits que la CPU". La 3060
      APROXIMA la division, la raiz y el filtrado; D3D lo permite (unos
      ULP). Para un juego: o se acepta la tolerancia de la especificacion en
      esas operaciones (y el juez mide en ULP), o esas operaciones van por
      la CPU para siempre. **Como se sabe:** la decision escrita aqui, y el
      juez con la regla elegida y su prueba del NO.
      **09-10, la decision (DL10 de PLAN_LAS_LIBRERIAS):** *"Exactos,
      tambien en 3060"* -- los mismos bits, con la secuencia de redondeo
      correcto de `ptxas` y no la tolerancia --. La DIVISION ya es exacta en
      la 3060 (LB6b: `proton-x-sm86/src/cociente.rs`, los bits de IEEE aunque
      el MUFU se equivoque); la raiz va con el propietario (cambia el SASS
      de BMOX-12 que corre en el metal); el filtrado, con LI2c.
- [ ] **LI2h -- el sombreador de geometria**, si LI0 dice que Cyberpunk lo
      usa. Si no, se aparca con motivo.
- [~] **LI2i -- el render target de FLOAT (HDR)** (el censo de Cyberpunk,
      12-10: 28 lotes de 34). La casa lo guarda en floats de 32 ya
      redondeados a su formato (`formato_ia::cuantizar`); la 3060 escribe
      ESO mismo (`RF32_GF32_BF32_AF32` o `RF32`, `destino::Flotante`, el mapa
      del destino a 16 MiB) y el pegamento del de pixel redondea la salida
      canal a canal (`pegamento::Redondeo`: igual, al half con F2FP.PACK_AB
      y HADD2.F32 -- las palabras de `ptxas` --, 0 o 1). El juez del kernel
      las conoce SOLO en esa forma; el simulador, tambien. *12-10, en el
      banco:* `el_redondeo_de_la_3060_es_el_de_la_cpu` (6 formatos, 20 000
      floats, bit a bit con la CPU), `un_cuadro_en_un_rgba16f_va_a_la_3060`
      (puerta, receta, pegado del kernel con su juez y el vigia),
      `el_render_target_de_float_cruza_la_receta` y
      `el_redondeo_al_half_solo_en_su_forma`, con sus pruebas del NO.
      **Falta:** el metal; la Z con float (la sombra en bloque es de 8 bits:
      lo para `tuberia::cabe` y lo vigila el guardian `la-3060`, regla Z);
      R11G11B10 y los UNORM de mas de 8 bits; limpiar un float en el lote;
      la carga de un NaN (en la 3060 puede salir otro NaN: el juez lo da por
      bueno). *Lo que se supone y se mira en el metal:* que F2FP.PACK_AB
      redondea al par mas cercano (es el `cvt.rn` de PTX).
- [~] **LI2j -- LA MEDIDA DEL JUEGO** (metal 12-10). La puerta dibuja
      1280x720 (Q0a1 de `EL_FOCO.md`), y Cyberpunk abrio una VENTANA de
      1738x1064: todos sus lotes iban a la CPU por eso, y el censo no lo
      decia (desde el 12-10 si: "un destino que no mide 1280x720"). DOOM pide
      1280x720 y la 3060 se lleva la pantalla entera. Dos caminos: abrir
      Q0a1 (otra medida, con filas alineadas a 128 B o por la copia) o que
      PROTON-X le diga al juego que la pantalla mide 1280x720. **Como se
      sabe:** el censo sin esa puerta, y la 3060 con lotes de Cyberpunk.
      **Hecho el segundo camino (12-10), como pieza:** "BMO-X le da lo que
      el juego quiere, y luego el dueno toma el control".
      - `proton-x-casa/src/pantalla.rs`: la pantalla que ve el juego deja de
        ser una constante (`PANTALLA = 1920x1080`) y la pone QUIEN CARGA.
        user32 (GetSystemMetrics, el area de trabajo, GetMonitorInfo,
        EnumDisplaySettings, el cursor, ClipCursor) y DXGI (la salida, sus
        modos, FindClosestMatchingMode) leen de alli: los mismos modos, de
        menor a mayor, las medidas comunes que CABEN y la pantalla la ultima.
      - Una ventana de arriba con marco no pasa de la pantalla
        (CreateWindowEx la recorta, como Windows a SM_CXMAXTRACK).
      - `puerta::MEDIDA` (proton-x-sm86): la medida que la puerta dibuja, de
        `cubo::ANCHO/ALTO` de ga10x. Con `--3060`, PROTON-X le da esa al
        juego, sin barra de tareas; sin `--3060`, Full HD como siempre.
      - El dueno manda: `--pantalla WxH` (cualquier orden con las otras) se
        queda aunque haya `--3060`, y la consola dice si es otra medida que
        la de la puerta (lo de otra medida va por la CPU). Y dentro del
        juego, su menu elige entre los modos que la casa ofrece.
      - Guardian `la-3060` regla MD: la medida sale de la GPU, y la3060.rs no
        escribe `(1280, 720)` a mano.
      **Por que no Full HD en la 3060 todavia:** la puerta (Q0a1) solo deja
      pasar 1280x720; la sombra de la Z es de 1280x720 a 8 bits; y un
      destino de float de 1920x1080 son 31.6 MiB, el doble del mapa del
      destino (8 PT = 16 MiB). Es Q0a1 con su sombra y su mapa: otra pieza.
      **El metal (12-10, 11:21):** la consola dijo "la pantalla que ve el
      juego: 1280x720" y la ventana de Cyberpunk salio de 1280x720. Lo que el
      censo seguia contando como "otra medida" (18 de 34 lotes) son los
      render targets PROPIOS del juego (256x1, 256x256: sus tablas), no la
      ventana: desde ahora el censo los dice aparte ("un render target del
      juego de otra medida"). Abrirlos es Q0a1 (la puerta de otras medidas),
      no esta casilla. Y el juez en vivo tiraba un PSO por un `Load` (TLD) que
      el simulador SI sabe: le faltaba la textura nula (arreglado, con prueba).

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
   ILLAPA:     gpu fn      ->  Programa  ->  proton-x-sm86  ->  juez  ->  3060   (LI7, 07-10)
```

Si ILLAPA tuviera su PROPIO emisor de SPIR-V a la 3060, cada cosa se
aprenderia DOS veces (los bucles, las texturas, el computo) y la escuela no
le serviria de nada al examen. Con un lector de SPIR-V al `Programa`, todo lo
que aprenda ILLAPA cae en el MISMO emisor, el MISMO juez y el MISMO .bsf
vivo que usa Cyberpunk.

- [x] **LI7 -- UNA SOLA LENGUA HACIA LA 3060, SIN SPIR-V.** Decision del
      propietario (07-10): *"gpu - sm86 - luego el juez verifica
      estrictamente y el 3060"*. HECHO el 07-10: `bmo-titan-sm86`
      (`toolchain/lang/titan/emisor-sm86`; desde el 08-10,
      `bmo-titan-prometeo` en `toolchain/lang/titan/prometeo` -- PROMETEO,
      LB3 de PLAN_LAS_LIBRERIAS --) escribe cada `gpu fn` como el
      `Programa` de la casa, la emite con `proton-x-sm86` y la juzga ESTRICTO
      el juez del SASS (la libreria de la 3060); el oraculo de cada build es
      la 3060 simulada, contra la casa y el calculo. El escritor de SPIR-V de
      TITAN++ se quito; las leyes L28-L31 se sellaron de nuevo. **Como se
      sabe:** 9 pruebas del crate y su banco (`mezcla`, `activa`, `suma`):
      los tres caminos dan los mismos bits en la bateria; las pruebas del NO:
      la division general rechazada en su linea y columna, el SASS sin
      esperas rechazado por el juez, el signo de -0, y una gpu fn medida
      contra el calculo de otra. **Falta:** la division general (LI2g), y los
      bucles (IL1). **09-10:** los bucles llegaron con LB5 (08-10) y la
      division general, EXACTA, con LB6b de PLAN_LAS_LIBRERIAS.

## El orden de los dos planes juntos

```text
   ahora      smp all, el primer Present por la CPU, LI0 el censo
   la escuela LI7 (HECHO) -> IL1 (bucles) -> IL2-IL4 (computo) -> IL7
              (3D): cada una con sus gpu fn chicas y el oraculo
   el examen  cada vez que la escuela abre algo, el censo de LI0 se repite
              con Cyberpunk: el % de PSO que deja la CPU dice cuanto sirvio
```
