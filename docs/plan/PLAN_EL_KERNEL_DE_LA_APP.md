# PLAN EL KERNEL DE LA APP -- cada app se administra sola; el kernel principal arbitra

> El propietario (15-10): *"el kernel propio pero en archivo, es kernel
> propio de la APP y empieza a organizar inspirado en mi kernel pero no es
> el kernel principal sino que organiza [...] son datos en .bex en general
> en una carpeta que guarda y mi kernel principal deja que el kernel de la
> APP administre todo, es decir, al ser validado ese mini kernel de la app,
> SABE que hacer [...] mi kernel principal se activa cuando algo no
> cuadra"*.
>
> **Es un plan LARGO y no es para ahora.** Va ENCIMA de
> [`PLAN_EL_TOQUE.md`](PLAN_EL_TOQUE.md): sin cargar al tocar, no hay nada
> que administrar.

---

## 0. La respuesta corta

```text
   que es        una APP es una CARPETA: un `.bex` que es su KERNEL (el que
                 organiza) y un `.bex` por FUNCIONALIDAD, cada una con su
                 peso y su contrato declarados. El kernel de la app decide
                 cuando vive cada una: la carga al tocarla, la trae antes
                 si el puntero se acerca, la suelta si se enfria
   el kernel     VALIDA una vez (la lente de B1, la firma, el contrato de
   principal     cada pieza), PRESTA de golpe (un bloque de RAM, un tramo
                 del disco) y se APARTA. Vuelve solo cuando algo no cuadra
   como se       NO preguntando en cada paso: el HARDWARE vigila por el.
   "activa       Lo que la app no tiene, la MMU no se lo deja tocar (un
   cuando algo   fallo de pagina despierta al kernel); lo que un aparato
   no cuadra"    no tiene, la IOMMU no se lo deja tocar. Es gratis mientras
                 todo cuadra, y el kernel solo corre cuando la CPU lo llama
   de donde      exokernel y library OS (MIT, Engler y Kaashoek, 1995): el
   viene         kernel protege y reparte, cada app trae su propio "sistema"
                 como biblioteca. Unikernels y los componentes de Fuchsia
                 son primos. Ninguno lo hizo con piezas que se cargan al
                 tocarlas y contratos por pieza: eso es de BMO-X
```

---

## 1. Por que es MAS fino que una DLL

```text
                 DLL (Windows) / .so (Linux)     una pieza de BMO-X
   se carga      entera, al arrancar o al        al TOCARLA (EL_TOQUE), o
                 pedirla                         antes si el puntero avisa
   declara       sus simbolos                    su PESO, lo que pide (RAM,
                                                 pantalla, 3060, disco) y
                                                 su contrato
   la comparten  todos, a ciegas                 la app que la trae; otra
                                                 app, solo si la pieza lo
                                                 declara
   se va         cuando nadie la usa (y casi     cuando se enfria: la decide
                 nunca)                          el kernel de la app
   la vigila     nadie despues de cargarla       la MMU y la IOMMU, siempre
```

---

## 2. Que impacto tiene (y que NO cambia)

- **Menos puertas al kernel.** Hoy cada cosa que una app pide cruza la
  puerta (~720 ticks de techo medido en el Ryzen, `INFO_PRESUPUESTO_PUERTA`).
  Si el kernel presta un BLOQUE y el kernel de la app lo reparte dentro,
  mil peticiones chicas son UNA puerta. Es lo que
  [`PLAN_LA_PUERTA_SE_PARTE.md`](PLAN_LA_PUERTA_SE_PARTE.md) mide con sus
  lotes, llevado a toda la app.
- **Cada app a su manera.** CraftPho administra mosaicos de 64 x 64;
  ILLAPA, particulas; ECCE, texto. Cada kernel de app sabe lo suyo mejor
  que un kernel general.
- **Un fallo se queda en su app.** Si el kernel de una app se equivoca,
  se rompe ESA app: la MMU no le deja tocar lo de otra. El kernel principal
  sigue siendo el unico que manda en el hardware.
- **Lo que NO cambia:** el kernel principal sigue siendo imprescindible.
  Solo el reparte marcos fisicos, habla con los aparatos y decide quien
  corre. "No cuesta" es casi verdad mientras todo cuadra -- el hardware
  vigila gratis --, pero cada fallo de pagina (cargar al tocar) SI cuesta
  una entrada al kernel: por eso EL6 trae antes.

---

## 3. La escalera

- [x] **KA-1 -- VER POR DENTRO** (el propietario, 15-10: *"click derecho y
      ver todo lo que hay adentro [...] es el lente de mi Kernel"*). HECHO
      en el anfitrion: `platform/abi/bmo-radiografia` (pura, sin monton)
      junta las TRES piezas con las que el kernel admite un programa -- la
      puerta (`bmo-bex-gate`), los papeles (`bmo-carga-juicio`) y la lente
      (`bmo-lente-x86`) -- sobre el fichero entero, y lee el MANIFIESTO
      (lo que el compilador declara: modulos, permisos, desde que linea usa
      cada puerta). En el escritorio: `dentro RUTA` en Ejecutar, y en el
      explorador (DATOS y EFI) clic derecho sobre un `.bex` o `.ibx` > "ver
      por dentro". Dice si el kernel lo ADMITIRIA. Es la herramienta con la
      que se leera cada pieza de KA1..KA7. Pruebas: ECCE por dentro (puerta
      SI, lente DEMOSTRADO, manifiesto con `screen = true`), uno roto (la
      puerta dice su NO por nombre) y sin sitio para la lente. Falta: verlo
      en el Ryzen (hoja 6b de EL_FOCO, fila RX).
- [x] **KA0 -- las decisiones de la seccion 5**, contestadas por el propietario
      el 15-10: se llama **NUCLEO** (`apps/<app>/nucleo.bex`), la primera
      app partida es **ECCE**, y **cada app trae sus piezas** (compartir,
      despues y si hace falta).
- [ ] **KA1 -- la APP CARPETA:** `apps/ecce/` con `nucleo.bex` y piezas
      `.bex`; el lanzador ve la carpeta como UNA app (un icono) y el
      cargador valida TODAS las piezas al admitir (B1, la firma, A6).
- [ ] **KA2 -- el CONTRATO de una pieza:** su peso, lo que pide (los
      permisos del `Titan.toml`, por pieza) y lo que ofrece. El kernel
      principal rechaza una pieza que pide mas de lo que la app tiene.
- [ ] **KA3 -- PRESTAR DE GOLPE:** un bloque de RAM y un tramo de
      ESTRATOS a la app, una vez; su kernel lo reparte sin puertas. **Como
      se sabe:** las puertas por segundo de una app, antes y despues.
- [ ] **KA4 -- el kernel de la app CARGA AL TOCAR** sus piezas (sobre EL1
      y EL2 de EL_TOQUE) y las suelta frias (EL4).
- [ ] **KA5 -- el puntero avisa** (EL6) dentro de la app: el kernel de la
      app sabe que boton lleva a que pieza.
- [ ] **KA6 -- "cuando algo no cuadra":** el fallo de una pieza (un `#PF`
      fuera de lo suyo, un contrato roto) despierta al kernel principal,
      que cierra la PIEZA -- no la app entera, si su kernel lo pide -- y lo
      apunta en la autopsia (B2).
- [ ] **KA7 -- TITAN++ escribe piezas:** `mod` que sale a su propio
      `.bex`, con el contrato en la cabecera del paquete.

---

## 4. Lo que puede salir mal, dicho antes

- **Validar una vez no es confiar para siempre:** una pieza cargada
  despues (KA4) se valida al cargarse, no al abrir la app.
- **Un kernel de app con un fallo puede perder los datos de SU app** (el
  documento sin guardar). Por eso lo que importa se guarda solo (como
  ECCE).
- **Muchas piezas chicas = muchos ficheros en ESTRATOS:** el disco tiene
  que leerlas de varias a la vez (la cola de 32 de EL6).

---

## 5. Las decisiones del propietario

```text
   D1  EL NOMBRE        CONTESTADA (15-10): NUCLEO -- `apps/<app>/nucleo.bex`
   D2  LA PRIMERA APP   CONTESTADA (15-10): ECCE -- chica, ya funciona y se ve
                        rapido (el deshacer y el guardado, piezas aparte)
   D3  COMPARTIR        CONTESTADA (15-10): cada app trae las suyas. Mas
                        simple y el fallo de una pieza solo toca a su app;
                        compartir con contrato se puede abrir despues
```

---

## 6. De donde sale

- Engler, Kaashoek y O'Toole, *Exokernel: An Operating System Architecture
  for Application-Level Resource Management* (SOSP 1995); Madhavapeddy y
  otros, *Unikernels* (ASPLOS 2013); la documentacion de componentes de
  Fuchsia. Citados de memoria: se leen antes de escribir la primera linea.
- Lo de BMO-X: `PLAN_EL_TOQUE.md`, `PLAN_LA_PUERTA_SE_PARTE.md`, la lente
  al admitir (B1), el `.bex` que declara lo que usa (A6) y las laminas con
  contrato (A1..A4 de EL_FOCO).
