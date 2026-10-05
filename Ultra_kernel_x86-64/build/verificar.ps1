# verificar.ps1 -- LA VENTANA VERIFICAR (2026-10-03): todo lo que comprueba,
# sin compilar nada para la maquina.
#
#   el contrato y PROTON-X     `comprobar.ps1` (lo de `bmo.ps1`)
#   el banco del anfitrion     `comprobar.ps1` (`cargo test --workspace`)
#   los guardianes de Python   `guardianes.ps1` (lo de `build.ps1`)
#   las tablas del contrato    `contrato.ps1`
#
# ** Por que existe: el propietario pidio *"multiples shells que trabajen en
# paralelo, cada uno de acuerdo como cumplen: uno para compilar, otro para
# verificar y el principal para ir al FAT32"*. Antes `bmo.ps1` lo hacia todo en
# fila: el banco (~90 s), los veintidos guardianes, y SOLO DESPUES empezaba a
# compilar. Ahora esta ventana comprueba mientras la ventana COMPILAR
# (`build.ps1 -SinGuardianes`) construye, y la principal (`bmo.ps1 -Paralelo`)
# espera a las dos y lleva el resultado al disco SOLO si las dos dicen bien.
#
# [!] Lo que comprueba es EXACTAMENTE lo de siempre, del mismo texto: aqui no
# hay una lista propia de guardianes, que se quedaria atras el primer dia.
#
# Se corre sola tambien: `.\Ultra_kernel_x86-64\build\verificar.ps1` comprueba
# todo y no compila ni toca un disco.

param(
    # Sin el banco del anfitrion (lo de `bmo.ps1 -Rapido`).
    [switch]$Rapido,
    # Donde dejar el sello de VALKYRIE para la ventana principal (vacio: no).
    [string]$Sello = ''
)

$root = Split-Path -Parent $PSScriptRoot
$raiz = Split-Path -Parent $root

function Titulo($m) { Write-Host "`n== $m" -ForegroundColor Cyan }
function Bien($m)   { Write-Host "   OK  $m" -ForegroundColor DarkGray }
function Muere($m)  { Write-Host "   [X] $m" -ForegroundColor Red; $script:salida = 1; exit 1 }
# Lo que dejo un `exit` de un fichero cargado con punto (ver `Fail` en comun.ps1).
$script:salida = $null

Write-Host 'BMO-X -- VERIFICAR (a la vez que la ventana COMPILAR)' -ForegroundColor White

# [!] La RUTA del sello se guarda ANTES: PowerShell no distingue mayusculas en
# las variables, y `$Sello` (el parametro, la ruta) y `$sello` (el texto que
# deja `comprobar.ps1`) son LA MISMA. Sin esto, la linea de abajo borraba la
# ruta, `comprobar.ps1` le metia "BMO-X Engine [V-ABI ...]" y el `Set-Content`
# del final buscaba una carpeta con ese nombre (05-10, en `bmo.ps1 -Paralelo`).
$rutaSello = $Sello

# Lo de `bmo.ps1`: el contrato, PROTON-X y el banco. Deja `$sello`.
$sello = ''
$selloD = ''
. (Join-Path $PSScriptRoot 'comprobar.ps1')
if ($null -ne $script:salida) { exit $script:salida }

# Lo de `build.ps1`: sus guardianes y el contrato de las tablas.
. (Join-Path $PSScriptRoot 'comun.ps1')
. (Join-Path $PSScriptRoot 'guardianes.ps1')
if ($null -ne $script:salida) { exit $script:salida }
. (Join-Path $PSScriptRoot 'contrato.ps1')
if ($null -ne $script:salida) { exit $script:salida }

if ($rutaSello) {
    Set-Content -LiteralPath $rutaSello -Value @("$sello", "$selloD") -Encoding Ascii
}
Tiempos
Write-Host '  === TODO VERIFICADO ===' -ForegroundColor Green
exit 0
