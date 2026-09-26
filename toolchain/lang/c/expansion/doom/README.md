# DOOM, en el arbol -- la expansion de BMO C

> Pedido por el propietario (2026-09-26): *"meter TODO el DOOM completo con
> configuracion y eso para empezar, porque ya es hora de meter mi VERRANO con
> mi API"*. Es la salida **b)** de [`../../README.md`](../../README.md)
> (seccion 4.1): la cola del port, versionada en el arbol, con un puntero a
> las fuentes GPL.

---

## 1. Que hay aqui

```text
   FUENTES.txt   de donde salen las fuentes GPL y a QUE COMMIT (fijado)
   traer.ps1     trae las fuentes, muda el port de BMO-externo y baja Freedoom
   default.cfg   la configuracion de la primera partida
   sobre/        lo que el port CAMBIA o AGREGA a doomgeneric, con su ruta:
                 doomgeneric/doomgeneric_bmo.c y cada fichero de DOOM tocado
   cola/         lo que era BMO-externo/doom-port: los stubs de include/
                 (lo que $BMO_MODS hace ganar), unity.py, las sondas

   fuentes/      (fuera de git) doomgeneric al commit fijado, y la OBRA que
                 arma el build: una copia limpia con sobre/ encima
   wad/          (fuera de git) los WAD
```

`sobre/` y `cola/` se llenan **una vez**, en la maquina donde el port ya vive:

```powershell
   toolchain\lang\c\expansion\doom\traer.ps1 -Mudar -Freedoom
   git status          # lo mudado, a la vista
   git commit ...      # desde hoy el port vive aqui
```

`-Mudar` solo copia a `sobre/` lo que **difiere** del commit fijado (mismo
contenido = no se copia), y no pisa nada que ya este en el arbol salvo con
`-Forzar`. Despues, en cualquier maquina, `traer.ps1` a secas trae las fuentes
y `build.ps1` compila como siempre: `build/ejemplos.ps1` mira primero aqui y,
si aqui no esta, sigue mirando `BMO-externo\` como antes.

## 2. Los WAD

| fichero en el volumen | que es | licencia |
|---|---|---|
| `apps/doom1.wad` | DOOM shareware, episodio 1 | de id Software, se reparte con sus terminos |
| `apps/freedm1.wad` | Freedoom: Phase 1 (como DOOM) | BSD |
| `apps/freedm2.wad` | Freedoom: Phase 2 (como DOOM II) | BSD |

Los nombres de Freedoom son **8.3 fijos**: el FAT32 de BMO-X busca por nombre
corto, y `freedoom1.wad` no lo es. Cual abre `doom.bex` lo decide el port, con
el `-iwad` que se arma en `doomgeneric_bmo.c`. doomgeneric reconoce el juego
por sus lumps (`E1M1` o `MAP01`), no por el nombre, asi que el mismo `.bex`
sirve para los tres.

## 3. La configuracion

`default.cfg` va a la **raiz** del volumen, que es donde DOOM la busca
(`./default.cfg`, visto en `docs/metal/METAL_2026-08-13.md`), y **solo si no
esta**: al salir, DOOM guarda ahi la del jugador, y un build que la pisara le
borraria sus ajustes cada vez.

Lleva solo numeros (volumen, medida de la pantalla, detalle, gamma...). **Las
teclas no**: DOOM las guarda como codigos de exploracion y no como las
`KEY_*` de `doomkeys.h`; un numero mal puesto ahi deja a alguien sin poder
disparar, y lo que no esta en el fichero se queda con el valor de fabrica.

## 4. Si DOOM "se cierra"

Sin `fallo de Ring 3` en el informe, DOOM **no se estrello: salio**. En DOOM
eso es casi siempre `I_Error`, que imprime un motivo y llama a `exit`. Los
tres que mas se ven:

```text
   IWAD file '...' not found!           no encontro el WAD que pide -iwad
   Game mode indeterminate. ...         no hay -iwad y no encontro ninguno
   Z_Malloc: failed on allocation ...   la ZONA (el bloque que DOOM pide al
                                        arrancar) no alcanza. doomgeneric
                                        pide 6 MiB por defecto (`i_system.c`,
                                        DEFAULT_RAM) y Freedoom tiene mapas y
                                        texturas mas grandes que el
                                        shareware: el primer sospechoso si
                                        solo falla con freedm1/freedm2. Se
                                        sube con `-mb 16` en el argv del port
   R_TextureNumForName: ... not found   un WAD que no casa con lo que el
                                        juego espera
```

(Los cuatro, comprobados en las fuentes del commit fijado: `d_iwad.c`,
`d_main.c`, `z_zone.c`, `r_data.c`.)

La linea exacta sale por la consola del escritorio. Con ella se sabe cual es.

## 5. Y VERRANO

DOOM pinta por software en un bufer de 32 bits (`DG_ScreenBuffer`), y el
escalado a la ventana lo hace hoy la CPU. VERRANO entra por ahi: ese bufer como
TEXTURA en la 3060 y dos triangulos que la estiran a la ventana. Es el carril
**D** de [`PLAN_VERRANO.md`](../../../../../docs/plan/PLAN_VERRANO.md), y pide M3
(las texturas) y M6 (un juego de otro proceso).

## 6. Licencia

`sobre/` y `cola/` son **GPL-2.0**: son ficheros de DOOM, o existen solo para
compilarse junto a DOOM. No los cubre la licencia Apache-2.0 del resto del
arbol (ver `NOTICE`). Las fuentes de doomgeneric no se copian aqui: se traen al
commit fijado. `README.md`, `FUENTES.txt`, `traer.ps1` y `default.cfg` son de
la casa.
