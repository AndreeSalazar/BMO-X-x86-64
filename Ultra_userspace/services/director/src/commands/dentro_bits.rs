//! `dentro bits OFFSET RUTA`: la vista cruda de la lente, dentro de BMO-X.
//!
//! [consumo] NADA      solo a peticion; una pagina de 64 bytes, solo lectura
//!
//! La consola guarda 200 filas: un volcado entero perderia su comienzo.
//! Se lee un intervalo acotado y se imprime como continuar, sin suponer que
//! los bytes son instrucciones. La vista y los limites son de la lente pura.

use bmo_radiografia::lente::crudo;
use bmo_userland as bmo;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_PLAIN};

pub(crate) fn mostrar(s: &mut Output, args: &[u8]) {
    let args = args.trim_ascii();
    let Some(corte) = args.iter().position(u8::is_ascii_whitespace) else {
        ayuda(s);
        return;
    };
    let ruta = args[corte..].trim_ascii();
    let offset = core::str::from_utf8(&args[..corte]).ok().and_then(|t| {
        if let Some(h) = t.strip_prefix("0x") { u64::from_str_radix(h, 16).ok() }
        else { t.parse::<u64>().ok() }
    });
    let Some(desde) = offset.filter(|_| !ruta.is_empty()) else {
        ayuda(s);
        return;
    };
    let Ok(a) = bmo::Archivo::leer_de(ruta) else {
        error(s, b"  no encuentro ese fichero para leer\n");
        return;
    };
    // size() es lo que QUEDA, no el total: se pregunta antes de mover cursor.
    let total = a.size();
    let Some(p) = crudo::pagina(total, desde) else {
        error(s, b"  ese offset esta despues del final del fichero\n");
        return;
    };
    s.with_ink(INK_ECHO);
    s.text(b"  == BYTES DEL FICHERO: "); s.text(ruta); s.byte(b'\n');
    s.with_ink(INK_PLAIN);
    s.text(b"  offsets de fichero, hex y bits 7..0; no son direcciones de RAM\n");
    s.text(b"  total "); s.dec(total); s.text(b" B; desde 0x"); s.hex(desde, 1);
    s.text(b" hasta 0x"); s.hex(p.hasta, 1); s.text(b" (fin excluido)\n");
    if p.bytes == 0 {
        s.text(b"  fin del fichero; no hay bytes en esta posicion\n");
        return;
    }
    let Some(mem) = bmo::Memoria::request(crudo::POR_PAGINA as u64) else {
        error(s, b"  sin memoria para esta pagina\n");
        return;
    };
    // saltar mueve el cursor del FICHERO; leer_en recibe el offset del BLOQUE.
    if a.saltar(desde) != desde {
        error(s, b"  no se pudo situar el cursor; no se muestran bytes\n");
        return;
    }
    let n = a.leer_en(&mem, 0, p.bytes as u64);
    if n > p.bytes as u64 {
        error(s, b"  lectura fuera de su ventana; no se muestran bytes\n");
        return;
    }
    // SAFETY: n <= bytes pedidos <= 64, dentro de la memoria concedida a este
    // proceso. Solo se lee lo que el kernel dice haber escrito en el bloque.
    let bytes = unsafe { core::slice::from_raw_parts(mem.base() as *const u8, n as usize) };
    for (k, bytes) in bytes.chunks(crudo::POR_FILA).enumerate() {
        if let Some(f) = crudo::fila(desde + (k * crudo::POR_FILA) as u64, bytes) {
            s.text(f.as_str().as_bytes()); s.byte(b'\n');
        }
    }
    if n != p.bytes as u64 {
        error(s, b"  lectura incompleta: esta pagina NO queda demostrada; repetir\n");
        return;
    }
    if let Some(sig) = p.siguiente {
        s.text(b"  siguiente: dentro bits 0x"); s.hex(sig, 1); s.byte(b' ');
        s.text(ruta); s.byte(b'\n');
    } else {
        s.text(b"  fin del fichero; pagina completa\n");
    }
}

fn ayuda(s: &mut Output) {
    s.text(b"  dentro bits OFFSET RUTA: 64 bytes en hex y bits, solo lectura\n");
    s.text(b"  OFFSET decimal o hexadecimal (0x0 para empezar); RUTA puede tener espacios\n");
}

fn error(s: &mut Output, texto: &[u8]) {
    s.with_ink(INK_ERR); s.text(texto); s.with_ink(INK_PLAIN);
}
