//! **Commands that talk to the DISK**: `ls`, `lee`, `escribe`, `guarda`.
//!
//! [consumo] NADA      no corre en reposo: lo pide el propietario escribiendo una
//!                     orden en la caja de Ejecutar o pulsando su tecla de
//!                     funcion (L6h)
//!
//! They are together because they share the failure they can hit -- a
//! capability that is not granted, a FAT32 name that does not fit in 8.3, a
//! close that fails after every write succeeded. `file_error_reason` says
//! which one, and it is next door on purpose.

use bmo_userland as bmo;

use super::After;
use crate::desktop::Desktop;
use crate::commands::complete::file_error_reason;
use crate::scene::output::{INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_BAD, INK_DIM, INK_OK};
use crate::text::{decimal, is_dot_entry};
use crate::{dump_output, DEFAULT_DUMP};

pub(crate) fn list(dsk: &mut Desktop, p: &bmo::Pantalla, dir_path: &[u8]) -> After {
    match bmo::Directorio::open(dir_path) {
        Ok(d) => {
            let mut count = 0u32;
            // Tope por si un directorio enorme se
            // comiera el fotograma entero.
            while count < 256 {
                let e = match d.next() {
                    Some(e) => e,
                    None => break,
                };
                let mut nom = [0u8; 12];
                let length = e.legible(&mut nom);
                // `.` y `..` no se muestran: aqui
                // no hay carpeta actual a la que
                // volver, asi que son ruido.
                // (29-09: aqui habia un `return`, y el `.` que abre toda
                // subcarpeta de FAT cortaba el listado antes de empezar.)
                if is_dot_entry(&nom[..length]) { continue; }
                // La fila se toca (29-09): clic escribe `ls` o `lee` de esa
                // entrada, Ctrl+clic lo corre. Un `.bex` se lanza por su ruta.
                let prefijo: &[u8] = if e.es_dir {
                    b"ls "
                } else if nom[..length].ends_with(b".bex") {
                    b""
                } else {
                    b"lee "
                };
                crate::desktop::tocable::apuntar_entrada(dsk.out.grid.mark(), prefijo, dir_path, &nom[..length], e.es_dir);
                dsk.out.grid.text(b"  ");
                dsk.out.grid.text(&nom[..length]);
                // Alinear la columna del medida.
                let mut k = length;
                while k < 14 { dsk.out.grid.byte(b' '); k += 1; }
                if e.es_dir {
                    dsk.out.grid.text(b"<DIR>");
                } else {
                    let mut d10 = [0u8; 10];
                    let n10 = decimal(e.bytes as u64, &mut d10);
                    dsk.out.grid.text(&d10[..n10]);
                }
                dsk.out.grid.byte(b'\n');
                count += 1;
            }
            if count == 0 {
                dsk.out.grid.text(b"  (vacio)
");
            }
            paint_status(&p, &dsk.run_box, "listo", INK_DIM);
        }
        // * El MOTIVO, no un "no pude" para todo.
        //
        // Esto tiraba el codigo con `Err(_)` y
        // decia siempre "no puedo abrir esa
        // carpeta". Cuando la tabla de directorios
        // del kernel se lleno, eso fue una mentira
        // exacta: la carpeta estaba ahi, lo que no
        // habia era ranura. Y mando a buscar el
        // fallo al disco, que estaba perfecto.
        //
        // Un error que no distingue sus causas es
        // un error que manda a mirar donde no es.
        Err(cod) => {
            // 25 = sin hueco, 26 = no esta. Ver
            // `ring0/obj/directorio.rs`.
            let (line, estado): (&[u8], &str) = if cod == 25 {
                (
                    b"  no queda slot de directorio en el kernel.\n",
                    "sin ranura libre",
                )
            } else {
                (
                    b"  no puedo open esa carpeta.\n",
                    "carpeta no encontrada",
                )
            };
            dsk.out.grid.with_ink(INK_ERR);
            dsk.out.grid.text(line);
            dsk.out.grid.with_ink(INK_PLAIN);
            paint_status(&p, &dsk.run_box, estado, INK_BAD);
        }
    }
    dsk.field.n = 0;
    After::Settle
}

/// -- Leer un archivo --
///
/// El hermano de `ls`: aquel dice QUE hay, este
/// muestra lo de DENTRO. Es la primera vez que un
/// programa de Ring 3 abre un archivo del disco.
pub(crate) fn read(dsk: &mut Desktop, p: &bmo::Pantalla, file_path: &[u8]) -> After {
    match bmo::Archivo::leer_de(file_path) {
        Ok(a) => {
            let mut chunk = [0u8; 256];
            let mut total = 0usize;
            // El ultimo byte se guarda segun pasa:
            // reconstruirlo al final obligaria a
            // saber en que trozo cayo, y el buffer
            // ya se ha reutilizado.
            let mut last = 0u8;
            // De 256 en 256 y con tope: un archivo
            // que no sea texto llenaria la rejilla
            // de basura y se comeria el fotograma.
            loop {
                let got = a.read(&mut chunk);
                if got == 0 { break; }
                dsk.out.grid.text(&chunk[..got]);
                last = chunk[got - 1];
                total += got;
                if total >= 2048 {
                    dsk.out.grid.text(b"\n  ...(cortado)\n");
                    last = b'\n';
                    break;
                }
            }
            if total == 0 {
                dsk.out.grid.text(b"  (vacio)\n");
            } else if last != b'\n' {
                // Sin esto, el proximo mensaje se
                // pega al final del archivo.
                dsk.out.grid.byte(b'\n');
            }
            a.close();
            paint_status(&p, &dsk.run_box, "listo", INK_DIM);
        }
        Err(e) => {
            dsk.out.grid.with_ink(INK_ERR);
            dsk.out.grid.text(b"  ");
            dsk.out.grid.text(file_error_reason(e));
            dsk.out.grid.byte(b'\n');
            dsk.out.grid.with_ink(INK_PLAIN);
            paint_status(&p, &dsk.run_box, "no se pudo leer", INK_BAD);
        }
    }
    dsk.field.n = 0;
    After::Settle
}

// == EL DISCO PERSONAL (D:), SOLO PARA MIRAR (N1b, 2026-09-29) =================
//
// `personal ls` y `personal lee` van por las MISMAS puertas que `ls` y `lee`
// --`Directorio` y `Archivo`-- con `d:` delante de la ruta: el kernel ve el
// prefijo y contesta desde el NTFS. Escribir no existe en esta orden, y el
// kernel lo niega igual si alguien lo intentara por la puerta de `escribe`.

/// `d:` + la ruta, en `buf`. `None` si no cabe en el renglon del kernel (128).
fn ruta_d<'b>(ruta: &[u8], buf: &'b mut [u8; 128]) -> Option<&'b [u8]> {
    let ruta = match ruta {
        [a, b':', resto @ ..] if *a | 0x20 == b'd' => resto,
        _ => ruta,
    };
    if ruta.len() + 2 > buf.len() { return None; }
    buf[0] = b'd';
    buf[1] = b':';
    buf[2..2 + ruta.len()].copy_from_slice(ruta);
    Some(&buf[..2 + ruta.len()])
}

/// Un nombre UTF-8 a la rejilla, que solo sabe ASCII: cada letra que no lo
/// es sale como `?` (una por letra, no una por byte).
fn nombre_ascii(dsk: &mut Desktop, nombre: &[u8]) -> usize {
    let mut n = 0;
    for &c in nombre {
        if c < 0x80 {
            dsk.out.grid.byte(if c < 0x20 { b'?' } else { c });
            n += 1;
        } else if c & 0xC0 != 0x80 {
            dsk.out.grid.byte(b'?');
            n += 1;
        }
    }
    n
}

/// Una medida que se lee: bytes hasta 1 MiB, y de ahi MiB (un `.archive` de
/// Cyberpunk pasa de los diez digitos que caben en `decimal`).
fn medida(dsk: &mut Desktop, bytes: u64) {
    let mut d10 = [0u8; 10];
    let (v, u): (u64, &[u8]) = if bytes < 1 << 20 { (bytes, b" B") } else { (bytes >> 20, b" MiB") };
    let n = decimal(v, &mut d10);
    dsk.out.grid.text(&d10[..n]);
    dsk.out.grid.text(u);
}

pub(crate) fn personal_ls(dsk: &mut Desktop, p: &bmo::Pantalla, ruta: &[u8]) -> After {
    let mut buf = [0u8; 128];
    let d = match ruta_d(ruta, &mut buf).map(bmo::Directorio::open) {
        Some(Ok(d)) => d,
        // `personal <ruta>` con la ruta de un FICHERO (metal, 29-09 13:18:
        // `personal Cyberpunk 2077/REDprelauncher.exe`): es un `lee`.
        Some(Err(26)) if ruta_d(ruta, &mut [0u8; 128]).map(bmo::Archivo::reflejar).is_some_and(|a| a.is_ok_and(|a| a.close())) => {
            return personal_lee(dsk, p, ruta);
        }
        otro => {
            let (line, estado): (&[u8], &str) = match otro {
                None => (b"  esa ruta es demasiado larga (128 bytes con el d:).\n", "ruta larga"),
                Some(Err(25)) => (b"  no queda slot de directorio en el kernel.\n", "sin ranura libre"),
                _ => (
                    b"  D: no esta montado, o esa carpeta no existe (mira `disco`: la fila personal).\n",
                    "carpeta no encontrada",
                ),
            };
            dsk.out.grid.with_ink(INK_ERR);
            dsk.out.grid.text(line);
            dsk.out.grid.with_ink(INK_PLAIN);
            paint_status(&p, &dsk.run_box, estado, INK_BAD);
            dsk.field.n = 0;
            return After::Settle;
        }
    };
    let mut nombre = [0u8; 256];
    let (mut vistas, mut ocultas, mut todas) = (0u32, 0u32, 0u32);
    // Tope por si una carpeta enorme se comiera el fotograma; y `todas` sigue
    // contando para decir cuantas quedaron sin pintar.
    while let Some((largo, carpeta, bytes)) = d.siguiente_largo(&mut nombre) {
        todas += 1;
        let nom = &nombre[..largo];
        // Los ficheros del propio NTFS (`$MFT`, `$Bitmap`...): Windows
        // tampoco los muestra.
        if nom.first() == Some(&b'$') { ocultas += 1; continue; }
        // `.`: la raiz de NTFS se tiene a si misma en su indice.
        if is_dot_entry(nom) { todas -= 1; continue; }
        if vistas >= 256 { continue; }
        // La fila se toca (29-09): con el nombre ENTERO, aunque en pantalla
        // salga cortado o con `?` (ver `desktop::tocable`).
        let prefijo: &[u8] = if carpeta { b"personal ls " } else { b"personal lee " };
        crate::desktop::tocable::apuntar_entrada(dsk.out.grid.mark(), prefijo, ruta, nom, carpeta);
        dsk.out.grid.text(b"  ");
        let mut k = nombre_ascii(dsk, nom);
        while k < 40 { dsk.out.grid.byte(b' '); k += 1; }
        if carpeta { dsk.out.grid.text(b"<DIR>"); } else { medida(dsk, bytes); }
        dsk.out.grid.byte(b'\n');
        vistas += 1;
    }
    drop(d);
    let mut d10 = [0u8; 10];
    dsk.out.grid.with_ink(INK_ECHO);
    if vistas == 0 && ocultas == 0 {
        dsk.out.grid.text(b"  (vacio)\n");
    }
    let visibles = todas - ocultas;
    if visibles > vistas {
        dsk.out.grid.text(b"  ...y ");
        let n = decimal((visibles - vistas) as u64, &mut d10);
        dsk.out.grid.text(&d10[..n]);
        dsk.out.grid.text(b" mas sin pintar\n");
    }
    if ocultas > 0 {
        dsk.out.grid.text(b"  (");
        let n = decimal(ocultas as u64, &mut d10);
        dsk.out.grid.text(&d10[..n]);
        dsk.out.grid.text(b" del propio NTFS, con $ delante, sin mostrar)\n");
    }
    dsk.out.grid.text(b"  D: es SOLO LECTURA: BMO-X no escribe ahi.\n");
    dsk.out.grid.with_ink(INK_PLAIN);
    paint_status(&p, &dsk.run_box, "listo", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// La ultima ruta de `personal censo` / `personal diario` (30-09): sin ruta,
/// se repite. Lo pidio el propietario: *"al escribir `"d:` ya da flojera"*.
/// Un solo hilo (el del escritorio) la lee y la escribe, y solo al dar Enter.
static mut ULTIMA: ([u8; 96], usize) = ([0; 96], 0);

/// **La linea de PROTON-X para `personal censo <ruta>`** (29-09):
/// `sys/proton-x.bex --censo "d:<ruta>"`, en `buf`. La lanza el editor
/// (`Edit::Launch`). Con `diario`, `personal diario <ruta>` (30-09): el
/// primer contacto, `--diario` en vez de `--censo`.
///
/// La ruta se LIMPIA (30-09): sin espacios a los lados, sin comillas, sin
/// `d:`/`D:` delante y con `\` como `/` (lo que se copia de Windows). Sin
/// ruta, la ultima. El `Err` es la frase que se le dice al propietario.
pub(crate) fn linea_censo(ruta: &[u8], diario: bool, buf: &mut [u8; crate::PATH_MAX]) -> Result<usize, &'static [u8]> {
    let mut r = ruta;
    while let [b' ' | b'"', resto @ ..] = r {
        r = resto;
    }
    while let [resto @ .., b' ' | b'"'] = r {
        r = resto;
    }
    if let [a, b':', resto @ ..] = r {
        if *a | 0x20 == b'd' {
            r = resto;
        }
    }
    let mut limpia = [0u8; 96];
    if r.len() > limpia.len() {
        return Err(b"la ruta es demasiado larga para los argumentos de PROTON-X (96 bytes)");
    }
    for (k, &c) in r.iter().enumerate() {
        limpia[k] = if c == b'\\' { b'/' } else { c };
    }
    // SAFETY: ver `ULTIMA`: el hilo del escritorio, al dar Enter.
    let ultima = unsafe { &mut *core::ptr::addr_of_mut!(ULTIMA) };
    let ruta: &[u8] = if r.is_empty() {
        if ultima.1 == 0 {
            return Err(b"sin ruta, y todavia no hay una ultima: personal diario <ruta> (TAB completa)");
        }
        &ultima.0[..ultima.1]
    } else {
        ultima.0[..r.len()].copy_from_slice(&limpia[..r.len()]);
        ultima.1 = r.len();
        &limpia[..r.len()]
    };
    const PROGRAMA: &[u8] = b"sys/proton-x.bex ";
    let bandera: &[u8] = if diario { b"--diario \"d:" } else { b"--censo \"d:" };
    let partes: [&[u8]; 4] = [PROGRAMA, bandera, ruta, b"\""];
    let n: usize = partes.iter().map(|x| x.len()).sum();
    if n > buf.len() || n - PROGRAMA.len() > 96 {
        return Err(b"la ruta es demasiado larga para los argumentos de PROTON-X (96 bytes)");
    }
    let mut k = 0;
    for x in partes {
        buf[k..k + x.len()].copy_from_slice(x);
        k += x.len();
    }
    Ok(n)
}

/// `personal lee <fichero>`: la medida, los primeros 64 bytes en hex y, si
/// empieza por `MZ`, si es un PE de verdad (su firma `PE\0\0` donde dice
/// `e_lfanew`). Es lo que PROTON-X mirara primero de un `.exe` de D:.
pub(crate) fn personal_lee(dsk: &mut Desktop, p: &bmo::Pantalla, ruta: &[u8]) -> After {
    let mut buf = [0u8; 128];
    let a = match ruta_d(ruta, &mut buf) {
        Some(r) => bmo::Archivo::reflejar(r),
        None => Err(bmo::ERROR_ARCH_NO_ESTA),
    };
    let a = match a {
        Ok(a) => a,
        Err(e) => {
            dsk.out.grid.with_ink(INK_ERR);
            dsk.out.grid.text(b"  ");
            dsk.out.grid.text(file_error_reason(e));
            dsk.out.grid.byte(b'\n');
            dsk.out.grid.with_ink(INK_PLAIN);
            paint_status(&p, &dsk.run_box, "no se pudo leer", INK_BAD);
            dsk.field.n = 0;
            return After::Settle;
        }
    };
    let total = a.size();
    dsk.out.grid.text(b"  medida: ");
    medida(dsk, total);
    dsk.out.grid.byte(b'\n');
    let mut cab = [0u8; 64];
    let n = a.read(&mut cab);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for fila in cab[..n].chunks(16) {
        dsk.out.grid.text(b"  ");
        for &c in fila {
            dsk.out.grid.byte(HEX[(c >> 4) as usize]);
            dsk.out.grid.byte(HEX[(c & 15) as usize]);
            dsk.out.grid.byte(b' ');
        }
        for _ in fila.len()..16 { dsk.out.grid.text(b"   "); }
        dsk.out.grid.byte(b' ');
        for &c in fila {
            dsk.out.grid.byte(if (0x20..0x7F).contains(&c) { c } else { b'.' });
        }
        dsk.out.grid.byte(b'\n');
    }
    if n >= 0x40 && cab[0] == b'M' && cab[1] == b'Z' {
        let pe = u32::from_le_bytes([cab[0x3C], cab[0x3D], cab[0x3E], cab[0x3F]]) as u64;
        let mut firma = [0u8; 6];
        let ok = pe + 6 <= total && a.saltar(pe) == pe && a.read(&mut firma) == 6 && firma[..4] == *b"PE\0\0";
        if ok {
            let maquina = u16::from_le_bytes([firma[4], firma[5]]);
            dsk.out.grid.with_ink(INK_GOOD);
            dsk.out.grid.text(match maquina {
                0x8664 => b"  MZ + PE: un ejecutable de Windows para x86-64.\n" as &[u8],
                0x014C => b"  MZ + PE: un ejecutable de Windows de 32 bits (x86).\n",
                _ => b"  MZ + PE: un ejecutable de Windows (otra maquina).\n",
            });
        } else {
            dsk.out.grid.with_ink(INK_ECHO);
            dsk.out.grid.text(b"  empieza por MZ pero sin firma PE: un ejecutable de DOS.\n");
        }
        dsk.out.grid.with_ink(INK_PLAIN);
    } else if n == 0 && total > 0 {
        dsk.out.grid.with_ink(INK_ERR);
        dsk.out.grid.text(b"  no llego ni un byte del disco.\n");
        dsk.out.grid.with_ink(INK_PLAIN);
    }
    a.close();
    paint_status(&p, &dsk.run_box, "listo", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// -- Escribir un archivo --
///
/// Lo que NUNCA habia pasado: un programa de Ring 3
/// dejando algo en el disco. Hasta hoy todo lo que
/// habia ahi lo puso el anfitrion al flashear o el
/// kernel con su caja negra.
pub(crate) fn write(dsk: &mut Desktop, p: &bmo::Pantalla, file_path: &[u8], text: &[u8]) -> After {
    match bmo::Archivo::create(file_path) {
        Ok(a) => {
            let placed = a.write(text);
            // El salto final: un archivo de texto
            // sin el ultimo salto es el clasico
            // que descuadra al siguiente que lo lee.
            a.write(b"\n");
            // * Aqui es donde llega al disco. Antes
            // de esto no hay nada escrito.
            if a.close() {
                dsk.out.grid.text(b"  guardado: ");
                let mut d10 = [0u8; 10];
                let n10 = decimal(placed as u64 + 1, &mut d10);
                dsk.out.grid.text(&d10[..n10]);
                dsk.out.grid.text(b" bytes\n");
                paint_status(&p, &dsk.run_box, "guardado", INK_OK);
            } else {
                dsk.out.grid.text(b"  no se guardo nada.\n");
                paint_status(&p, &dsk.run_box, "no se pudo guardar", INK_BAD);
            }
        }
        Err(e) => {
            dsk.out.grid.with_ink(INK_ERR);
            dsk.out.grid.text(b"  ");
            dsk.out.grid.text(file_error_reason(e));
            dsk.out.grid.byte(b'\n');
            dsk.out.grid.with_ink(INK_PLAIN);
            paint_status(&p, &dsk.run_box, "no se pudo crear", INK_BAD);
        }
    }
    dsk.field.n = 0;
    After::Settle
}

/// -- Volcar el historial a un .txt --
///
/// El hermano manual del volcado automatico: aquel
/// guarda lo de UNA corrida, este guarda todo lo que
/// quede en el historial, que es lo que hace falta
/// cuando lo interesante son tres comandos juntos.
/// **`save <tema>`: un informe, su fichero, y nada mas dentro.**
///
/// Devuelve `(ruta, cual)` si `arg` es uno de los temas, o `None` si es una ruta
/// normal.
///
/// # Por que en ficheros separados y no todo en uno
///
/// Lo pidio el propietario: *"que pueda dividir en carpetas, como mem.txt, cpu.txt"*.
/// Y tiene el motivo a favor: `salida.txt` crece con todo lo que se ha tecleado
/// en la sesion, asi que para comparar la memoria de antes y despues de matar un
/// programa hay que buscar dos trozos dentro de un fichero largo. Cuatro
/// ficheros cortos con **una sola tabla cada uno** se comparan poniendolos uno
/// al lado del otro.
///
/// [!] La ruta se distingue del tema por **igualdad exacta**: `mem` es el tema y
/// `mem.txt` es una ruta. Adivinar cual queria seria escribir en el sitio
/// equivocado, que en un disco es peor que no escribir.
///
/// `apps` lleva las dos tablas de programas: la memoria pedida y la ficha
/// BEF2 (2026-09-20). `disco` y `autopsia` entraron el mismo dia: eran los dos
/// capitulos del maestro que no se podian pedir sueltos.
/// Los temas sueltos van a la misma carpeta que las hojas del informe
/// maestro (2026-09-21): `informe/` es donde se lee, `datos/` es lo que
/// leen los programas.
fn tema(arg: &[u8]) -> Option<(&'static [u8], u8)> {
    match arg {
        b"cpu" => Some((b"informe/cpu.txt", 0)),
        b"mem" | b"ram" => Some((b"informe/mem.txt", 1)),
        b"consumo" | b"gasto" | b"w" => Some((b"informe/consumo.txt", 2)),
        b"apps" | b"programas" | b"bef" => Some((b"informe/apps.txt", 3)),
        b"disco" => Some((b"informe/disco.txt", 4)),
        b"autopsia" => Some((b"informe/autopsia.txt", 5)),
        _ => None,
    }
}

// == LOS DOS MODOS DEL `save` (2026-09-24) =====================================
//
// Peticion del propietario: *"si me dan sorpresa, eso es el motivo"*. Un fallo
// de Ring 0 mata la sesion y el `save` que no se hizo antes ya no se hace. Asi
// que el `save` tiene dos modos:
//
//    AUTOMATICO   antes de cada orden ARRIESGADA (las que escriben en el
//                 hardware: hoy `iommu encender` e `iommu apagar`), el
//                 escritorio guarda el informe maestro SOLO, y la orden no
//                 corre si el save no se pudo escribir
//    MANUAL       solo cuando el propietario teclea `save`
//
// Por defecto AUTOMATICO: la maquina trabaja para quien la usa. `save auto` y
// `save manual` lo cambian. Y `save mode` es otra cosa: la VERIFICACION
// TOTAL (`verificar.rs`), que guarda antes de cada paso en los dos modos.

static SAVE_AUTOMATICO: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(true);

/// **Antes de una orden arriesgada.** En modo automatico guarda el informe
/// maestro y devuelve si se pudo; en manual, `true` sin hacer nada.
pub(crate) fn antes_de_arriesgar(dsk: &mut Desktop, p: &bmo::Pantalla, que: &[u8]) -> bool {
    if !SAVE_AUTOMATICO.load(core::sync::atomic::Ordering::Relaxed) {
        return true;
    }
    dsk.out.grid.with_ink(INK_ECHO);
    dsk.out.grid.text(b"  save AUTOMATICO antes de `");
    dsk.out.grid.text(que);
    dsk.out.grid.text(b"` (si algo cae, esto ya esta en el disco):\n");
    dsk.out.grid.with_ink(INK_PLAIN);
    match super::save_maestro::maestro(dsk, DEFAULT_DUMP, p.rayo()) {
        Ok(_) => {
            dsk.out.grid.with_ink(INK_GOOD);
            dsk.out.grid.text(b"  save automatico GUARDADO en ");
            dsk.out.grid.text(DEFAULT_DUMP);
            dsk.out.grid.byte(b'\n');
            dsk.out.grid.with_ink(INK_PLAIN);
            true
        }
        Err(e) => {
            dsk.out.grid.with_ink(INK_ERR);
            dsk.out.grid.text(b"  save automatico NO se pudo (");
            dsk.out.grid.text(file_error_reason(e));
            dsk.out.grid.text(b"): la orden NO se hace\n");
            dsk.out.grid.with_ink(INK_PLAIN);
            false
        }
    }
}

fn modo_del_save(dsk: &mut Desktop, p: &bmo::Pantalla, arg: &[u8]) -> Option<After> {
    use core::sync::atomic::Ordering;
    match arg {
        b"auto" | b"automatico" => SAVE_AUTOMATICO.store(true, Ordering::Relaxed),
        b"manual" => SAVE_AUTOMATICO.store(false, Ordering::Relaxed),
        _ => return None,
    }
    let auto = SAVE_AUTOMATICO.load(Ordering::Relaxed);
    dsk.out.grid.with_ink(if auto { INK_GOOD } else { INK_ERR });
    dsk.out.grid.text(if auto {
        b"  save AUTOMATICO: se guarda solo antes de cada orden que escribe en el hardware\n" as &[u8]
    } else {
        b"  save MANUAL: solo cuando lo teclees. Una orden arriesgada que tumbe la maquina se lleva la sesion\n"
    });
    dsk.out.grid.with_ink(INK_PLAIN);
    paint_status(p, &dsk.run_box, if auto { "save automatico" } else { "save manual" }, INK_DIM);
    dsk.field.n = 0;
    Some(After::Settle)
}

pub(crate) fn save(dsk: &mut Desktop, p: &bmo::Pantalla, arg: &[u8]) -> After {
    // `save auto`, `save manual`, `save modo`: el modo, no un fichero con ese
    // nombre.
    if let Some(a) = modo_del_save(dsk, p, arg) {
        return a;
    }
    // `save mode [-paso ...]`: la verificacion total. Ver `verificar.rs`.
    if let Some(a) = super::verificar::save_mode(dsk, p, arg) {
        return a;
    }
    // ** `save <tema>`: solo esa tabla, en su propio fichero.
    if let Some((dest, cual)) = tema(arg) {
        // La marca se toma ANTES de pintar: lo que va al fichero es exactamente
        // lo que esta funcion escriba a partir de aqui y ni una linea de lo que
        // hubiera antes en la pantalla. `rows_since` ya recorta si el anillo se
        // dio la vuelta, asi que no hay forma de pedir filas que ya no estan.
        let marca = dsk.out.grid.mark();
        match cual {
            0 => super::reports::report_cpu(&mut dsk.out.grid, dsk.tick.consumo.ultimo),
            1 => super::reports::report_memory(&mut dsk.out.grid),
            2 => super::reports::report_consumo(&mut dsk.out.grid, &dsk.tick),
            3 => {
                super::reports::report_apps(&mut dsk.out.grid);
                super::save_maestro::report_programas(&mut dsk.out.grid);
            }
            4 => super::reports::report_disco(&mut dsk.out.grid),
            _ => super::reports::report_autopsy(&mut dsk.out.grid),
        }
        let (from, to) = dsk.out.grid.rows_since(marca);
        match dump_output(&dsk.out.grid, dest, from, to) {
            Ok(bytes) => {
                dsk.out.grid.with_ink(INK_GOOD);
                dsk.out.grid.text(b"  guardado en ");
                dsk.out.grid.text(dest);
                dsk.out.grid.text(b": ");
                let mut d = [0u8; 10];
                let k = decimal(bytes as u64, &mut d);
                dsk.out.grid.text(&d[..k]);
                dsk.out.grid.text(b" bytes\n");
                dsk.out.grid.with_ink(INK_PLAIN);
                paint_status(&p, &dsk.run_box, "volcado", INK_OK);
            }
            Err(e) => {
                dsk.out.grid.with_ink(INK_ERR);
                dsk.out.grid.text(b"  ");
                dsk.out.grid.text(file_error_reason(e));
                dsk.out.grid.byte(b'\n');
                dsk.out.grid.with_ink(INK_PLAIN);
                paint_status(&p, &dsk.run_box, "no se pudo guardar", INK_BAD);
            }
        }
        dsk.field.n = 0;
        return After::Settle;
    }

    let dest = if arg.is_empty() { DEFAULT_DUMP } else { arg };
    // ** `save` A SECAS ES EL INFORME MAESTRO (2026-09-20): la sesion y
    // despues TODO lo que la maquina sabe decir, en siete capitulos y siempre
    // en el mismo orden. Hasta hoy era el historial con la tabla de consumo
    // pegada al final --una mitad de la maquina--, y cada vez que hacia falta
    // la otra mitad habia que pedir otro arranque con un informe puesto a
    // mano. Este `.txt` es lo unico que cruza del Ryzen al otro lado, y en
    // esta maquina un viaje se mide en reinicios.
    //
    // Sigue pasando TODO por la pantalla (un solo camino de salida: lo que
    // esta en el fichero se vio); el porque va por capitulos esta en la
    // cabecera de `save_maestro.rs`.
    match super::save_maestro::maestro(dsk, dest, p.rayo()) {
        Ok((bytes, lineas, hojas)) => {
            dsk.out.grid.with_ink(INK_GOOD);
            dsk.out.grid.text(b"  guardado en ");
            dsk.out.grid.text(dest);
            dsk.out.grid.text(b": ");
            let mut d = [0u8; 10];
            let k = decimal(bytes as u64, &mut d);
            dsk.out.grid.text(&d[..k]);
            dsk.out.grid.text(b" bytes, ");
            let k = decimal(lineas as u64, &mut d);
            dsk.out.grid.text(&d[..k]);
            dsk.out.grid.text(b" lineas, 8 capitulos");
            // ** Y las hojas de `informe/`: 10 es el indice, las ocho y DATOS.TXT. Menos
            // no es un fallo del informe --ya esta escrito--, es la carpeta
            // que no estaba o una ranura que no habia, y se dice.
            if hojas == 10 {
                dsk.out.grid.text(b"; y 10 hojas en informe/ (DATOS.TXT para maquinas)\n");
            } else {
                dsk.out.grid.with_ink(INK_ERR);
                dsk.out.grid.text(b"; en informe/ solo ");
                let k = decimal(hojas as u64, &mut d);
                dsk.out.grid.text(&d[..k]);
                dsk.out.grid.text(b" de 10 hojas (falta la carpeta en el disco de datos?)\n");
            }
            dsk.out.grid.with_ink(INK_PLAIN);
            paint_status(&p, &dsk.run_box, "volcado", INK_OK);
        }
        Err(0) => {
            dsk.out.grid.with_ink(INK_ERR);
            dsk.out.grid.text(b"  no se guardo nada. el motivo esta en F11.\n");
            dsk.out.grid.with_ink(INK_PLAIN);
            paint_status(&p, &dsk.run_box, "no se pudo guardar", INK_BAD);
        }
        Err(e) => {
            dsk.out.grid.with_ink(INK_ERR);
            dsk.out.grid.text(b"  ");
            dsk.out.grid.text(file_error_reason(e));
            dsk.out.grid.byte(b'\n');
            dsk.out.grid.with_ink(INK_PLAIN);
            paint_status(&p, &dsk.run_box, "no se pudo crear", INK_BAD);
        }
    }
    dsk.field.n = 0;
    After::Settle
}
