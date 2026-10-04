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

Y para una `.maqueta` (MAQUETA 2), el navegador la pinta tal cual con
`--maqueta`, y el compilador hace su propia foto con el pintor de la casa:

```text
node toolchain/tools/espejo-cara/foto.js toolchain/tools/maqueta/pruebas/tarjeta.maqueta maqueta nav.png --maqueta
cargo run -p bmo-maqueta -- --foto toolchain/tools/maqueta/pruebas/tarjeta.maqueta casa.png
cargo run -p bmo-espejo-cara --bin espejo-cara -- comparar nav.png casa.png
```

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
| 03-10 | HERMES, mensajes (antes) | 86,94 % | 90,54 % | la letra de 8 x 16 y las medidas de antes |
| 03-10 | HERMES, mensajes | 88,09 % | 91,09 % | la letra de la casa y las columnas de la maqueta (35 / 69 / 236 / 265) |
| 04-10 | ESTADOS, `pruebas/panel.maqueta` en `reposo` / en `abierta` (`foto.js --estado`) | 99,20 / 94,80 % | 99,44 / 97,25 % | el navegador abre el bloque `@estado`; MAQUETA maqueta y juzga cada estado |
| 04-10 | PIEZAS, `pruebas/escaparate.maqueta` (la tarjeta dos veces, con `<usa>`) | 91,67 % | 93,56 % | el navegador compone con Shadow DOM y mide 788 x 276, lo mismo que MAQUETA |
| 04-10 | MAQUETA 2, `pruebas/tarjeta.maqueta` | 89,97 % | 92,25 % | la foto del anfitrion (`maqueta --foto`) contra `foto.js ... --maqueta`: la misma fuente de verdad, dos pintores |

Lo que falta para el 100 % y NO se va a hacer: la maqueta pone amigos,
fichas, asientos y charlas DE EJEMPLO (nova, orbe, faro, una OFERTA, emojis);
la app pone lo que hay de verdad. Esas zonas nunca van a ser iguales, y esta
bien: el ESPEJO mide la CARA, no inventa contenido. En HERMES, ademas, la
app lleva el REPRODUCTOR abajo (66 px) que la foto de la maqueta no tiene.

## Lo que NO es

No es un navegador dentro de BMO-X. El navegador se usa en TU PC para hacer
la foto de la maqueta, como GCC en el ESPEJO de C o GnuCOBOL en el de COBOL:
es la regla, no la pieza. En la maquina corre la app con su letra
(`bmo-letra`) y su pluma, de la casa.
