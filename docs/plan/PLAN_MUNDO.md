# PLAN MUNDO -- el Minecraft propio, en TITAN++, por VERRANO

> **Estado vigente 16-10, despues de la tanda 20:36:** MUNDO es juego
> propio y cliente nativo de prueba, no un port completo de Minecraft.
> MC1 y guardado confirmados; entrada relativa confirmada por el propietario.
> C1 visto con 325 fotogramas GPU; Q0a3 conserva cuatro pixeles diferentes.
> Ultima tanda: 1.141 fotogramas CPU por G0/0x15, muestra 19,60 FPS.
> Primero [G0](PLAN_LA_3060.md), despues igualdad de imagen y fisica.

> Abierto el 2026-10-11. El orden lo aprobo el propietario el 10-10 (seccion
> 5h de [`PLAN_LA_TINTA.md`](PLAN_LA_TINTA.md)): TB1 de LA TINTA primero,
> despues *"minecraft propio en 3D"* para ENSENIAR a VERRANO, e ILLAPA al
> final. El 11-10: *"bien continua que mas faltan [...] en juegos, minecraft
> propio y bueno 3D y VERRANO..... que mas faltan?"*.
>
> Y el mismo dia, sobre la lengua: *"recuerda reemplazamos el SPIR-V [...]
> si es VULKAN simplemente es esto que es VERRANO que toma su lugar que come
> VULKAN y DX12 (y en general claro)"*. MUNDO no pasa por Vulkan ni por
> D3D12: habla VERRANO directo, como todo lo de la casa.

---

# 0. PARA QUE ES

**16-10, referencia local pedida por el propietario:** se localizo Minecraft
26.3 y New World. Se inventariaron modelos, texturas y firmas de clases,
sin ejecutar Java ni incorporar el juego al repo. [Estudio junto a MUNDO](../../Ultra_userspace/apps/mundo/ESTUDIO_MINECRAFT.md).
Esta lectura prepara compatibilidad por datos; no cierra MC6 ni cambia la
siguiente meta jugable MC2a.

Un juego de bloques es la mejor escuela de un motor 3D: mucha geometria
sencilla, una camara que anda, un mundo que CAMBIA (se quitan y se ponen
bloques) y que hay que guardar. Cada peldanio de abajo obliga a VERRANO o a
TITAN++ a aprender algo que despues usa cualquier juego -- y, lejos, lo que
pide Cyberpunk por PROTON-X --.

```text
   LA APP         Ultra_userspace/apps/mundo/ (TITAN++), apps/mundo.bex
   LO QUE DIBUJA  VERRANO: la lamina (LB7b), la profundidad y el descarte
                  (V2), el recorte de cerca (V2b), en la 3060
   LA PRUEBA      toolchain/lang/titan/emisor-x86_64/tests/mundo.rs: el
                  escritorio de mentira la lanza, le manda teclas, lee la
                  lamina y la dibuja con el juez de la CPU
```

---

# 1. LA ESCALERA

- [x] **MC1 -- el mundo de bloques, la camara y picar.** Un mundo de
      16 x 8 x 16 en una `[byte]` (TA1): piedra, hierba, una torre, un arbol
      y una loma. Sus caras JUNTADAS en rectangulos (*greedy meshing*: un
      suelo de 16 x 16 es una cara, no 256) y, cada fotograma, solo las que
      miran a la camara (entero y exacto: una cara +x se ve desde x mayor
      que su plano). Las cuentas de cada vertice, gpu fn (girar con la
      camara y pasar a recorte); quien tapa a quien, las caras de detras y lo
      que queda detras de los ojos, VERRANO. Teclas: w s a d andar, q e
      girar (15 grados), r f subir y bajar, x quita el bloque que se mira, c
      pone uno de tablas, Esc cierra.
      **11-10, HECHO en el anfitrion:** 23 caras al abrir (46 triangulos);
      el juez dibuja el suelo hasta el borde de abajo, la torre, la loma y el
      arbol; con y sin descarte, la misma imagen (las caras van bien);
      andar y girar cambian la imagen (girando a la derecha la torre sale);
      x abre un hueco en la loma (23 -> 27 caras) y c lo tapa con tablas que
      se ven. Saboteado con las caras al reves: cae. Lo que hizo falta por
      el camino, cada uno con su prueba:
      - VERRANO, **V2b**: el recorte de cerca en el juez (sin el, el suelo,
        que es UNA cara que pasa por debajo de la camara, se perdia entero).
      - E1, **la pila**: un `mut` prestado ocupaba en el marco la tabla
        ENTERA (es un puntero: 8 bytes), y `let t = [x; n]` la tenia dos
        veces (con un temporal al lado). `trozo` paso de 38 KiB a 7
        (`emisor-x86_64/tests/pila.rs`).
      - El juez de TITAN++, **T0057**: un `mut` que cambia en UN camino y
        vuelve pronto por otro (`return false`) ya no es "no cambia nunca"
        (`ejemplos/nivel7/cambia_en_un_camino.titan`).
      **16-10:** el metal rechazo el prestamo C1 y despues el dibujo copiado.
      Se agrega el error de VERRANO y el ultimo motivo del kernel a la salida;
      si la GPU no acepta, el escritorio usa VERRANO CPU con profundidad y
      descarte. Tambien usa CPU si no hay GPU lista o el ancho no es multiplo
      de 32. Compilado; el dibujo del mundo pasa en el juez del anfitrion.
      **METAL 16-10, confirmado por el propietario:** probo jugar y verlo
      funcionar: "FUNCIONO MUY BIEN". Se cierra la base funcional de MC1.
      En esa primera confirmacion no se identifico el backend. La cuarta
      tanda posterior acredita C1 con 325 fotogramas GPU; Q0a3 sigue abierto.
- [ ] **MC2 -- mirar arriba y abajo, y la mira.** El cabeceo de la camara
      (otra rotacion en las gpu fn), el bloque apuntado RESALTADO (sus
      aristas en otro color) y el rayo de picar en 3D, no solo a la altura
      de los ojos. El raton gira la camara.
      **16-10:** el raton relativo y la captura estan implementados y los
      controles fueron confirmados. Falta el bloque resaltado y comprobar
      en metal los casos especificos de limites y alcance de MC2a.
- [ ] **MC3 -- la fisica.** Gravedad, saltar, y el jugador como una CAJA
      (no un punto) que choca con los bloques por ejes. Andar con la tecla
      MANTENIDA (eventos 1 y 2, pulsada y soltada), no con la repeticion de
      la letra.
- [ ] **MC4 -- un mundo grande: trozos.** Trozos de 16 x 16 x 16, generados
      con ruido (montes, cuevas, arboles), y solo los de cerca. Las caras
      de cada trozo se juntan una vez y se guardan.
      **Necesita (lo que mas falta):** que los vertices no vivan en la pila.
      Hoy una tabla de f32 vive en el marco (8 bytes por celda en E1, 48
      KiB para todos): por eso MC1 dibuja 80 caras como mucho. Dos caminos,
      se elige al llegar: (a) `[f32]` en el monton y `director.publica`
      desde una lista; (b) M2 de VERRANO -- los vertices FIJOS en la VRAM y
      solo la camara por fotograma --, que es lo que hace un juego de
      verdad. Y el tope de la lamina en el DIRECTOR (`laminas.rs`, 768
      vertices) sube con el.
- [x] **MC5 -- guardar el mundo.** `datos/mundo.bmv`: cabecera BMW1 de ocho
      bytes (magia, medidas 16 x 8 x 16, reservado), seguida de 2048 bloques.
      **16-10, HECHO en el anfitrion:** carga validando longitud, cabecera
      y todos los tipos antes de usar los bloques; G guarda, Esc guarda los
      cambios y cierra. Si el disco falla al salir, conserva el juego abierto.
      Seis pruebas de MUNDO pasan: dibujo, movimiento, picar, persistencia,
      guardados corruptos y fallo de escritura. No guarda la camara.
      **Metal, segunda tanda 16-10:** tras la consigna de cerrar/recargar,
      el propietario confirma "bien cumplen". Cierre por confirmacion del
      propietario; los nuevos logs no registran guardar/cargar por separado.
      Uso: [README de la app](../../Ultra_userspace/apps/mundo/README.md).
      **18-10, el propietario elige (a) primero y (b) despues.** Hecho en el
      anfitrion: las gpu fn y `director.publica` toman listas `[f32]` del
      monton, y un valor suelto llega a cada celda. MUNDO cuenta todas sus
      caras de una vez (`dibuja`), sin tablas de la pila: el techo pasa de
      80 caras a 126, que es lo que el DIRECTOR dibuja de una lamina (768
      vertices, `laminas.rs`). Las pruebas de MUNDO dan los mismos pixeles;
      el primer fotograma cuesta 2.800.000 instrucciones (antes 2.855.000).
      Faltan: subir los 768 del DIRECTOR (y verlo en el metal), los trozos
      y el ruido.
- [ ] **MC6 -- texturas.** Cada tipo de bloque con su dibujo de 16 x 16, de
      un atlas. **Necesita:** M3 de VERRANO (la textura en la VRAM y su
      muestreador; Ring 0, la tarea de las texturas con mips).
- [ ] **MC7 -- la luz.** La luz del cielo por columna y la de las antorchas
      que se esparce bloque a bloque, como en el juego de verdad; y niebla
      a lo lejos.
- [ ] **MC8 -- agua y cristal.** Integrar la MEZCLA de VERRANO en este juego
      y dibujar lo transparente al final, de lejos a cerca.

---

# 2. LO QUE LE FALTA A VERRANO Y A TITAN++ PARA JUEGOS, en orden

```text
   PIEZA                         PARA          DONDE
   V2b el recorte de cerca       MC1           HECHO en el anfitrion
   los vertices fuera de la      MC4           E1 ([f32] en el monton) o
   pila                                        M2 de VERRANO
   M2 = V3 la matriz en la 3060  MC4, todo     PLAN_VERRANO: vertices fijos
                                               en la VRAM, la camara por
                                               constantes
   M3 las texturas               MC6           PLAN_VERRANO + Ring 0
   el raton atrapado y las       MC2, MC3      el escritorio (superficie)
   teclas mantenidas
   la mezcla (transparencia)     MC8           VERRANO
   M5 Vulkan a VERRANO           juegos de     PLAN_VERRANO V5: VERRANO se
                                 fuera         come Vulkan
   PROTON-X: D3D12 a VERRANO     Cyberpunk     PLAN_PROTON_X, PLAN_LA_
                                               ESCALERA_PROTON_X
```

---

# 3. LA LISTA DEL PROPIETARIO

```text
   QUE                        APROBADO   HECHO EN EL ANFITRION   EN EL METAL
   el orden (TINTA, MUNDO,    si (10-10) --                      --
   ILLAPA)
   MC1 bloques, camara,       si         si (tests/mundo.rs)     SI, PROPIETARIO 16-10
   picar                     (juego funcional confirmado 16-10; backend sin identificar)
   V2b el recorte de cerca    con MC1    si (juez de la CPU)     GPU POR PROBAR
   MC5 guardar                si (16-10) si (tests/mundo.rs)     SI, PROPIETARIO 16-10
   MC2..MC4, MC6..MC8         por cerrar --                      --
```

## 4. METAS VIGENTES -- separar entrada, dibujo y fisica

- [x] MC1 funcional en metal, por confirmacion del propietario (16-10).
- [x] MC5 implementado y probado en el anfitrion, incluidos corrupcion y fallo de disco.
- [x] MC5 metal: confirmado por el propietario en la segunda tanda del 16-10.
- [ ] MC2a: mirar arriba y abajo por teclado, mira central y rayo de picar
      que siga la inclinacion. Probar arriba/abajo, alcance y bloque colocado.
      **HECHO en el anfitrion, 16-10:** I/K hasta +/-75 grados, yaw y pitch
      coherentes con el rayo, mira siempre delante por profundidad cero.
      Nueve pruebas pasan, incluidas persistencia, suelo picado y tablas
      colocadas con inclinacion, limites del giro y mira visible. 80 caras
      del mundo (480 vertices) + mira (12), lamina de 492. Usa O1 en la camara.
      El juego usa O1 y el propietario confirma los controles nuevos;
      la comprobacion especifica de alcance, limites y variantes de O1
      sigue pendiente en metal. El respaldo CPU ya se vio funcionando.
- [x] MC2b-entrada: movimiento relativo del raton y captura al jugar;
      liberar al salir o perder el foco. Confirmacion del propietario 16-10.
      **Mirada relativa hecha en anfitrion, 16-10:** modulo pointer en TITAN++,
      cursor oculto al capturar y restaurado al liberar; Esc primero suelta,
      clic recaptura, foco perdido revoca. El propietario confirma los
      controles en metal (tanda 20:36); sigue pendiente resaltar el bloque.
      `gpu laminas` mide intervalos/FPS: muestra CPU de 19,60 en esa tanda.
      [Control y prueba](../../Ultra_userspace/apps/mundo/CONTROL.md).
- [ ] MC2b-resaltado: mostrar el bloque apuntado por el mismo rayo que
      quita/coloca. Comprobar que cambia al mirar y desaparece sin objetivo.
- [ ] MC3: teclas mantenidas, gravedad, salto y colision del cuerpo por ejes.
- [x] GPU-C1: corregir el rechazo 640x360/Z y ver buffers prestados en metal.
      Cuarta tanda: 325 fotogramas GPU prestados, cero CPU. Cumplido para
      esa corrida; no equivale a fiabilidad G0 ni a igualdad Q0a3.
- [ ] GPU-Q0a3: aislar los cuatro pixeles diferentes y cerrar la igualdad;
      seguir distinguiendo GPU de CPU. El respaldo no cierra esa meta.
      **Quinta tanda, 20:36:** capturas 1080p ya guardadas; 1.141 fotogramas
      CPU, cero GPU por G0 (booter y descargador 0x15, WPR2 no baja).
      Prioridad al arranque: foto previa del GSP nueva, pendiente de metal.
      **Cuarta tanda 16-10:** 325 fotogramas GPU prestados, cero CPU;
      C1 acreditado para la corrida. El juez difiere en 4/230.400 pixeles:
      Q0a3 de igualdad sigue abierto. Captura 1080p confirmada despues,
      pero esa quinta tanda uso CPU y no aporta el caso GPU de esos pixeles.
      **Tercera tanda 16-10:** GSP listo; rechazo local `Vertices` en ambas
      rutas. Reproducido: VRN1 no admitia Z a 640x360. Corregido en anfitrion
      junto con la copia de sombra (solo el rectangulo del destino).
      Esa tanda pidio metal con kernel/DIRECTOR nuevos; la cuarta ya
      acredito C1. El cierre que sigue pendiente es Q0a3, no ese rechazo.
- [ ] Despues MC4: vertices fuera de la pila y trozos; luego texturas,
      luz y transparencia. No aumentar el mundo sobre el limite de 80 caras.

El orden siguiente es G0, Q0a3 y despues resaltado/fisica. El guardado ya
esta confirmado por el propietario; no se pide repetirlo por rutina.
Una sesion normal con `gpu laminas` y `save` aporta datos, salga bien o mal
la RTX. Cyberpunk mantiene su hito y su plan independiente.

**Lo que se prueba en el metal** (y que seria un NO):

```text
   1  `run apps/mundo.bex` desde Ejecutar (o su icono): la ventana con el
      suelo verde hasta abajo, la torre gris, la loma y el arbol.
      NO: la ventana sin dibujo, o el suelo a trozos
   2  w s a d, q e, r f: la camara anda y gira, sin tirones.
      NO: no se mueve, o la imagen se rompe al girar
   3  andar hasta la loma (w unas 30 veces), x: un hueco; c: tablas en el
      hueco. NO: no cambia nada
   4  la consola: `[Q0a3] el juez ... la 3060 da lo MISMO` (o un recuento
      reducido de pixeles). Si dice DISTINTO, conservar el primer pixel,
      el recuento y la escena; no deducir la causa solo por su posicion.
      Si rechaza, conservar la linea `[VERRANO] dibujo rechazado` con su
      motivo. `[VERRANO CPU]` acredita el respaldo, no el render por GPU.
   5  quitar un bloque, G, Esc y volver a abrir: el hueco sigue. Poner tablas,
      Esc y volver a abrir: siguen las tablas. NO: reaparece el mundo inicial
      tras un mensaje que dice guardado correcto.
```
