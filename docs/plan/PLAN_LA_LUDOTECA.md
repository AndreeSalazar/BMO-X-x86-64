# PLAN LA LUDOTECA -- los juegos que compraste, en BMO-X, y por donde NO

> Pedido por el propietario (2026-09-25): *"los launchers son Python con
> Electron... Rust como base podria tener un crate simple pero elegante para
> tener el API de GOG y otros, asi BMO-X tendria su app simple para tener
> juegos; lo mismo con Proton de Steam, que es abierto, pero en uso
> correcto"*.

---

## 0. La respuesta corta

**Si a la app, no a meter Proton.** Un launcher (Heroic, Lutris, GOG Galaxy)
hace tres cosas muy distintas, y cada una va a un sitio distinto:

```text
   1  la LISTA      que juegos tienes, sus portadas, sus ficheros
                    -> BMO-X, un crate Rust puro y una app. SI
   2  la DESCARGA   hablar con la tienda: HTTPS, OAuth, su CDN
                    -> la ANTENA (el movil o un PC). BMO-X no tiene TLS:
                       es el muro de la criptografia (README)
   3  EJECUTAR      el juego de Windows o Linux
                    -> NO en BMO-X. Proton es Wine + DXVK + vkd3d sobre un
                       kernel Linux y un Vulkan conforme: traerlo es traer
                       POSIX y Win32 por la puerta de atras
                       (`docs/identidad/ENTRAR_EN_SU_ECOSISTEMA.md`, "Y NO,
                       con Windows no es lo mismo"; `PLAN_CLOUD_LOCAL.md`,
                       seccion 4)
```

## 1. Los tres caminos para JUGAR, del mas limpio al mas lejano

```text
   A  NATIVO: el motor abierto, compilado a .bex, con los DATOS que compraste
      DOOM ya lo hace: el motor es un port GPL fuera del arbol y el WAD va
      al lado (apps/doom1.wad; `build/ejemplos.ps1`). GOG vende DOOM, DOOM
      II, Heretic, Hexen, Quake... con sus WAD y PAK dentro: el juego que
      pagaste, corriendo en un motor que BMO-X compila. ESTE es el uso
      correcto: nada pirata, nada de Windows, y la 3060 ya pinta
   B  STREAMING: el juego corre en TU PC con Windows (o en un Linux con
      Proton, fuera de BMO-X), y BMO-X lo ve y le manda teclado y raton
      Es el modo ESPEJO (S6 de PLAN_CLOUD_LOCAL), como Moonlight / Steam
      Link, y con `gpu video` la 3060 ya pone el fotograma en pantalla
   C  PROTON DENTRO: no. Cuesta un Linux y un Vulkan enteros (NVK tardo ~2
      anios con varios expertos, PLAN_LA_3060 L2), y rompe la promesa de
      la superficie chica (`docs/identidad/LA_COMPATIBILIDAD.md`)
```

## 2. El crate: `bmo-ludoteca` (puro, no_std, con banco)

Lo que Heroic hace en miles de lineas de Electron, aqui es un formato de
lineas y una tabla:

```text
   JUEGO <id> <tienda> <titulo>              uno por juego que tienes
   FICHERO <id> <nombre> <bytes> <sha256>    lo que la antena puede traer
   MOTOR <id> <motor>                        si hay motor nativo (camino A)
```

- La ANTENA habla con la tienda (su HTTPS, su sesion) y le da a BMO-X solo
  estas lineas, como hace con las paginas (`bmo-antena/src/lamina.rs`): el
  mismo lector que dice UNA vez la linea eterna, con cuota y cuarentena.
- La tabla de MOTORES es de BMO-X, no de la tienda: `doom -> DOOM.WAD`,
  `quake -> id1/pak0.pak`... Un juego sin motor sale en la lista marcado
  "por streaming" (camino B), nunca "instalar".
- La SUMA: cada fichero llega con su sha256 y BMO-X la comprueba antes de
  guardarlo (ESTRATOS). Lo que no cuadra no entra.

## 3. Los escalones

- [ ] **J0 -- el formato, puro y con banco.** `platform/shared/bmo-ludoteca`:
      las tres lineas, la tabla de motores y el juez de la suma. **Como se
      sabe:** `cargo test -p bmo-ludoteca` en verde con lineas mutadas.
- [ ] **J1 -- la Biblioteca los muestra.** La Biblioteca del escritorio
      (`scene/data/biblioteca.rs`) lista los juegos de un fichero de lineas
      en `datos/`, con su camino (A nativo, B streaming). Sin red: el fichero
      se copia a mano. **Como se sabe:** DOOM sale como "nativo" y lanza el
      `.bex` de siempre.
- [ ] **J2 -- la antena pide la lista a GOG.** En `toolchain/tools/antena/`,
      con la sesion del propietario, como hacen Heroic y gogdl (su API no
      es publica: lo que la tienda permite es cosa del propietario de la cuenta,
      como el video en `PLAN_CLOUD_LOCAL`). **Como se sabe:** `LUDOTECA` a
      la antena devuelve las lineas de tus juegos.
- [ ] **J3 -- traer los DATOS de un juego nativo.** El WAD o PAK de un juego
      del camino A, por la antena, a ESTRATOS, con su suma. Pide TCP en el
      metal (G5). **Como se sabe:** DOOM II arranca con el `doom2.wad` que
      trajo la antena.
- [ ] **J4 -- el camino B.** Un juego de Windows lanzado en el PC y visto en
      BMO-X: es S6 (ESPEJO) con un fotograma por `gpu video`.

## 4. Lo que NO se hace

- Ningun `.exe` en BMO-X, ni Wine, ni Proton, ni una capa Win32.
- Ningun fichero de un juego en el repositorio: ni WAD ni PAK (DOOM ya lo
  cumple: `build/ejemplos.ps1`, "ni el codigo ni el WAD pueden vivir aqui").
- Nada de saltarse el DRM o las condiciones de una tienda: GOG vende sin DRM
  y es la primera por eso; Steam, solo por el camino B.
