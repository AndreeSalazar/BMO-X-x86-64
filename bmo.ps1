# BMO -- una sola orden.
#
# `.\bmo.ps1`              comprueba y construye TODO. No toca ningun disco.
# `.\bmo.ps1 -Desplegar`   ademas lo lleva al Kingston.
# `.\desplegar.ps1`        lo mismo, en una palabra. Ver su cabecera: la
#                          seguridad se queda y la molestia se va.
# `.\bmo.ps1 -Rapido`      se salta el banco de pruebas (para iterar).
#
# == Por que existe, siendo que `build.ps1` ya construye ==
#
# `build.ps1` construye. Lo que NO hace es **comprobar antes**: el banco de
# pruebas del compilador y los crates del anfitrion se corren a mano, y lo que
# se corre a mano se deja de correr. Esta orden los pone delante del build, que
# es el unico sitio donde no se olvidan.
#
# Y el orden importa: primero lo que se prueba en el anfitrion --rapido y
# barato-- y solo si pasa se construye lo que va a la maquina. Al reves se
# tarda tres minutos en descubrir algo que un test decia en tres segundos.

param(
    [switch]$Desplegar,
    # **No preguntes, escribe.** Se lo pidio el propietario --*"da flojera"*-- y se
    # puede dar porque la pregunta NO es lo que protege: lo que protege son las
    # tres comprobaciones de `build/discos.ps1`, que siguen intactas. Y el `-Si`
    # tiene techo: por encima de 64 GiB se pregunta igual, porque ahi si hay
    # algo que perder. Ver la cabecera de `TECHO_SIN_PREGUNTA_GIB`.
    [switch]$Si,
    [switch]$Rapido,
    # A que unidades va, si se despliega.
    #
    # [!] AQUI DECIA "se piden EXPLICITAS y sin valor por defecto util", y es
    # FALSO desde que estan escritas dos lineas mas abajo: `D` y `A` son
    # valores por defecto perfectamente utiles. El comentario describia una
    # politica que el codigo de su propia linea no cumple.
    #
    # ** Lo que de verdad protege el NVMe no son estas letras: es que
    # `-Desplegar` sea OPCIONAL. Un `bmo.ps1` suelto --tecleado por quien sea,
    # o por un script de comprobacion-- no toca ningun disco, y eso vale mas
    # que obligar a escribir dos letras que siempre son las mismas.
    #
    # Corregido el 2026-09-04, el mismo dia en que un comentario sobre una
    # proteccion ya retirada --en `usb/rescate.rs`-- costo tres dias.
    #
    # ** Y el 2026-09-16 la letra por defecto del ARRANQUE se QUITA: decia `D`
    # desde que D: era el Ventoy, y ese dia D: era ya un disco NTFS del propietario
    # llamado "Personal" (desde el 27-08). Un `desplegar.ps1 -Si` a secas
    # habria escrito EFI\BOOT en un disco que no es de BMO. Las letras que
    # siempre son las mismas se tecleaban igual (`-Arranque A -Datos A`); lo
    # que cambia es que ahora, sin letra, se NIEGA con el motivo en vez de
    # adivinar. Un valor por defecto es una decision que nadie tomo hoy.
    [string]$Arranque = '',
    [string]$Datos = '',
    # ** EL KERNEL DE MEDIDA. Devuelve los dos `rdtsc` a `dispatch`, o sea el
    # REPARTO de una puerta entre su mitad Rust y el resto -- lo unico que puede
    # decir donde se van los ~945 ciclos.
    #
    # [!] NO SE DEJA PUESTO: cuesta ~112 ciclos en CADA puerta de CADA programa,
    # un 11%. Se despliega, se corre `sys/precio.bex`, se apunta el reparto y se
    # vuelve a desplegar sin esta bandera. Un instrumento que se queda puesto
    # deja de ser una medida y pasa a ser un peaje.
    [switch]$Metro,
    # ** LAS TRES VENTANAS (2026-10-03). Abre una ventana que COMPILA y otra
    # que VERIFICA (el contrato, el banco y los guardianes), a la vez; esta, la
    # PRINCIPAL, espera a las dos y solo si las dos dicen bien va al disco.
    # Ver `Ultra_kernel_x86-64\build\paralelo.ps1`. Sin ella, todo en fila
    # como siempre.
    [switch]$Paralelo
)

# [!] NADA DE `$ErrorActionPreference = 'Stop'` AQUI.
#
# Estuvo puesto y rompio DOS cosas, la segunda peor que la primera:
#
#   1. El bucle de `cargo test`, que moria en el primer warning -- cada linea
#      de stderr de un nativo es un ErrorRecord, y con `Stop` es terminante.
#   2. **`build.ps1` entero**, que se ejecuta en esta misma sesion y por tanto
#      HEREDA la preferencia. Un guion que llevaba meses funcionando empezo a
#      morirse en `warning: unused variable`, y no por nada suyo: por una linea
#      escrita en el guion que lo llama.
#
# Aqui los fallos se detectan mirando lo que las herramientas DICEN --las filas
# que pasan, el codigo de salida del build-- y no dejando que el shell decida
# que un aviso del compilador es motivo para abortar.
$raiz = $PSScriptRoot
$t0 = Get-Date

function Titulo($m) { Write-Host "`n== $m" -ForegroundColor Cyan }
function Bien($m)   { Write-Host "   OK  $m" -ForegroundColor DarkGray }
function Muere($m)  { Write-Host "   [X] $m" -ForegroundColor Red; $script:salida = 1; exit 1 }
# Lo que dejo un `exit` de un fichero cargado con punto (ver `Fail` en
# Ultra_kernel_x86-64\build\comun.ps1: alli esta el por que).
$script:salida = $null

Write-Host "BMO-X -- comprobar y construir" -ForegroundColor White

# -- LAS UNIDADES SE DICEN, NO SE ADIVINAN -- y se comprueba ANTES del banco,
# que tarda 90 s: negarse despues de comprobar todo es hacer esperar para nada.
if ($Desplegar -and (-not $Arranque -or -not $Datos)) {
    Write-Host '   sin -Arranque <letra> y -Datos <letra> no se despliega:' -ForegroundColor Red
    Write-Host '   en esta maquina el disco de BMO es A: (.\desplegar.ps1 -Si -Arranque A -Datos A)' -ForegroundColor Red
    Write-Host '   y ninguna otra letra es suya. Ver bmo-maquina-y-discos.' -ForegroundColor Red
    exit 1
}

# -- 0, 0b y 1: EL CONTRATO, PROTON-X Y EL BANCO -----------------------------
#
# En `Ultra_kernel_x86-64\build\comprobar.ps1` desde el 2026-10-03, el texto
# tal cual: con `-Paralelo` lo corre la ventana VERIFICAR (ver abajo) y aqui
# no se repite.
if (-not $Paralelo) {
    . (Join-Path $raiz 'Ultra_kernel_x86-64\build\comprobar.ps1')
    if ($null -ne $script:salida) { exit $script:salida }
}

# -- 2. CONSTRUIR ---------------------------------------------------------
#
# `build.ps1` hace el resto y lleva sus propios guardianes de contrato dentro:
# los dos syscalls contra `bmo-abi`, la tabla `OP_INFO` en sus tres sitios,
# `KIND_AUDIO` en kernel y ABI, ningun opcode repetido, y el portico de ASCII.
Titulo 'Construir (toolchain + kernel + Ring 3 + los .bex)'
$build = Join-Path $raiz 'Ultra_kernel_x86-64\build.ps1'
if (-not (Test-Path $build)) { Muere "no esta build.ps1" }

if ($Paralelo) {
    # ** LAS TRES VENTANAS: COMPILAR y VERIFICAR a la vez; esta espera.
    . (Join-Path $raiz 'Ultra_kernel_x86-64\build\paralelo.ps1')
    $verificar = Join-Path $raiz 'Ultra_kernel_x86-64\build\verificar.ps1'
    $selloFichero = Join-Path $script:carpetaVentanas 'VERIFICAR.sello'
    Remove-Item -LiteralPath $selloFichero -ErrorAction SilentlyContinue
    $ordenCompilar = "& '" + ($build -replace "'", "''") + "' -BuildOnly -SinGuardianes" + $(if ($Metro) { ' -Metro' } else { '' })
    $ordenVerificar = "& '" + ($verificar -replace "'", "''") + "' -Sello '" + ($selloFichero -replace "'", "''") + "'" + $(if ($Rapido) { ' -Rapido' } else { '' })
    $ventanas = @(
        (Abrir-Ventana 'COMPILAR' $ordenCompilar),
        (Abrir-Ventana 'VERIFICAR' $ordenVerificar)
    )
    Bien 'dos ventanas abiertas: COMPILAR y VERIFICAR (esta espera a las dos)'
    $mala = Esperar-Ventanas $ventanas
    if ($mala) {
        Muere ('la ventana ' + $mala.Nombre + ' no acabo bien -- lo dice ella; NO se toco ningun disco')
    }
    if (Test-Path -LiteralPath $selloFichero) {
        $lineas = @(Get-Content -LiteralPath $selloFichero)
        if ($lineas.Count -ge 1) { $sello = $lineas[0] }
        if ($lineas.Count -ge 2) { $selloD = $lineas[1] }
    }
    $py = (Get-Command python -ErrorAction SilentlyContinue)
    if ($Desplegar) {
        Titulo ('Al disco: ' + $Arranque + ' y ' + $Datos + ' (lo que COMPILAR dejo en staging)')
        if ($Metro) {
            Write-Host "   y va el KERNEL DE MEDIDA: acuerdate de volver sin -Metro" -ForegroundColor Yellow
        }
        & $build -SoloDiscos -Todo -Drive $Arranque -Data $Datos -Yes:$Si
    } else {
        $global:LASTEXITCODE = 0
    }
} elseif ($Desplegar) {
    # [!] LAS UNIDADES SE DICEN, NO SE ADIVINAN. Y desde el 16-09,
    # literalmente: sin letra el script se nego arriba, antes del banco.
    #
    # En esta maquina el NVMe es el Windows del propietario y BMO vive en un Kingston
    # SATA. Un build que eligiera unidad por su cuenta seria la unica orden de
    # este repositorio capaz de estropear algo que no es suyo. El gate de
    # identidad de ESTRATOS protege el volumen de datos, pero la letra la pone
    # una persona.
    Write-Host "   se va a ESCRIBIR en $Arranque y en $Datos" -ForegroundColor Yellow
    if ($Si) {
        Write-Host "   -Si: sin preguntar (las comprobaciones duras siguen)" -ForegroundColor DarkGray
    }
    if ($Metro) {
        Write-Host "   y va el KERNEL DE MEDIDA: acuerdate de volver sin -Metro" -ForegroundColor Yellow
    }
    & $build -Todo -Drive $Arranque -Data $Datos -Metro:$Metro -Yes:$Si
} else {
    & $build -BuildOnly -Metro:$Metro
}
if ($LASTEXITCODE -ne 0) { Muere 'fallo el build' }

$seg = [int]((Get-Date) - $t0).TotalSeconds
Write-Host "`nlisto en $seg s" -ForegroundColor Green
if (-not $Desplegar) {
    Write-Host "   (no se toco ningun disco -- para desplegar: .\desplegar.ps1)" -ForegroundColor DarkGray
}

# -- EL SELLO DE VALKYRIE, LO ULTIMO QUE SE VE -------------------------------
#
# ** Y las tres ramas son el punto entero. Un `Compliant` que sale siempre no
# informa de nada: informa de que la linea existe. Aqui la ausencia del sello
# es RUIDOSA, porque el dia que alguien rompa R13-R16 el build muere antes --
# pero el dia que falte `python`, el build PASA y no ha comprobado nada, y esa
# es justo la tarde en la que un sello mentiroso se cuela en una captura.
if ($sello) {
    Write-Host "`n   $sello" -ForegroundColor Magenta
    if ($selloD) { Write-Host "   $selloD" -ForegroundColor DarkGray }
} elseif ($py) {
    Write-Host "`n   [!] V-ABI SIN SELLAR -- el contrato paso y no emitio sello" -ForegroundColor Yellow
} else {
    Write-Host "`n   [!] V-ABI SIN SELLAR -- sin python no hay nada que sellar" -ForegroundColor Yellow
}
