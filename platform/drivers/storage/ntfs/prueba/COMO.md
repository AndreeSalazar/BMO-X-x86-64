# Los discos del banco de NTFS

Dos imagenes de 4 MiB hechas por NTFS DE VERDAD (`mkntfs` y `ntfs-3g`, de
`ntfs-3g` 2022.10.3, Ubuntu), no por este lector: el banco
(`platform/drivers/storage/ntfs/src/pruebas.rs`) comprueba que se leen como
las escribio Linux. Hechas el 29-09.

Los contenidos siguen `patron(n, k)`: el byte `i` vale
`(i*31 + k*7 + (i >> 9)) & 0xFF`.

## `disco.ntfs` (clusteres de 4 KiB)

```sh
truncate -s 4M disco.ntfs
mkntfs -F -q -Q -s 512 -c 4096 -L PRUEBA disco.ntfs
ntfs-3g disco.ntfs mnt
```

y dentro (Python, en `mnt/`):

```text
bin/x64/Cyberpunk2077.exe                       patron(70000, 1)
archive/pc/content/basegame_4_gamedata.archive  patron(300000, 2)
hola.txt                                        "hola desde NTFS\n" (residente)
vacio.bin                                       0 bytes
Cafe nandu.txt, con acentos: U+00E9, U+00F1, U+00FA   "unicode\n"
r6/scripts/script_numero_NNN_con_un_nombre_bastante_largo.reds, NNN = 000..079
                                                "n=N\n" (80: bloques INDX)
frag/rNNN.bin   patron(16384, NNN) hasta llenar el disco (120), y se BORRAN
                los pares: quedan 60 vivos y 60 entradas viejas
frag/partido.bin                                patron(200000, 3): dos tramos
```

## `lista.ntfs` (clusteres de 512 B)

```sh
truncate -s 4M lista.ntfs
mkntfs -F -q -Q -s 512 -c 512 -L LISTA lista.ntfs
ntfs-3g lista.ntfs mnt
```

`disperso.bin`: 800 veces, los 512 bytes `i*1024 .. i*1024+512` de
`patron(800*1024, 5)` escritos con `pwrite` en su sitio, y el fichero llevado
a 800 KiB con `ftruncate`. Queda disperso (sparse): 1606 tramos, 801 de ellos
huecos, que no caben en un registro -- `ntfsinfo` lo muestra con
`$ATTRIBUTE_LIST` y su `$DATA` en los registros 64, 66, 67, 68 y 69.
