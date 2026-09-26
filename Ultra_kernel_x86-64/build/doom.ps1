# doom.ps1 -- DOOM en el build: el port, su icono, los WAD y la config.
#
# Salio de `ejemplos.ps1` el 2026-09-26, entero y sin cambiar una linea:
# con el port en el arbol (`toolchain\lang\c\expansion\doom`) aquel paso
# de las 1.000 lineas de codigo (L6a), y DOOM es una pieza con nombre propio.
#
# Se carga con `.` desde `ejemplos.ps1`, en SU ambito: usa `$repo`,
# `$dataBase`, `$SinDoom`, `Step`, `Obrero`, `Fail` y `Nuevo-Bico` de alli.

# -- DOOM: OPCIONAL, y fuera del arbol ------------------------
#
# ** POR QUE ESTE PASO SE SALTA SOLO Y NO FALLA.
#
# DOOM es GPL-2.0 y su WAD es de id Software; el arbol de BMO tiene licencia
# Techne. Ni el codigo ni el WAD pueden vivir aqui, asi que el port entero
# vive en `BMO-externo\`, al lado del repo y fuera de el.
#
# (26-09) Ahora el PORT si vive aqui (`toolchain\lang\c\expansion\doom`):
# lo que es de la casa, y lo que toca de DOOM, en `sobre\`. Las fuentes GPL
# enteras y los WAD siguen fuera de git: `traer.ps1` las trae al commit
# fijado. Ver el README de esa carpeta.
#
# Lo que SI puede vivir aqui es una RUTA. Este paso mira si el port esta; si
# no esta, dice una linea y sigue. Un `build.ps1` que fallara porque a otro
# no le apetece bajarse DOOM seria un build roto para todo el mundo menos
# para el propietario.
#
# Se puede apagar con `-SinDoom` aunque el port este puesto: compilar 56.465
# lineas cuesta lo suyo y no hace falta en cada vuelta.
#
# [!] El compilador se invoca con `cwd` = RAIZ DEL REPO (lo pone el
# `Push-Location $repo` de arriba). `Roots::find` sube desde el cwd para
# encontrar `tables\`, y desde el arbol de DOOM no la encuentra: un dia
# entero de sintomas raros salio de esto.
#
# `BMO_MODS` apunta a las cabeceras de SONDA, que tapan a las del sistema
# para lo que DOOM incluye y BMO no tiene (`<direct.h>`, `<io.h>`...). Las
# que el repo si implementa quedaron apartadas como `*.h.sonda-vieja`
# justamente para que NO tapen: una cabecera que solo declara esconde a la
# que tiene cuerpo.
if (-not $SinDoom) {
    $doomRaiz  = Join-Path (Split-Path -Parent $repo) 'BMO-externo'
    $doomFte   = Join-Path $doomRaiz 'doom\doomgeneric\doomgeneric\doomgeneric_bmo.c'
    $doomWad   = Join-Path $doomRaiz 'doom\doom1.wad'
    $doomInc   = Join-Path $doomRaiz 'doom-port\include'
    # ** DESDE EL 26-09, DOOM PUEDE VIVIR EN EL ARBOL, y si esta, manda.
    #
    # `toolchain\lang\c\expansion\doom\traer.ps1` deja tres cosas: las
    # fuentes GPL al commit de FUENTES.txt en `fuentes\` (fuera de git), lo
    # que el port les cambia o agrega en `sobre\` y los stubs en `cola\`.
    # Aqui se arma la OBRA: una copia limpia de las fuentes con `sobre\`
    # encima, cada build desde cero -- asi un fichero borrado de `sobre\` no
    # sobrevive en la obra. Si el arbol no lo tiene, BMO-externo como antes.
    $doomExp   = Join-Path $repo 'toolchain\lang\c\expansion\doom'
    $doomLimpio = Join-Path $doomExp 'fuentes\doomgeneric'
    $doomSobre = Join-Path $doomExp 'sobre'
    if ((Test-Path (Join-Path $doomSobre 'doomgeneric\doomgeneric_bmo.c')) -and
        (Test-Path (Join-Path $doomLimpio 'doomgeneric\doomgeneric.h'))) {
        $obra = Join-Path $doomExp 'fuentes\obra'
        if (Test-Path $obra) { Remove-Item -Recurse -Force $obra }
        foreach ($desde in $doomLimpio, $doomSobre) {
            Get-ChildItem -LiteralPath $desde -Recurse -File | ForEach-Object {
                $rel = $_.FullName.Substring($desde.Length).TrimStart('\')
                if (('\' + $rel) -match '\\\.git\\') { return }
                $dst = Join-Path $obra $rel
                New-Item -ItemType Directory -Force -Path (Split-Path -Parent $dst) | Out-Null
                Copy-Item -LiteralPath $_.FullName -Destination $dst -Force
            }
        }
        $doomFte = Join-Path $obra 'doomgeneric\doomgeneric_bmo.c'
        $doomInc = Join-Path $doomExp 'cola\include'
        Write-Host '    [doom] el port del ARBOL (expansion\doom), fuentes fijadas' -ForegroundColor DarkGray
    }
    $doomWadArbol = Join-Path $doomExp 'wad\doom1.wad'
    if (Test-Path $doomWadArbol) { $doomWad = $doomWadArbol }
    if (-not (Test-Path $doomFte)) {
        Write-Host '    [doom] BMO-externo no esta: se salta (es GPL, vive fuera del repo)' -ForegroundColor DarkGray
    } elseif (-not (Test-Path $doomWad)) {
        Write-Host '    [doom] falta doom1.wad: se compila igual, pero no habra con que jugar' -ForegroundColor Yellow
    }
    if (Test-Path $doomFte) {
        Step 'Building DOOM (opcional, GPL, fuera del arbol)'
        $doomDst = Join-Path (Join-Path $dataBase 'apps') 'doom.bex'
        $modsPrevio = $env:BMO_MODS
        $env:BMO_MODS = $doomInc
        try {
            $out = & (Obrero bmo-c-x86-64) $doomFte -o $doomDst 2>&1
            $out | ForEach-Object {
                if ($_ -match 'ok:|error') { Write-Host ('    [doom] ' + $_) -ForegroundColor DarkGray }
            }
            if ($LASTEXITCODE -ne 0) { Fail 'DOOM no compilo' }
            if (-not (Test-Path $doomDst)) { Fail 'DOOM compilo y no salio doom.bex' }
        } finally {
            # Se devuelve SIEMPRE. `BMO_MODS` tapa cabeceras del sistema, y
            # dejarlo puesto haria que el paso siguiente compilara contra
            # las de sonda sin que nadie lo pidiera.
            $env:BMO_MODS = $modsPrevio
        }
        # ** LA CARA DE DOOM, DENTRO DE DOOM.
        #
        # El icono es un recurso mas del paquete, y por eso el escritorio no
        # necesita ni un `.lnk` que apunte aqui ni una cache de iconos que
        # reconstruir: copias el `.bex` y va con su cara. Ver
        # `escena\lanzador.rs`.
        #
        # Se dibuja aqui y no se trae de fuera **porque una imagen de DOOM
        # seria de id Software**. Estos 256 pixeles son originales, y por
        # eso pueden vivir en este fichero mientras el resto del port no.
        $doomIco = Join-Path $env:TEMP 'bmo-doom-icono'
        [System.IO.File]::WriteAllBytes($doomIco, (Nuevo-Bico @(
            '................',
            '.....RRRRRR.....',
            '...RRRRRRRRRR...',
            '..RRRRRRRRRRRR..',
            '..RRRRRRRRRRRR..',
            '..RRoooRRoooRR..',
            '..RRoWoRRoWoRR..',
            '..RRoooRRoooRR..',
            '..RRRRRRRRRRRR..',
            '..RRRddddddRRR..',
            '..RRdWdWdWdWdR..',
            '..RRddddddddRR..',
            '...RRRRRRRRRR...',
            '....RRRRRRRR....',
            '................',
            '................'
        )))
        $out = & (Obrero bmo-pack) $doomDst '-r' ('icono=' + $doomIco) '-o' $doomDst 2>&1
        $out | ForEach-Object {
            if ($_ -match 'recurso\(s\)' -or $_ -match '\[X\]') {
                Write-Host ('    [doom] ' + $_) -ForegroundColor DarkGray
            }
        }
        if ($LASTEXITCODE -ne 0) { Fail 'no se pudo meter el icono en doom.bex' }
        Remove-Item -Force $doomIco -ErrorAction SilentlyContinue

        # El WAD va al lado, tal cual. **No se empaqueta dentro del `.bex`**
        # aunque el formato lo permita: `lanzar.rs::con_buffer` se trae el
        # fichero ENTERO a un bufer de 4 MiB, asi que un paquete de 5,5 MB
        # no arrancaria. Ver el escalon 2 de `docs\identidad\LA_RAM.md` -- el dia que
        # el cargador lea solo lo cargable, esto pasa a ser un `bmo-pack`.
        if (Test-Path $doomWad) {
            $wadDst = Join-Path (Join-Path $dataBase 'apps') 'doom1.wad'
            Copy-Item -LiteralPath $doomWad -Destination $wadDst -Force
            Write-Host ('    [doom] doom1.wad (' + (Get-Item $wadDst).Length + ' B) -> apps\') -ForegroundColor DarkGray
        }
        # ** L0 de PLAN_LA_LUDOTECA (25-09): FREEDOOM, un DOOM entero y LIBRE
        # (BSD), para el mismo `doom.bex`. Si esta en `BMO-externo\doom\`
        # (freedoom.github.io), va al lado con un nombre 8.3 FIJO: el FAT32 de
        # BMO-X busca por nombre corto, y `freedoom1.wad` no lo es -- Windows
        # le inventaria `FREEDO~1.WAD`, y ese nombre no lo sabe nadie.
        # Que `doom.bex` lo ABRA depende del port (fuera del arbol): se
        # prueba en el Ryzen. Sin los WAD, este paso no dice nada.
        foreach ($n in 1, 2) {
            $libre = Join-Path $doomExp ('wad\freedoom' + $n + '.wad')
            if (-not (Test-Path $libre)) {
                $libre = Join-Path $doomRaiz ('doom\freedoom' + $n + '.wad')
            }
            if (Test-Path $libre) {
                $libreDst = Join-Path (Join-Path $dataBase 'apps') ('freedm' + $n + '.wad')
                Copy-Item -LiteralPath $libre -Destination $libreDst -Force
                Write-Host ('    [doom] freedoom' + $n + '.wad (Freedoom, BSD) -> apps\freedm' + $n + '.wad') -ForegroundColor DarkGray
            }
        }
        # ** LA CONFIGURACION: `default.cfg` en la RAIZ del volumen, que es
        # donde DOOM la busca (`./default.cfg`, METAL_2026-08-13). Solo si NO
        # esta: al salir, DOOM guarda ahi la del jugador, y un build que la
        # pisara le borraria sus ajustes cada vez.
        $doomCfg = Join-Path $doomExp 'default.cfg'
        $cfgDst  = Join-Path $dataBase 'default.cfg'
        if ((Test-Path $doomCfg) -and -not (Test-Path $cfgDst)) {
            Copy-Item -LiteralPath $doomCfg -Destination $cfgDst
            Write-Host '    [doom] default.cfg -> raiz (la primera vez; luego es del jugador)' -ForegroundColor DarkGray
        }
        # El consejo de aqui decia "desde el shell de Ring 0", y eso era
        # cierto mientras `lend_screen` tenia un plazo de 500 ms: DOOM tarda
        # ~10 s en reclamar la pantalla, el escritorio se cansaba y se la
        # quedaba. Arreglado -- ahora la espera es por VIDA y no por reloj,
        # asi que el camino bueno es el ICONO: es el unico que devuelve la
        # pantalla al escritorio cuando el programa muere. Por el shell de
        # Ring 0 no hay quien la recupere y se acaba en el panel del kernel.
        Write-Host '    [doom] lanzalo con:  CLIC en su icono del escritorio' -ForegroundColor DarkGray
        Write-Host '    [doom]   (`run apps/doom.bex` desde Ring 0 tambien va, pero al morir' -ForegroundColor DarkGray
        Write-Host '    [doom]    la pantalla NO vuelve al escritorio: se queda el kernel)' -ForegroundColor DarkGray
    }
}
