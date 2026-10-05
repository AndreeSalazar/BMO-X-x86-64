# imagenes.ps1 -- las imagenes que el build FABRICA (2026-10-05): `Pon-U16`,
# `Pon-U32`, `Nuevo-Bmp`, `Nuevo-Qoi`, `Nuevo-Fondo` y `Nuevo-Bico` (los
# iconos de 16x16 que van dentro de cada `.bex`).
#
# ** Por que es un fichero: `ejemplos.ps1` paso de las 1.000 lineas de codigo
# (L6a) y estas seis son lo unico suyo que no depende de nada de lo de
# alrededor. Movidas tal cual, en el mismo orden; solo definen funciones.
#
# [!] Se carga con punto desde `ejemplos.ps1` (las usan el y `doom.ps1`) y
# usa su `Fail`.


function Pon-U16($lista, $v) { $lista.AddRange([BitConverter]::GetBytes([uint16]$v)) }
function Pon-U32($lista, $v) { $lista.AddRange([BitConverter]::GetBytes([uint32]$v)) }

# Un BMP de 16x16, 24 bits, de ABAJO ARRIBA y con relleno a 4: la forma mas
# comun, y la que obliga al conversor a dar la vuelta a las filas.
function Nuevo-Bmp {
    $w = 16; $h = 16
    $fila = [int][math]::Floor((24 * $w + 31) / 32) * 4
    $datos = $fila * $h
    $b = New-Object System.Collections.Generic.List[byte]
    $b.Add(66); $b.Add(77)
    Pon-U32 $b (54 + $datos); Pon-U32 $b 0; Pon-U32 $b 54
    Pon-U32 $b 40; Pon-U32 $b $w; Pon-U32 $b $h; Pon-U16 $b 1; Pon-U16 $b 24
    Pon-U32 $b 0; Pon-U32 $b $datos; Pon-U32 $b 2835; Pon-U32 $b 2835; Pon-U32 $b 0; Pon-U32 $b 0
    for ($y = $h - 1; $y -ge 0; $y--) {
        for ($x = 0; $x -lt $w; $x++) { $b.Add([byte](16 * $x)); $b.Add([byte](16 * $y)); $b.Add(200) }
        for ($p = 3 * $w; $p -lt $fila; $p++) { $b.Add(0) }
    }
    return $b.ToArray()
}

# Un QOI de 16x16: doce filas de color (RGB) y cuatro TRANSPARENTES con RUN,
# para que el conversor pase por las dos formas y por el alfa a cero.
function Nuevo-Qoi {
    $w = 16; $h = 16
    $b = New-Object System.Collections.Generic.List[byte]
    $b.AddRange([byte[]](113, 111, 105, 102))
    foreach ($v in @($w, $h)) { $x = [BitConverter]::GetBytes([uint32]$v); [array]::Reverse($x); $b.AddRange($x) }
    $b.Add(4); $b.Add(0)
    for ($y = 0; $y -lt 12; $y++) {
        for ($x = 0; $x -lt $w; $x++) { $b.Add(254); $b.Add([byte](16 * $x)); $b.Add([byte](16 * $y)); $b.Add(128) }
    }
    $b.AddRange([byte[]](255, 0, 0, 0, 0))
    $b.Add([byte](192 + 61)); $b.Add(192)
    $b.AddRange([byte[]](0, 0, 0, 0, 0, 0, 0, 1))
    return $b.ToArray()
}

# El FONDO del escritorio (2026-09-13): un atardecer de 480x270 en QOI --cielo,
# luna, estrellas, dos cordilleras y su reflejo en un lago-- que el DIRECTOR
# escala a la pantalla (`scene/fondo.rs`). Se GENERA por lo mismo que las fotos:
# un binario en el repo no se lee en un diff. Solo usa RGB y RUN.
# ** Desde el 2026-09-22 es el RESPALDO: el fondo es la ciudad del gato, y esto
# sale solo si falta python o Pillow para convertirla (ver mas abajo).
function Nuevo-Fondo {
    $w = 480; $h = 270; $lago = 212
    $ms = New-Object System.IO.MemoryStream
    $ms.Write([byte[]](113, 111, 105, 102), 0, 4)
    foreach ($v in @($w, $h)) { $x = [BitConverter]::GetBytes([uint32]$v); [array]::Reverse($x); $ms.Write($x, 0, 4) }
    $ms.WriteByte(3); $ms.WriteByte(0)
    # El cielo por fila: noche arriba, violeta a media altura, rosa en el horizonte.
    $cielo = New-Object 'int[]' $h
    for ($y = 0; $y -lt $h; $y++) {
        $t = [Math]::Min($y, $lago)
        if ($t -lt 120) { $k = $t / 120.0; $r = 14 + 52 * $k; $g = 16 + 22 * $k; $b = 38 + 58 * $k }
        else { $k = ($t - 120) / [double]($lago - 120); $r = 66 + 150 * $k; $g = 38 + 66 * $k; $b = 96 + 20 * $k }
        $cielo[$y] = ([int]$r -shl 16) -bor ([int]$g -shl 8) -bor [int]$b
    }
    $lejos = New-Object 'int[]' $w; $cerca = New-Object 'int[]' $w
    for ($x = 0; $x -lt $w; $x++) {
        $lejos[$x] = [int](150 + 20 * [Math]::Sin($x / 37.0) + 9 * [Math]::Sin($x / 11.0 + 1))
        $cerca[$x] = [int](184 + 16 * [Math]::Sin($x / 53.0 + 2) + 7 * [Math]::Sin($x / 17.0))
    }
    $prev = -1; $run = 0
    for ($y = 0; $y -lt $h; $y++) {
        # Debajo del lago se mira la fila ESPEJO, y sale a media luz.
        $reflejo = $y -ge $lago
        $yy = if ($reflejo) { 2 * $lago - $y - 1 } else { $y }
        for ($x = 0; $x -lt $w; $x++) {
            if ($yy -lt $lejos[$x]) {
                $c = $cielo[$yy]
                $dx = $x - 360; $dy = $yy - 62
                if ($dx * $dx + $dy * $dy -lt 196) { $c = 0xF0DCC8 }
                elseif ($yy -lt 130 -and (($x * 7919 + $yy * 104729) % 1013) -eq 0) { $c = 0xE6E6FF }
            }
            elseif ($yy -lt $cerca[$x]) { $c = 0x3A2E5A }
            else { $c = 0x1C1630 }
            if ($reflejo) { $c = ($c -shr 1) -band 0x7F7F7F }
            if ($c -eq $prev) {
                $run++
                if ($run -eq 62) { $ms.WriteByte(253); $run = 0 }
            } else {
                if ($run -gt 0) { $ms.WriteByte([byte](191 + $run)); $run = 0 }
                $ms.WriteByte(254)
                $ms.WriteByte([byte](($c -shr 16) -band 255)); $ms.WriteByte([byte](($c -shr 8) -band 255)); $ms.WriteByte([byte]($c -band 255))
                $prev = $c
            }
        }
    }
    if ($run -gt 0) { $ms.WriteByte([byte](191 + $run)) }
    $ms.Write([byte[]](0, 0, 0, 0, 0, 0, 0, 1), 0, 8)
    return ,$ms.ToArray()
}

function Nuevo-Bico {
    param([string[]]$filas)
    $lado = 16
    if ($filas.Count -ne $lado) { Fail ('un icono son ' + $lado + ' filas, no ' + $filas.Count) }
    $bytes = New-Object System.Collections.Generic.List[byte]
    $bytes.AddRange([byte[]][System.Text.Encoding]::ASCII.GetBytes('BICO'))
    $bytes.AddRange([byte[]][System.BitConverter]::GetBytes([uint16]$lado))
    $bytes.AddRange([byte[]][System.BitConverter]::GetBytes([uint16]$lado))
    foreach ($f in $filas) {
        if ($f.Length -ne $lado) { Fail ('una fila del icono mide ' + $f.Length + ' y no ' + $lado) }
        foreach ($ch in $f.ToCharArray()) {
            $c = [string]$ch
            if (-not $BICO_PALETA.ContainsKey($c)) { Fail ("el icono usa '" + $c + "', que no esta en la paleta") }
            $bytes.AddRange([byte[]]$BICO_PALETA[$c])
        }
    }
    $esperado = 8 + $lado * $lado * 4
    if ($bytes.Count -ne $esperado) { Fail ('el icono salio de ' + $bytes.Count + ' B y son ' + $esperado) }
    return $bytes.ToArray()
}
