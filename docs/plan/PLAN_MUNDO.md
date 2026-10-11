# PLAN MUNDO -- el Minecraft propio, en TITAN++, por VERRANO

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

- [ ] **MC1 -- el mundo de bloques, la camara y picar.** Un mundo de
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
      **Falta el metal** (seccion 3).
- [ ] **MC2 -- mirar arriba y abajo, y la mira.** El cabeceo de la camara
      (otra rotacion en las gpu fn), el bloque apuntado RESALTADO (sus
      aristas en otro color) y el rayo de picar en 3D, no solo a la altura
      de los ojos. El raton gira la camara.
      **Necesita:** el raton ATRAPADO (movimiento relativo, sin salir de la
      ventana): hoy el buzon da posiciones absolutas.
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
- [ ] **MC5 -- guardar el mundo.** `datos/mundo.bin`: los bytes del mundo
      con `director.crea/escribe/cierra` (4e), y se carga al abrir.
- [ ] **MC6 -- texturas.** Cada tipo de bloque con su dibujo de 16 x 16, de
      un atlas. **Necesita:** M3 de VERRANO (la textura en la VRAM y su
      muestreador; Ring 0, la tarea de las texturas con mips).
- [ ] **MC7 -- la luz.** La luz del cielo por columna y la de las antorchas
      que se esparce bloque a bloque, como en el juego de verdad; y niebla
      a lo lejos.
- [ ] **MC8 -- agua y cristal.** Lo transparente necesita MEZCLA en VERRANO
      (hoy no la hay) y dibujar lo transparente al final, de lejos a cerca.

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
   MC1 bloques, camara,       si         si (tests/mundo.rs)     POR PROBAR
   picar
   V2b el recorte de cerca    con MC1    si (juez de la CPU)     POR PROBAR
   MC2..MC8                   no         --                      --
```

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
      pequenio de pixeles). Si dice DISTINTO con muchos pixeles abajo de la
      pantalla, es el recorte de cerca de la 3060 (V2b de PLAN_VERRANO)
```
