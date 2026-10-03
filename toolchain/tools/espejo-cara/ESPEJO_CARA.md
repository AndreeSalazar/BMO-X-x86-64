# ESPEJO DE CARA

La app de BMO-X contra su maqueta, **medido**. El propietario (03-10):
*"HTML y CSS, esos dos son motivos: me gustaria que mi BMO-X refleje las
maquetas que hiciste... que sea IGUAL"*.

## Las tres piezas

```text
   1. la REGLA      foto.js: la maqueta en el navegador, foto de su ventana
   2. la CARA       cara-bankcat / cara-hermes: la app pintada por SU codigo
                    (los mismos ficheros, por #[path]), en el anfitrion
   3. el ESPEJO     espejo-cara comparar: cuanto se parecen y DONDE no
```

## Como se usa

```text
node toolchain/tools/espejo-cara/foto.js docs/arte/maqueta_bankcat.html .ventana maqueta.png
cargo run -p bmo-espejo-cara --bin cara-bankcat -- salida/
cargo run -p bmo-espejo-cara --bin espejo-cara -- comparar maqueta.png salida/cartera.png --mapa diff.png
```

`diff.png` es la app apagada con lo distinto en rojo. Las zonas peores salen
primero: es la lista de trabajo.

## La tinta: un sitio para los colores

```text
cargo run -p bmo-espejo-cara --bin espejo-cara -- tinta docs/arte/maqueta_bankcat.html > Ultra_userspace/apps/bankcat/src/tinta.rs
cargo run -p bmo-espejo-cara --bin espejo-cara -- tinta docs/arte/maqueta_bankcat.html --comprobar Ultra_userspace/apps/bankcat/src/tinta.rs
```

El `:root` de la maqueta es la FUENTE; `tinta.rs` se genera. Un color que
cambia en la maqueta cambia en la app al regenerar, y `--comprobar` falla si
alguien edito uno de los dos a mano.

## Lo medido

| fecha | app | igual | parecido | que cambio |
|---|---|---|---|---|
| 03-10 | BANK CAT, cartera | 92,04 % | 94,85 % | la letra de la casa (`bmo-letra`), el gato trazo a trazo de su SVG, las medidas del navegador |

Lo que falta para el 100 % y NO se va a hacer: la maqueta pone amigos,
fichas y asientos DE EJEMPLO; la app pone lo que el motor contesto. Esas
zonas (la lista de la izquierda) nunca van a ser iguales, y esta bien.

## Lo que NO es

No es un navegador dentro de BMO-X. El navegador se usa en TU PC para hacer
la foto de la maqueta, como GCC en el ESPEJO de C o GnuCOBOL en el de COBOL:
es la regla, no la pieza. En la maquina corre la app con su letra
(`bmo-letra`) y su pluma, de la casa.
