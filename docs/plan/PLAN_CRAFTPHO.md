# PLAN CRAFTPHO -- pintar y retocar en BMO-X, con capas, en la 3060

> El propietario (15-10): *"investigas Internet artcraft? puede servir como
> datos [...] que lienzo sea como nombre nuevo (CraftPho) que es photoshop +
> krita + Clip studio paint con todo con capas y eso"*.

---

## 0. La respuesta corta

```text
   que es        la app de pintar y retocar de BMO-X: Photoshop (retoque,
                 capas de ajuste, mascaras), Krita (pinceles, mosaicos de
                 64 x 64) y Clip Studio Paint (dibujo a linea, estabilizador)
   quien hace    TITAN++ administra la app; INTI calcula lo de CPU (rellenar,
   que           suavizar un trazo, deshacer, leer y escribir ficheros);
                 VERRANO dibuja en la 3060 lo que se ve (componer las capas,
                 los modos de mezcla, los filtros, el lienzo con zoom)
   la regla      DOS compositores, el de la 3060 y el de la CPU, y el de la
                 CPU es el JUEZ: cada fotograma de prueba, bit a bit (la ley
                 L32 y la regla 1b de EL_FOCO)
   el nombre     CraftPho es la APP. El `lienzo` del escritorio (el bloque
                 de RAM donde pinta el director antes de volcar) se queda
                 con su nombre: es otra cosa, y renombrarlo romperia cien
                 sitios sin ganar nada
```

---

## 1. Lo que se investigo (15-10): ArtCraft y PhotoCraft

**ArtCraft** (`github.com/storytold/artcraft`, MIT o Apache-2.0, Rust) es un
"IDE para artistas" de IA: compone imagenes y escenas 3D y llama a modelos
generativos de fuera (FLUX, Kling, Veo...) con la llave del usuario. **No es
un motor de pintura**: para CraftPho sirve poco.

**PhotoCraft** (`github.com/storytold/photocraft`, del mismo grupo, MIT o
Apache-2.0, octubre de 2026, "alpha temprana") SI es lo que se buscaba: un
Photoshop en Rust. Lo que vale la pena aprender de el (ideas, no codigo: es
de licencia permisiva, pero aqui todo sale de la casa):

```text
   dos compositores        uno en la GPU (wgpu) y otro de referencia en la
                           CPU, probados el uno contra el otro -- lo mismo que
                           VERRANO ya hace con su backend CPU
   mosaicos con copia      de 256 x 256, que solo se copian al escribirlos:
   al escribir             el deshacer sale barato (Krita usa 64 x 64)
   un registro de ordenes  cada accion es una orden con nombre; deshacer,
                           guion y repeticion salen del mismo sitio
   la lista de mezclas     27 de Photoshop + 6 de Paint.NET: la lista que hay
                           que cubrir
   el corpus de PSD        los 309 ficheros de prueba de psd-tools
```

[!] El codigo de PhotoCraft se escribio en buena parte con IA: es una
referencia de que se puede, no una prueba de que este bien.

**Krita** es GPL-3.0 (algunos ficheros LGPL): se LEE para aprender
(el mosaico de 64, la separacion entre la funcion de mezcla y lo que la
envuelve) y NO se copia. CraftPho se escribe desde las especificaciones
abiertas: W3C Compositing and Blending, OpenRaster, PSD de Adobe y PNG.

---

## 2. Que hace cada lado

```text
   la 3060 (VERRANO)        componer la pila de capas, los 16 modos de mezcla
                            del W3C, opacidad, mascaras y recorte; los ajustes
                            (curvas, niveles, tono) como tablas; el desenfoque;
                            el lienzo con zoom y giro; estampar los toques del
                            pincel en tandas
   la CPU (INTI)            rellenar y la varita (dependen del vecino); suavizar
                            el trazo y el estabilizador; el deshacer (que
                            mosaico se copia); comprimir y leer ficheros (ZIP,
                            PNG, el RLE de PSD)
   TITAN++                  la app: la ventana, los paneles, las herramientas,
                            las ordenes; habla con las dos puertas
```

---

## 3. La escalera (de lo mas chico a lo mas grande)

- [ ] **K0 -- la pila de capas, compuesta.** Capas de pixeles en mosaicos de
      64 x 64, visibles u ocultas, con opacidad; NORMAL (source-over,
      premultiplicado) en la 3060 por VERRANO, y el mismo calculo en la CPU.
      **Como se sabe:** tres capas, la 3060 y la CPU, IGUAL bit a bit.
- [ ] **K1 -- los 12 modos separables del W3C** (multiplicar, trama,
      superponer, oscurecer, aclarar, sobreexponer, subexponer, luz fuerte,
      luz suave, diferencia, exclusion, normal), un programa con un `match`
      por modo. **Como se sabe:** cada modo contra la formula del W3C en la
      CPU, y la 3060 contra la CPU.
- [ ] **K2 -- los 4 no separables** (tono, saturacion, color, luminosidad).
- [ ] **K3 -- el pincel redondo:** toques espaciados por un tanto del
      diametro, la presion al grosor y a la opacidad, la goma, y bloquear la
      transparencia.
- [ ] **K4 -- deshacer:** por trazo, los mosaicos tocados (copia al escribir)
      y las ordenes de las propiedades de capa.
- [ ] **K5 -- OpenRaster:** guardar y abrir `.ora` (ZIP + `stack.xml` + PNG),
      y abrirlo en Krita o GIMP para ver que es lo mismo.
- [ ] **K6 -- grupos, mascaras de capa y de recorte** (src-atop), y relleno
      aparte de opacidad.
- [ ] **K7 -- la seleccion** (rectangulo, lazo, varita) como mascara de 8 bits.
- [ ] **K8 -- el estabilizador** (ponderado, despues el de Krita) y las
      curvas de presion: el dibujo a linea de Clip Studio.
- [ ] **K9 -- capas de ajuste en la 3060** (niveles, curvas, tono y
      saturacion, desenfoque gaussiano).
- [ ] **K10 -- los modos de Photoshop que faltan** y 16 bits o coma flotante.
- [ ] **K11 -- PSD**: importar y exportar lo basico de las capas.
- [ ] **K12 -- abrir `.kra`** (solo lectura, con el formato de mosaicos que
      Krita documenta).
- [ ] **K13 -- capas vectoriales y difuminar.** El `.clip` de Clip Studio no
      tiene especificacion publica: no se promete.

**Lo que pide de abajo:** K0 necesita que una app dibuje en SU ventana por la
3060 (B3 y C1 de EL_FOCO, hechos en el anfitrion; falta el Ryzen) y una
tableta o raton con presion (hoy el raton no la tiene: K3 empieza con presion
fija). El juez de la CPU es el backend CPU de VERRANO.

---

## 4. Las formulas (W3C Compositing and Blending Level 1)

```text
   source-over, premultiplicado (c = color * alfa):
      co = cs + cb * (1 - as)          ao = as + ab * (1 - as)
   un modo B antes de componer:
      Cs' = (1 - ab) * Cs + ab * B(Cb, Cs)
   separables (por canal, de 0 a 1):
      multiplicar   Cb * Cs              trama        Cb + Cs - Cb * Cs
      oscurecer     min(Cb, Cs)          aclarar      max(Cb, Cs)
      diferencia    |Cb - Cs|            exclusion    Cb + Cs - 2 * Cb * Cs
      superponer    luz fuerte con los argumentos al reves
      luz fuerte    Cs <= 0.5: multiplicar(Cb, 2Cs); si no, trama(Cb, 2Cs - 1)
      (sobreexponer, subexponer y luz suave: en la especificacion, con sus
       casos en 0 y en 1)
   no separables: Lum(C) = 0.3 R + 0.59 G + 0.11 B, y SetLum/SetSat/ClipColor
```

Fuente: https://www.w3.org/TR/compositing-1/ -- las formulas se comprueban
contra ella el dia que se escriban, no contra esta copia.
