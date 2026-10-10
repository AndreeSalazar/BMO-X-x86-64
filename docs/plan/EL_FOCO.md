# EL FOCO -- una cosa a la vez, hasta el final (10-10)

> Peticion del propietario, **2026-10-10**: *"enfoca en algo, terminar eso y
> luego seguimos [...] me gustaria que enfoques mas en TITAN++ y VERRANO con
> PROTON-X + INTI esos elementos y LAMINA la RAM, el INTI en NAVEGAR si
> eliminar porque ya TITAN++ lo toma, puedes organizar?"*
>
> [`ABIERTO.md`](ABIERTO.md) dice QUE falta; [`EL_ORDEN.md`](EL_ORDEN.md), con
> que CRITERIO. Esta pagina dice QUE SE HACE AHORA: UNA cosa, con su linea de
> llegada, y la cola detras. Lo que no esta aqui espera.

---

# 1. EL MAPA: que hace cada uno

```text
   TITAN++    EL QUE ADMINISTRA. Las apps se escriben en TITAN++, y TITAN++
              conecta la CPU (por INTI) y la GPU (por VERRANO). Sus gpu fn
              cuentan en la 3060 o en la CPU, bit a bit lo mismo (LB4)
   INTI       LA PUERTA A LA CPU, y nada mas. Se queda lo de CPU: cpu.inti,
              pulso.inti, bico.inti, png.inti y el emisor. Pierde TODO lo de
              app, pieza a pieza, el dia que su relevo en TITAN++ existe
              (3.3 de PLAN_INTI_SAMURAI)
   VERRANO    LA PUERTA A LA GPU: la API de dibujo. Detras, la RTX 3060 12G
              (o la CPU, la misma API)
   LAMINA     LA RAM COMPARTIDA: quien cuenta PUBLICA en un bloque de RAM, y
              quien dibuja lo LEE ahi, sin copias ni syscalls por byte (la de
              VERRANO para vertices; la de la antena para paginas)
   PROTON-X   LOS JUEGOS DE WINDOWS: traduce D3D12 y DXIL a la casa, y lo que
              la puerta deja, a la 3060 (la receta)
```

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

- [ ] **F1 -- la VENTANA en TITAN++** (TA2): `director.ventana(ancho, alto)`
      pide su superficie y la ofrece, una fila de pixeles se escribe, y
      `director.presenta()` la entrega; con `screen = true` en el Titan.toml
      y su linea en el certificado. **Como se sabe:** un ejemplo de nivel 11
      lanzado por un escritorio de mentira en el anfitrion pinta un
      degradado y el escritorio lee los mismos pixeles, bit a bit; y sus NO
      (sin `screen`, sin `use director`, desde una gpu fn).
- [ ] **F2 -- los BYTES** (TA1): un tipo de 8 bits sin signo y tablas grandes
      en un bloque pedido. **Como se sabe:** su codigo T, su ejemplo y su
      nivel en la GRAMATICA; una tabla de millones de celdas sin copia.
- [ ] **F3 -- la ENTRADA** (TA3): el raton y las teclas que el DIRECTOR le
      manda a su ventana. **Como se sabe:** el escritorio de mentira manda un
      clic y una tecla, y el programa los ve.
- [ ] **F4 -- NAVEGAR en TITAN++**: toma la lamina que le OFRECE el
      antenista (o la del disco, `datos/ejemplo.lam`), la pinta en su ventana
      y devuelve clics y teclas, como la version 3 de `navegar.inti`.
      **Como se sabe:** en el anfitrion, la de example.com pintada por el de
      TITAN++ y por el de INTI da los MISMOS pixeles.
- [ ] **F5 -- el corte 4c de INTI**: `navegar.inti` fuera del build y del
      arbol, el icono y `run apps/navegar...` apuntan al de TITAN++, y el
      perfil `pleno` sale con el. **Como se sabe:** el build sin
      `navegar.ibx`; y en el metal, del propietario, NAVEGAR abre desde su
      icono y pinta example.com.

Cada paso entra con sus pruebas en el anfitrion; el metal, UNA vez, al final
(F5).

# 4. LA COLA: lo que va despues, en este orden

```text
   Q1  el juez de E8f/E8g en el metal: un .exe de DX12 con un cbuffer
       indexado, SampleLevel y Load en una 2D de una mip, que la puerta mande
       a la 3060; su tabla en Windows y despues en el Ryzen
       (PLAN_LA_LENGUA_DE_LA_3060)
   Q2  el sonido en TITAN++, y el corte 4d de INTI (musica.inti fuera)
   Q3  DOOM por la 3060 cerrado (D2c, la linea [perf]) y su paleta (D3)
   Q4  las texturas con mips en la VRAM, Ring 0: lo que deja a Cyberpunk
       mandar a la 3060 sus SampleLevel de verdad
   Q5  el Sample normal de un pixel (la mip de su cuadro)
   Q6  adelgazar la CPU de alrededor del dibujo (S3 y S4 de PLAN_VERRANO)
```

**Lo del propietario, en cada arranque:** apagar (no reiniciar), `gpu init`
y `save`: cada `SALIDA.TXT` es una fila de G0 hasta diez. Y una decision que
espera sin prisa: el CUBO de E8g (la division de la 3060, la de la casa, o
esperar).
