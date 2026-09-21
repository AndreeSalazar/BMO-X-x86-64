//! **Commands that talk to the DISK**: `ls`, `lee`, `escribe`, `guarda`.
//!
//! [consumo] NADA      no corre en reposo: lo pide el dueno escribiendo una
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
use crate::scene::output::{INK_ERR, INK_GOOD, INK_PLAIN};
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
                // `.` y `..` no se ensenan: aqui
                // no hay carpeta actual a la que
                // volver, asi que son ruido.
                if is_dot_entry(&nom[..length]) { return After::NextKey; }
                dsk.out.grid.text(b"  ");
                dsk.out.grid.text(&nom[..length]);
                // Alinear la columna del tamano.
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
/// ensena lo de DENTRO. Es la primera vez que un
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
                last = chunk[dsk.field.n - 1];
                total += dsk.field.n;
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
/// Lo pidio el dueno: *"que pueda dividir en carpetas, como mem.txt, cpu.txt"*.
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

pub(crate) fn save(dsk: &mut Desktop, p: &bmo::Pantalla, arg: &[u8]) -> After {
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
    match super::save_maestro::maestro(dsk, dest) {
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
            dsk.out.grid.text(b" lineas, 7 capitulos");
            // ** Y las hojas de `informe/`: 9 es el indice, las siete y DATOS.TXT. Menos
            // de 8 no es un fallo del informe --ya esta escrito--, es la
            // carpeta que no estaba o una ranura que no habia, y se dice.
            if hojas == 9 {
                dsk.out.grid.text(b"; y 9 hojas en informe/ (DATOS.TXT para maquinas)\n");
            } else {
                dsk.out.grid.with_ink(INK_ERR);
                dsk.out.grid.text(b"; en informe/ solo ");
                let k = decimal(hojas as u64, &mut d);
                dsk.out.grid.text(&d[..k]);
                dsk.out.grid.text(b" de 9 hojas (falta la carpeta en el disco de datos?)\n");
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
