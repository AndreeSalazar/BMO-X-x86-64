# Ambitos: antecedente integrado del inventario GPU

El VERIFICAR del propietario se detuvo por el commit
`91eaac2f24349d080444f258c5576456f6374695`, cuyo asunto es
`Prepare GA106 HS research and offline firmware inventory`.
Fue introducido por Codex en la PR #63 y ya esta integrado en main.
Incumple el formato `tipo(ambito): algo`. Un asunto adecuado habria sido
`feat(gpu): preparar inventario de firmware y evidencia HS`.

No se reescribe el historial compartido. Se aplica el mecanismo de linea
base que documenta `ambitos.py`, con un corte menor que sellar HEAD.

## Revision antes de cambiar la base

| Dato | Valor |
| --- | --- |
| Base anterior | `d4f5d5720dc65a27794b307f4a48428343fce3c9` |
| HEAD revisado | `4d05236ead385f0253a4b35143c76d9c4184b029` |
| Commits juzgados con tipos, ambitos, alias y ASCII vigentes | 80 |
| Rechazados | 1: el asunto de `91eaac2f2434` |
| Nueva base | `91eaac2f24349d080444f258c5576456f6374695` |
| Commits posteriores que siguen juzgados | 12, todos validos |

Se comprobo que ambas bases existen como commits y son ancestros del HEAD
revisado antes de contar. No se acepta una salida vacia por falta de historial.
La nueva base excluye 68 commits del rango anterior; 67 de ellos pasaron y
el restante es el antecedente registrado arriba. No se omiten otros rechazos.

La lista de ambitos, los alias, los tipos y el codigo del guardian permanecen
iguales. Los nuevos commits siguen necesitando un asunto valido. El mensaje
exacto rechazado sigue siendo rechazado por `--msg`; mover la base no lo
convierte en un formato admitido.

## Comprobaciones

- `ambitos.py --check` con el rango restante.
- `--msg` rechaza el asunto antiguo y acepta el asunto de esta correccion.
- Una prueba con ambito inexistente sigue siendo rechazada.
- ASCII, enlaces y `git diff --check`.

Esto corrige el bloqueo del historial. No es una prueba de la GPU ni del
build completo; el propietario debe repetir VERIFICAR tras integrar la PR.
