# traer.ps1 -- DOOM ENTERO en el arbol: las fuentes fijadas, la mudanza del
# port y los WAD libres. Lo lee `README.md` de esta carpeta; lo usa
# `Ultra_kernel_x86-64/build/ejemplos.ps1`.
#
#   .\traer.ps1              trae las fuentes GPL al commit de FUENTES.txt
#   .\traer.ps1 -Mudar       y ademas muda el port de ..\BMO-externo al arbol
#   .\traer.ps1 -Freedoom    y ademas baja Freedoom (BSD) a wad\
#
# ** Nada de aqui se ejecuta solo en el build: el build solo LEE lo que este
# guion dejo. Bajar de internet a mitad de un build es un build que un dia
# falla por la red y nadie sabe por que.

param(
    [switch]$Mudar,
    [switch]$Freedoom,
    # Pisa lo que ya hubiera en sobre\ y cola\. Sin esto, la mudanza no toca
    # nada que ya este en el arbol: lo del arbol es lo que manda.
    [switch]$Forzar
)

$ErrorActionPreference = 'Stop'
$aqui     = $PSScriptRoot
$repo     = (Resolve-Path (Join-Path $aqui '..\..\..\..\..')).Path
$externo  = Join-Path (Split-Path -Parent $repo) 'BMO-externo'
$fuentes  = Join-Path $aqui 'fuentes'
$prist    = Join-Path $fuentes 'doomgeneric'
$sobre    = Join-Path $aqui 'sobre'
$cola     = Join-Path $aqui 'cola'
$wad      = Join-Path $aqui 'wad'

function Linea($clave) {
    $l = Get-Content (Join-Path $aqui 'FUENTES.txt') |
        Where-Object { $_ -match ('^' + $clave + '\s+') } | Select-Object -First 1
    if (-not $l) { throw "FUENTES.txt no dice '$clave'" }
    return ($l -split '\s+', 2)[1].Trim()
}

# -- 1. Las fuentes GPL, AL COMMIT FIJADO -----------------------------------
#
# Se trae SOLO ese commit (`fetch --depth 1` de un SHA: GitHub lo sirve). Si
# ya estan y el commit no es el de FUENTES.txt, se dice y NO se pisan: puede
# ser alguien probando otro a proposito.
$url    = Linea 'repo'
$commit = Linea 'commit'
if (-not (Test-Path (Join-Path $prist '.git'))) {
    Write-Host "[doom] fuentes: $url @ $($commit.Substring(0, 7))"
    New-Item -ItemType Directory -Force -Path $prist | Out-Null
    git -C $prist init -q
    git -C $prist remote add origin $url
    git -C $prist fetch -q --depth 1 origin $commit
    if ($LASTEXITCODE -ne 0) { throw 'no se pudo traer el commit fijado' }
    git -C $prist checkout -q FETCH_HEAD
} else {
    $tiene = (git -C $prist rev-parse HEAD).Trim()
    if ($tiene -ne $commit) {
        Write-Host "[doom] [!] fuentes\doomgeneric esta en $tiene, FUENTES.txt dice $commit" -ForegroundColor Yellow
        Write-Host '[doom]     no se toca. Para volver: borra fuentes\ y repite.' -ForegroundColor Yellow
    } else {
        Write-Host "[doom] fuentes: ya estan, @ $($commit.Substring(0, 7))" -ForegroundColor DarkGray
    }
}

# -- 2. LA MUDANZA: el port de BMO-externo pasa al arbol ---------------------
#
# Dos cosas distintas, y cada una a su carpeta:
#
#   sobre\   lo que el port CAMBIA o AGREGA al arbol de doomgeneric, con su
#            misma ruta: doomgeneric\doomgeneric_bmo.c y cada fichero de DOOM
#            que se toco para cazar fallos (el RANGECHECK, los censos...).
#            Solo lo que difiere del commit fijado: el build lo pone ENCIMA
#            de una copia limpia y sale el mismo arbol que habia fuera
#   cola\    BMO-externo\doom-port entero: los stubs de `include\` (lo que
#            `$BMO_MODS` hace ganar), unity.py, las sondas
#
# ** Lo que NO se muda (26-09, lo encontro la primera mudanza de verdad):
#   - fuera de `doomgeneric\` del arbol de doomgeneric: su `.gitignore` (que
#     dice `doomgeneric` y hacia a git ignorar el port ENTERO), el `.sln`, sus
#     README y LICENSE. Son del repo de arriba, no del port
#   - lo GENERADO: `out\` (la musica renderizada, ~86 MB de .wav que ademas
#     pueden salir del WAD comercial), objetos, ejecutables, WAD
#   - los RESTOS: `*.antes`, `*.orig`, `*.bak`, y las `*.h.sonda-vieja` que se
#     apartaron para que NO taparan a las de la fabrica
# Y se compara SIN los finales de linea: un checkout de Windows (CRLF) y uno
# de Linux (LF) del mismo fichero son el mismo fichero (ver `Huella`).
$restos = '\\\.git\\|\\__pycache__\\|\\out\\|\.(o|obj|exe|bex|bo|wad|pdb|ilk|pyc|wav|mp3|ogg|antes|orig|bak|sonda-vieja)$|(^|\\)\.gitignore$'

# ** Todo fichero que no sea BINARIO (sin un byte cero) se compara sin sus
# CR antes de LF. Primero se decidia por la extension, y `Makefile.sdl` o un
# `.vcxproj` se colaban como "del port" solo por venir en CRLF (26-09, nueve
# de 27). Latin-1 es uno a uno con los bytes: nada se pierde al pasar a texto.
function Huella($f) {
    $b = [IO.File]::ReadAllBytes($f)
    if ([Array]::IndexOf($b, [byte]0) -lt 0) {
        $l1 = [Text.Encoding]::GetEncoding(28591)
        $b = $l1.GetBytes(($l1.GetString($b) -replace "`r`n", "`n"))
    }
    $sha = [Security.Cryptography.SHA256]::Create()
    return [BitConverter]::ToString($sha.ComputeHash($b))
}

if ($Mudar) {
    $suyo = Join-Path $externo 'doom\doomgeneric'
    if (-not (Test-Path $suyo)) { throw "no esta ${suyo}: nada que mudar" }
    # ** DE QUE COMMIT PARTE EL PORT. Si el de fuera es un clon, lo dice su
    # HEAD; y si no es el de FUENTES.txt, comparar contra el fijado copiaria
    # como "del port" todo lo que cambio arriba entre los dos. Se para.
    if (Test-Path (Join-Path $suyo '.git')) {
        $base = (git -C $suyo rev-parse HEAD).Trim()
        if ($base -ne $commit) {
            Write-Host "[doom] [!] el port de fuera parte de $base" -ForegroundColor Yellow
            Write-Host "[doom]     y FUENTES.txt fija $commit" -ForegroundColor Yellow
            Write-Host '[doom]     Pon ese commit en FUENTES.txt, borra fuentes\ (y sobre\ y cola\ si ya' -ForegroundColor Yellow
            Write-Host '[doom]     los habia) y repite: asi sobre\ es SOLO lo que el port cambio.' -ForegroundColor Yellow
            throw 'el commit base del port no es el fijado'
        }
    }
    $n = 0; $iguales = 0; $fuera = 0
    Get-ChildItem -LiteralPath $suyo -Recurse -File -Force | ForEach-Object {
        $rel = $_.FullName.Substring($suyo.Length).TrimStart('\')
        if (-not $rel.StartsWith('doomgeneric\') -or ('\' + $rel) -match $restos) {
            if (-not ($rel -match '^\.git\\')) { $fuera++ }
            return
        }
        $limpio = Join-Path $prist $rel
        if ((Test-Path $limpio) -and (Huella $limpio) -eq (Huella $_.FullName)) {
            $iguales++
            return
        }
        $dst = Join-Path $sobre $rel
        if ((Test-Path $dst) -and -not $Forzar) { return }
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $dst) | Out-Null
        Copy-Item -LiteralPath $_.FullName -Destination $dst -Force
        Write-Host "[doom]   sobre\$rel"
        $n++
    }
    Write-Host "[doom] sobre: $n fichero(s) del port; $iguales iguales al commit fijado; $fuera que no se mudan"

    $suCola = Join-Path $externo 'doom-port'
    if (Test-Path $suCola) {
        $m = 0; $fuera = 0
        Get-ChildItem -LiteralPath $suCola -Recurse -File -Force | ForEach-Object {
            $rel = $_.FullName.Substring($suCola.Length).TrimStart('\')
            if (('\' + $rel) -match $restos) { $fuera++; return }
            $dst = Join-Path $cola $rel
            if ((Test-Path $dst) -and -not $Forzar) { return }
            New-Item -ItemType Directory -Force -Path (Split-Path -Parent $dst) | Out-Null
            Copy-Item -LiteralPath $_.FullName -Destination $dst -Force
            $m++
        }
        Write-Host "[doom] cola: $m fichero(s) de doom-port; $fuera que no se mudan"
    } else {
        Write-Host "[doom] [!] no esta ${suCola}: la cola no se muda" -ForegroundColor Yellow
    }
    Write-Host '[doom] Revisa `git status` y haz el commit: desde hoy el port vive aqui.'
}

# -- 3. FREEDOOM (BSD), a wad\ -----------------------------------------------
#
# Primero el zip que ya este en BMO-externo (alli se bajo el 25-09, con su
# CHECKSUM al lado); si no esta, se baja. Si hay CHECKSUM se compara el
# SHA-256 y, si no casa, se para: un WAD corrupto da un DOOM que se cierra
# sin que nadie sepa por que.
if ($Freedoom) {
    $zipUrl = Linea 'freedoom'
    $zipNom = Split-Path -Leaf $zipUrl
    New-Item -ItemType Directory -Force -Path $wad | Out-Null
    $local = Join-Path $externo $zipNom
    $borrarZip = $false
    if (Test-Path $local) {
        $zip = $local
        Write-Host "[doom] Freedoom: el zip de BMO-externo ($zipNom)"
    } else {
        $zip = Join-Path $wad $zipNom
        Write-Host "[doom] Freedoom: $zipUrl"
        Invoke-WebRequest -Uri $zipUrl -OutFile $zip
        $borrarZip = $true
    }
    $suma = Join-Path $externo ($zipNom -replace '\.zip$', '-CHECKSUM')
    if (Test-Path $suma) {
        $h = (Get-FileHash -Algorithm SHA256 $zip).Hash.ToLower()
        $dice = (Get-Content $suma -Raw).ToLower()
        if ($dice -match $h) {
            Write-Host "[doom]   SHA-256 casa con $(Split-Path -Leaf $suma)" -ForegroundColor DarkGray
        } elseif ($dice -match '[0-9a-f]{64}') {
            throw "el SHA-256 de $zipNom ($h) no esta en $(Split-Path -Leaf $suma)"
        } else {
            Write-Host "[doom]   [!] $(Split-Path -Leaf $suma) no trae un SHA-256: no se comprueba" -ForegroundColor Yellow
        }
    }
    $tmp = Join-Path $wad 'tmp'
    Expand-Archive -LiteralPath $zip -DestinationPath $tmp -Force
    foreach ($w in 'freedoom1.wad', 'freedoom2.wad') {
        $f = Get-ChildItem -LiteralPath $tmp -Recurse -Filter $w | Select-Object -First 1
        if ($f) {
            Move-Item -LiteralPath $f.FullName -Destination (Join-Path $wad $w) -Force
            Write-Host "[doom]   wad\$w ($($f.Length) B)"
        }
    }
    Remove-Item -Recurse -Force $tmp
    if ($borrarZip) { Remove-Item -Force $zip }
}
