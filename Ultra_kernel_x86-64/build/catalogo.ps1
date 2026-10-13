# BMO -- EL CATALOGO DE LOS EJEMPLOS: que se compila y adonde va.
#
# ** Por que es un fichero (13-10): `ejemplos.ps1` volvio a pasar de las
# 1.000 lineas de codigo (L6a), como el 26-09 (`doom.ps1`) y el 05-10
# (`imagenes.ps1`). Esto es la LISTA -- los programas de COBOL, Ada, C++ y C,
# sus recursos y la paleta de BICO -- y `ejemplos.ps1` es lo que HACE con
# ella. Solo define variables: no compila ni copia nada.
#
# [!] Se carga con punto desde `ejemplos.ps1`, en SU ambito.

# -- Programas COBOL de ejemplo -----------------------------------
#
# Se compilan AQUI y salen al mismo staging que el compositor. Antes se
# generaban a mano dentro de toolchain/lang/cobol/examples/ y nunca llegaban al
# disco: `run apps/extracto.bex` contestaba "no esta: revisa la ruta" y parecia
# un fallo del cargador cuando el archivo sencillamente no se habia copiado.
#
# El nombre destino se recorta a 8.3 a proposito y de forma explicita: el
# driver FAT32 del kernel se NIEGA a recortar (un nombre recortado abre otro
# archivo), asi que el que no quepa se dice aqui y no en el arranque.
# Las rutas llevan el NIVEL delante: los ejemplos estan en escalera (1-basico,
# 2-decimal, 3-presentacion, 4-ficheros, 5-tablas), ordenados por cuanto COBOL
# hace falta que el compilador sepa. Ver examples\README.md.
# * POR NIVELES, y no en un monton. Los ejemplos estan en ESCALERA -cada uno
# pide una cosa mas que el anterior- y esa escalera se pierde si en el disco
# caen todos revueltos. Con niveles se puede VERIFICAR de uno en uno:
#
#     run cobol/1/hola.bex     y si eso va, subir
#     run cobol/2/banco.bex    y si eso va, subir
#     ...
#
# Y cuando algo se rompa, el orden dice por donde empezar a mirar: si falla el
# 10, comprobar primero que el 1 sigue vivo.
#
# [!] La carpeta es el NUMERO a secas y no el nombre largo. No es pereza: el
# driver FAT32 del kernel se NIEGA a recortar, y `3-presentacion` son trece
# letras. Un `n3presen` seria feo y no diria mas que un `3`; el nombre del nivel
# vive en examples\README.md, que es donde se lee.
$cobolEjemplos = @(
    @{ src = 'toolchain\lang\cobol\examples\1-basico\hola.cob';           out = 'hola.bex'     ; dir = 'cobol\1' },
    @{ src = 'toolchain\lang\cobol\examples\2-decimal\banco.cob';         out = 'banco.bex'    ; dir = 'cobol\2' },
    @{ src = 'toolchain\lang\cobol\examples\2-decimal\calc.cob';          out = 'calc.bex'     ; dir = 'cobol\2' },
    @{ src = 'toolchain\lang\cobol\examples\2-decimal\calcgui.cob';       out = 'calcgui.bex'  ; dir = 'cobol\2' },
    @{ src = 'toolchain\lang\cobol\examples\3-presentacion\extracto.cob'; out = 'extracto.bex' ; dir = 'cobol\3' },
    @{ src = 'toolchain\lang\cobol\examples\4-ficheros\batch.cob';        out = 'batch.bex'    ; dir = 'cobol\4' },
    # `conceptos` son nueve letras y el driver FAT32 se NIEGA a recortar, asi
    # que el destino es `concep`. La comprobacion de 8.3 de abajo lo cazaria
    # igual, pero mejor no llegar a que la cace.
    @{ src = 'toolchain\lang\cobol\examples\5-tablas\conceptos.cob';      out = 'concep.bex'   ; dir = 'cobol\5' },
    @{ src = 'toolchain\lang\cobol\examples\6-condiciones\cartera.cob';   out = 'carter.bex'   ; dir = 'cobol\6' },
    @{ src = 'toolchain\lang\cobol\examples\7-empaquetado\cuentas.cob';   out = 'cuentas.bex'  ; dir = 'cobol\7' },
    @{ src = 'toolchain\lang\cobol\examples\8-parrafos\cierre.cob';       out = 'cierre.bex'   ; dir = 'cobol\8' },
    @{ src = 'toolchain\lang\cobol\examples\9-decision\comision.cob';     out = 'comisio.bex'  ; dir = 'cobol\9' },
    @{ src = 'toolchain\lang\cobol\examples\10-binario\maestro.cob';      out = 'maestro.bex'  ; dir = 'cobol\10' },
    # BANK CAT (03-10): el motor del libro, con la LIBRERIA de copybooks (`COPY`).
    @{ src = 'toolchain\lang\cobol\examples\11-bankcat\libro.cob';      out = 'libro.bex'    ; dir = 'cobol\11' }
)
# -- Programas ADA de ejemplo -------------------------------------
#
# Crates PROPIOS, sin dependencia de los otros frontends: `bmo-ada-front` analiza
# y `bmo-ada-x86-64` (su `emisor-x86_64/`, partido el 2026-09-18) emite. El
# decimal exacto es el mismo que el de COBOL y no por copia: el Annex F de Ada
# copio las reglas de COBOL, asi que dos lenguajes que dicen lo mismo acaban en
# la misma aritmetica de enteros escalados.
$adaEjemplos = @(
    @{ src = 'toolchain\lang\ada\examples\1-basico\cierre.adb'; out = 'cierre.bex' ; dir = 'ada' }
)
# -- Programas C++ de ejemplo -------------------------------------
#
# ** 2026-09-17: el primer .bex de C++ que llega al disco. C++ estuvo APARCADO
# desde el 12-08 con dos filas rojas que resultaron ser del ARNES de pruebas y
# no del compilador, y su linea de ordenes tomaba `-o` como nombre de fichero.
# Este ejemplo encontro ademas que un derivado no llamaba al destructor de su
# base. Ver toolchain/lang/cpp/APARCADO.md, seccion 7.
$cppEjemplos = @(
    @{ src = 'toolchain\lang\cpp\examples\1-clases\cuentas.cpp'; out = 'cuentas.bex' ; dir = 'cpp' }
)
# -- Programas C de ejemplo ---------------------------------------
#
# * Este paso NO EXISTIA. COBOL y Ada llegaban al disco y C no, asi que los
# ejemplos de C solo se podian ejecutar si alguien los embebia a mano en el
# kernel -- y `scroll_C.bex` no llegaba al Kingston por eso, no por un fallo del
# compilador. Un lenguaje que compila y cuyo binario no se despliega esta a
# medias.
#
# `hola_C.c` prueba lo basico (bucles, %d, resta con signo, switch, %s).
# `scroll_C.c` usa las cabeceras `<bmo/...>`: la puerta de syscalls desde C, la
# capability de entrada y el modelo de scroll.
# `memoria_C.c` ESTRENA `KIND_MEMORIA`: pide, escribe, relee y agota el tope de
# cuatro peticiones. Es el unico programa que ejerce la capability de memoria.
$cEjemplos = @(
    @{ src = 'toolchain\lang\c\examples\hola_C.c';   out = 'holac.bex'  ; dir = 'c' },
    @{ src = 'toolchain\lang\c\examples\scroll_C.c'; out = 'scrollc.bex' ; dir = 'c' },
    @{ src = 'toolchain\lang\c\examples\pregunta_C.c'; out = 'pregc.bex'  ; dir = 'c' },
    @{ src = 'toolchain\lang\c\examples\memoria_C.c'; out = 'memc.bex'   ; dir = 'c' },
    # ** EL SELLO (W^X, 2026-09-23): escribe `mov eax,42; ret`, lo sella, lo
    # llama (42) y al final escribe en el codigo sellado A PROPOSITO: lo
    # correcto es que muera ahi con un fallo de pagina de Ring 3.
    @{ src = 'toolchain\lang\c\examples\sello_C.c';  out = 'sello.bex'  ; dir = 'c' },
    # ** LA SONDA: el unico programa que usa la superficie MAL a proposito.
    # Handles inventados, operaciones que no existen, el renglon de ruta
    # inundado, el tope de memoria forzado, medidas imposibles. Cada empujon
    # tiene UNA respuesta correcta --el kernel dice que no y sigue vivo-- y que
    # el programa llegue a imprimir su recuento ya es media prueba.
    @{ src = 'toolchain\lang\c\examples\sonda_C.c';  out = 'sonda.bex'  ; dir = 'c' },
    # El ensayo general de DOOM: 2.5D en punto fijo sobre la pantalla real.
    @{ src = 'toolchain\lang\c\examples\raycaster_C.c'; out = 'ray.bex'    ; dir = 'c' },
    # ** EL BLOC DE NOTAS, y lo que estrena: ESCRIBIR dentro de una ventana.
    # Hasta el 11-09 una app recibia SCANCODES y nunca la LETRA que producian,
    # asi que no se podia teclear en una superficie sin copiarle la
    # distribucion castellana entera. Ahora el DIRECTOR reenvia el caracter ya
    # cocido --bit 62 del evento-- y esto solo lo lee. Abre `datos\notas.txt`,
    # se escribe encima y se guarda con el boton de su barra.
    # ** Y SE QUEDA EN `c\`, AUNQUE NO SEA UN EJEMPLO QUE SE CIERRA.
    #
    # Primero fue a `apps\`, porque el lanzador del escritorio lista `apps\` y
    # nada mas: ahi tendria icono, y un bloc de notas al que hay que llamar
    # escribiendo su ruta es un bloc de notas que no se usa. Y se deshizo
    # **porque `apps\` ya significa algo** y lo dice esta misma cabecera: lo
    # que alguien trae de FUERA del repo. El criterio es la procedencia, no si
    # el programa es util, y meter aqui una excepcion a la primera incomodidad
    # es como se deshacen las carpetas que se ordenaron una vez.
    #
    # Asi que hoy se lanza escribiendo `run c/texto.bex` en la caja del
    # escritorio, y lo que falta --que el lanzador mire algo mas que `apps\`--
    # esta escrito como casilla en `docs/plan/PLAN_DIRECTOR.md`. El icono va
    # dentro del `.bex` igual: es su cara, la lleve quien la lea o no.
    @{ src = 'toolchain\lang\c\examples\texto_C.c';     out = 'texto.bex'  ; dir = 'c' },
    # ** UNA APP SACA SU PROPIA IMAGEN DE DENTRO DE SI MISMA. Junta tres
    # piezas que existian por separado y nunca se habian usado juntas:
    # `paquete.h` (mi imagen, sin escribir ninguna ruta), `imagen.h` (el BICO,
    # a escala entera) y `superficie.h`. El icono que lee es EL MISMO recurso
    # que el escritorio pinta en la rejilla: el dato es uno, no hay copia.
    # Y se pinta sobre un TABLERO de cuadros a proposito -- el alfa es un bit,
    # y un fondo liso esconde que no se respete.
    @{ src = 'toolchain\lang\c\examples\imagen_C.c';    out = 'imagen.bex' ; dir = 'c' },
    # ** LA GUIA, y el texto NO esta en el programa: viaja como recurso.
    # Es la idea del `.datex` del propietario hecha con lo que el sistema ya
    # soporta -- para cambiar la guia se edita `guia.txt` y se reempaca, y el
    # programa no se toca ni se recompila.
    @{ src = 'toolchain\lang\c\examples\guia_C.c';      out = 'guia.bex'   ; dir = 'c' },
    # ** EL CUBO, Y SIN UN SOLO TRIANGULO. Lo pidio el propietario: *\"el triangulo
    # esta muy quemado\"*. Aqui el elemento que se dibuja es LA CAJA entera,
    # trazada por el metodo de las laminas: exacta a cualquier zoom, sin
    # vertices, sin recorte y con la normal saliendo de la interseccion.
    # Trae su propio metro (`[cubo] ... us`) porque cuesta por PIXEL.
    @{ src = 'toolchain\lang\c\examples\cubo_C.c';      out = 'cubo.bex'   ; dir = 'c' },
    # La prueba de fopen/fread/fseek. Lee `datos\salida.txt` DOS veces y
    # compara: si las dos lecturas coinciden, la cadena de ficheros funciona.
    @{ src = 'toolchain\lang\c\examples\leer_C.c';      out = 'leer.bex'   ; dir = 'c' },
    # ESTRENA `KIND_AUDIO`. Comprueba el CONTRATO y no el oido: que hay handle,
    # que el tope de duracion se cumple, que es exclusivo y --la que importa--
    # que el handle soltado ya NO pita. Puede que no se oiga nada y este todo
    # bien: el puerto del altavoz existe en todo x86, el zumbador no.
    @{ src = 'toolchain\lang\c\examples\sonido_C.c';    out = 'sonido.bex' ; dir = 'c' },
    # `<bmo/musica.h>`: notas por nombre, figuras y tempo. Se DIBUJA mientras
    # suena, porque puede que no suene -- si la placa no trae zumbador, la
    # pantalla es la unica prueba de que la cadena entera funciono.
    @{ src = 'toolchain\lang\c\examples\musica_C.c';    out = 'musica.bex' ; dir = 'c' },
    # ** EL PAQUETE: este `.bex` viaja con datos DENTRO y los lee sin escribir
    # ninguna ruta -- le pide al kernel su propia imagen. Se empaqueta justo
    # despues de compilarlo, ver `$cRecursos`.
    @{ src = 'toolchain\lang\c\examples\caja_C.c';      out = 'caja.bex'   ; dir = 'c' },
    # ** UNA PIEZA DE VERDAD sobre `KIND_AUDIO`: el ritornello de "La primavera"
    # de Vivaldi (1725, dominio publico). `musica.bex` prueba la libreria nota a
    # nota; esta prueba la PIEZA -- que ocho compases seguidos no deriven, y que
    # el eco forte/piano que Vivaldi escribio salga por `BMO_SONIDO_VOLUMEN`.
    @{ src = 'toolchain\lang\c\examples\vivaldi_C.c';   out = 'vivaldi.bex'; dir = 'c' },
    # ** EL NUMERO QUE NO EXISTIA: cuantos ciclos vale una puerta.
    # Compara bucle vacio, llamada normal, `INVOKE` pelado sobre la tarea
    # actual, e `INVOKE` sobre un handle de verdad. Se queda con el MINIMO,
    # porque el temporizador expropia y una media se puede inflar. Decide si
    # algo puede pasar por la superficie o tiene que ser codigo enlazado --
    # empezando por el runtime de Python. Ver `docs/maestro/PYTHON_MAESTRO.md`.
    @{ src = 'toolchain\lang\c\examples\coste_C.c';     out = 'coste.bex'  ; dir = 'c' },
    # *** DE QUE ESTA HECHA UNA PUERTA: parte los ticks en FIJO --lo que
    # cuesta cruzar y volver-- y TRABAJO --lo que se pidio--, midiendo una
    # puerta que el kernel RECHAZA. Un rechazo recorre la maquina entera y no
    # hace nada, asi que ES el fijo, medido y no estimado.
    #
    # ** Y con eso proyecta la tabla del LOTE: si una puerta llevara N
    # operaciones, el fijo se divide entre N y el trabajo no. Es lo unico que
    # puede bajar la fila `puerta` de `presupuesto.rs` a su meta de 300 sin
    # quitarle nada al trabajo. Ver `docs/plan/PLAN_LA_PUERTA_SE_PARTE.md`.
    @{ src = 'toolchain\lang\c\examples\ciclos_C.c';    out = 'ciclos.bex' ; dir = 'c' },
    # LA MEDIDA DEL BLIT: memcpy a RAM contra memcpy al framebuffer (WC), y
    # un bucle de 8 bytes como tercera fila. Ver su cabecera.
    @{ src = 'toolchain\lang\c\examples\blit_C.c';      out = 'blit.bex'   ; dir = 'c' }
)

# * LOS RECURSOS QUE VAN DENTRO DE UN `.bex`.
#
# Se meten DESPUES de compilar, y por eso es un paso aparte y no una opcion del
# compilador: el codigo lo emite el frontend, los datos llegan de quien monta la
# app. Ver `toolchain	oolsmo-pack`.
#
# ** Los bytes van escritos AQUI y no en un fichero suelto a proposito: el
# programa comprueba su contenido (`1..8`), asi que si esta lista y el ejemplo
# se separan, la prueba lo dice en vez de pasar por casualidad.
$cRecursos = @(
    @{ bex = 'c\caja.bex'; recursos = @(
        @{ nombre = 'saludo.txt'; texto = 'hola desde dentro de la caja' },
        @{ nombre = 'cuenta.bin'; bytes  = @(1,2,3,4,5,6,7,8) }
    ) }
    # ** LA CARA DEL BLOC DE NOTAS. Por el mismo camino que la de DOOM --un
    # recurso del paquete-- asi que el escritorio no necesita ni un acceso
    # directo ni una cache de iconos: el `.bex` va con su cara dentro.
    #
    # Una hoja con su barra azul arriba, que es lo que el programa muestra.
    # ** EL ICONO DE `imagen.bex` TIENE AGUJEROS A PROPOSITO. Un rombo deja
    # las cuatro esquinas transparentes, y eso es lo que hace que la prueba
    # del alfa sea una prueba: sobre el tablero de cuadros, las esquinas
    # tienen que dejar ver los cuadros.
    # ** LA GUIA: su TEXTO y su CARA, los dos dentro del mismo fichero.
    # ** Y LA TEXTURA DEL CUBO ES SU PROPIO ICONO. El mismo recurso que el
    # escritorio pintaria en la rejilla se ve pegado en sus caras: el dato es
    # uno, y un cubo con un cubo dibujado encima es exactamente lo que es.
    @{ bex = 'c\cubo.bex'; recursos = @(
        @{ nombre = 'icono'; icono = @(
            '................',
            '....oooooooo....',
            '...oWWWWWWWWo...',
            '..oWWWWWWWWWWo..',
            '.oooooooooooooo.',
            '.obbbbbboggggggo',
            '.obbbbbboggggggo',
            '.obbbbbboggggggo',
            '.obbbbbboggggggo',
            '.obbbbbboggggggo',
            '.obbbbbboggggggo',
            '.obbbbbboggggggo',
            '.obbbbbboggggggo',
            '.oooooooooooooo.',
            '................',
            '................'
        ) }
    ) }
    @{ bex = 'c\guia.bex'; recursos = @(
        @{ nombre = 'guia.txt'; desde = 'toolchain\lang\c\examples\guia.txt' },
        @{ nombre = 'icono'; icono = @(
            '................',
            '..oooooooooooo..',
            '..oWWWWWWWWWWo..',
            '..oWggggggggWo..',
            '..oWWWWWWWWWWo..',
            '..oWggggggWWWo..',
            '..oWWWWWWWWWWo..',
            '..oWggggggggWo..',
            '..oWWWWWWWWWWo..',
            '..oWgggggWWWWo..',
            '..oWWWWWWWWWWo..',
            '..oWWWbbbbWWWo..',
            '..oWWWbWWbWWWo..',
            '..oWWWWWbbWWWo..',
            '..oWWWWWbWWWWo..',
            '..oooooooooooo..'
        ) }
    ) }
    @{ bex = 'c\imagen.bex'; recursos = @(
        @{ nombre = 'icono'; icono = @(
            '.......oo.......',
            '......obbo......',
            '.....obbbbo.....',
            '....obbbbbbo....',
            '...obbbWWbbbo...',
            '..obbbWWWWbbbo..',
            '.obbbWWWWWWbbbo.',
            'obbbWWWWWWWWbbbo',
            'obbbWWWWWWWWbbbo',
            '.obbbWWWWWWbbbo.',
            '..obbbWWWWbbbo..',
            '...obbbWWbbbo...',
            '....obbbbbbo....',
            '.....obbbbo.....',
            '......obbo......',
            '.......oo.......'
        ) }
    ) }
    @{ bex = 'c\texto.bex'; recursos = @(
        @{ nombre = 'icono'; icono = @(
            '................',
            '...oooooooooo...',
            '...obbbbbbbbo...',
            '...oWWWWWWWWo...',
            '...oWggggggWo...',
            '...oWWWWWWWWo...',
            '...oWgggggWWo...',
            '...oWWWWWWWWo...',
            '...oWgggggggo...',
            '...oWWWWWWWWo...',
            '...oWggggWWWo...',
            '...oWWWWWWWWo...',
            '...oWgggggWWo...',
            '...oWWWWWWWWo...',
            '...oooooooooo...',
            '................'
        ) }
    ) }
    # ** LA CARA DE NAVEGAR: una antena (el mastil y sus ondas) sobre la
    # pantalla que la muestra. Desde el 10-10 es el `.bex` de TITAN++ (corte
    # 4c): el mismo formato, el mismo cargador, la misma rejilla del
    # escritorio, y el juez de carga de acuerdo con el icono dentro.
    @{ bex = 'apps\navegar.bex'; recursos = @(
        @{ nombre = 'icono'; icono = @(
            '.......bb.......',
            '.....bb..bb.....',
            '....b..bb..b....',
            '....b.b..b.b....',
            '.......oo.......',
            '.......oo.......',
            '..oooooooooooo..',
            '..oWWWWWWWWWWo..',
            '..oWggggggggWo..',
            '..oWWWWWWWWWWo..',
            '..oWgggggWWWWo..',
            '..oWWWWWWWWWWo..',
            '..oooooooooooo..',
            '.....oooooo.....',
            '....oooooooo....',
            '................'
        ) }
    ) }
    # ** LA CARA DE RESOLUCION (R1 de EL_FOCO, 2026-10-10): una pantalla con
    # sus dos flechas de esquina a esquina, la medida que crece.
    @{ bex = 'apps\resol.bex'; recursos = @(
        @{ nombre = 'icono'; icono = @(
            '................',
            '.oooooooooooooo.',
            '.oWWWWWWWWWWWWo.',
            '.oWbbbWWWWWWWWo.',
            '.oWbbWWWWWWWWWo.',
            '.oWbWbWWWWWWWWo.',
            '.oWWWWbWWWWWWWo.',
            '.oWWWWWbWWWWWWo.',
            '.oWWWWWWbWWWWWo.',
            '.oWWWWWWWbWbWWo.',
            '.oWWWWWWWWWbbWo.',
            '.oWWWWWWWWbbbWo.',
            '.oWWWWWWWWWWWWo.',
            '.oooooooooooooo.',
            '......oooo......',
            '....oooooooo....'
        ) }
    ) }
    # ** LA CARA DE MUNDO (MC1 de PLAN_MUNDO, 2026-10-11): un bloque de
    # hierba en perspectiva -- su cara de arriba y sus dos lados --.
    @{ bex = 'apps\mundo.bex'; recursos = @(
        @{ nombre = 'icono'; icono = @(
            '................',
            '.......oo.......',
            '.....oovvoo.....',
            '...oovvvvvvoo...',
            '.oovvvvvvvvvvoo.',
            '.ottvvvvvvvvsso.',
            '.ottttvvvvsssso.',
            '.ottttttsssssso.',
            '.ottttttsssssso.',
            '.ottttttsssssso.',
            '.ottttttsssssso.',
            '.ottttttsssssso.',
            '..oottttssssoo..',
            '....oottssoo....',
            '......oooo......',
            '................'
        ) }
    ) }
    # ** LA CARA DE TINTA (TB1 de PLAN_LA_TINTA, 2026-10-10): una hoja con un
    # trazo de tinta, y la pluma que lo hizo.
    @{ bex = 'apps\tinta.bex'; recursos = @(
        @{ nombre = 'icono'; icono = @(
            '................',
            '.oooooooooo.....',
            '.oWWWWWWWWo..oo.',
            '.oWWWWWWWWo.obbo',
            '.oWWWWWWWWoobbo.',
            '.oWWWWWWWobbbo..',
            '.oWooWWWobbbo...',
            '.oWoooWobbbo....',
            '.oWWoooggbo.....',
            '.oWWWooogo......',
            '.oWWWWoooo......',
            '.oWWWWWWWo......',
            '.oWWWWWWWWo.....',
            '.oooooooooo.....',
            '................',
            '................'
        ) }
    ) }
)

# * EL FORMATO `BICO`, escrito aqui porque aqui es donde nace un icono.
#
#     0..4   "BICO"      4..6  ancho (u16)     6..8  alto (u16)
#     8..    ancho*alto pixeles BGRA, u32 little-endian
#
# 16x16 y el escritorio lo pinta al doble. Se guarda chico a proposito: la
# gracia de meter el icono en el paquete es que **no cueste nada llevarlo**, y
# un icono que engorda la app es un icono que alguien acabara quitando. 16x16
# son 1032 bytes; a 32x32 serian 4104.
#
# Los iconos se escriben como DIBUJO y no como una lista de numeros. Una rejilla
# de dieciseis lineas se lee, se corrige y se ve mal cuando esta mal; un array
# de 256 enteros no. Es el mismo criterio que `ring0\core\gato.rs`.
#
#   .  transparente (alfa 0: el escritorio se ve a traves)
#   o  contorno oscuro   R  rojo   d  rojo oscuro   W  blanco
#
# [!] Los colores van como **cuatro bytes en el orden del fichero (B, G, R, A)**
# y no como un `0xAARRGGBB`, por dos razones y la segunda escuece:
#
#   1. Asi la tabla dice el orden de bytes que se escribe, en vez de obligar a
#      recordar que un `u32` little-endian se guarda al reves de como se lee.
#   2. **PowerShell 5.1 lee `0xFFB4342A` como un `Int32` NEGATIVO** (-4967382) y
#      el cast a `uint32` revienta con "valor demasiado grande o demasiado
#      chico". Sin suffijo `u` en esta version, cualquier color con el alfa a
#      `FF` cae en la trampa -- o sea todos los opacos.
$BICO_PALETA = @{
    '.' = @(0x00, 0x00, 0x00, 0x00)
    'o' = @(0x10, 0x10, 0x1B, 0xFF)
    'R' = @(0x2A, 0x34, 0xB4, 0xFF)
    'd' = @(0x16, 0x1C, 0x6B, 0xFF)
    'W' = @(0xE0, 0xE6, 0xF0, 0xFF)
    # Los dos del bloc de notas: el azul de su barra y el gris de sus renglones.
    'b' = @(0xFF, 0xA6, 0x58, 0xFF)
    'g' = @(0x90, 0x83, 0x76, 0xFF)
    # Los tres de MUNDO (en BGRA, como todos): la hierba y sus dos lados de tierra.
    # (letras que no tenia nadie: las claves de PowerShell no miran mayusculas)
    'v' = @(0x30, 0xAA, 0x4C, 0xFF)
    't' = @(0x24, 0x48, 0x72, 0xFF)
    's' = @(0x18, 0x30, 0x4C, 0xFF)
}
