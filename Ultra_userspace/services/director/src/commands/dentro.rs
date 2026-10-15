//! **`dentro RUTA`: VER POR DENTRO un `.bex`, con los ojos del kernel** (15-10).
//!
//! > El propietario: *"click derecho y ver todo lo que hay adentro pero para
//! > leer que son cada uno y que cumplen, es el lente de mi Kernel"*.
//!
//! [consumo] NADA      corre cuando alguien lo pide (la orden o el clic derecho)
//!
//! Lo que se dice sale de `bmo-radiografia`, que son las tres piezas con las
//! que el kernel admite un programa -- la puerta, los papeles y la lente --
//! sobre el fichero entero. Si aqui dice que entra, el kernel lo deja entrar.
//!
//! El fichero se lee en UN bloque prestado (como el fondo de pantalla) y el
//! mapa de la lente en otro; los dos vuelven al acabar.

use bmo_radiografia::{nombre_anexo, nombre_clase, nombres_instrucciones, radiografiar, Cual};
use bmo_userland as bmo;

use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};

/// Un `.bex` de mas de esto no se radiografia entero (DOOM mide ~1,7 MiB).
const TOPE: u64 = 32 << 20;
/// El mapa de la lente: un bit por byte de codigo (cubre 8 MiB) y lo pendiente.
const VISTO: u64 = 1 << 20;
const PILA: u64 = 64 * 1024;

pub(crate) fn dentro(dsk: &mut Desktop, _p: &bmo::Pantalla, ruta: &[u8]) -> After {
    let s = &mut dsk.out.grid;
    let ruta = ruta.trim_ascii();
    if ruta.is_empty() {
        s.text(b"  dentro RUTA: lo que hay dentro de un .bex (o clic derecho > ver por dentro)\n");
        return After::Settle;
    }
    let Ok(a) = bmo::Archivo::leer_de(ruta) else {
        s.with_ink(INK_ERR);
        s.text(b"  no encuentro ese fichero\n");
        s.with_ink(INK_PLAIN);
        return After::Settle;
    };
    let mide = a.size();
    if mide > TOPE {
        s.with_ink(INK_ERR);
        s.text(b"  pasa de 32 MiB: no se radiografia\n");
        s.with_ink(INK_PLAIN);
        return After::Settle;
    }
    let (Some(fichero), Some(mapa)) = (bmo::Memoria::request(mide.max(1)), bmo::Memoria::request(VISTO + PILA * 4)) else {
        s.with_ink(INK_ERR);
        s.text(b"  sin memoria para leerlo\n");
        s.with_ink(INK_PLAIN);
        return After::Settle;
    };
    let n = a.leer_en(&fichero, 0, mide) as usize;
    // SAFETY: `n` bytes que el kernel acaba de escribir en un bloque de este
    // proceso; el mapa es otro bloque suyo, de pagina, partido en dos que no
    // se pisan (el bit por byte y la pila de u32, alineada a 4).
    let bytes = unsafe { core::slice::from_raw_parts(fichero.base() as *const u8, n) };
    let visto = unsafe { core::slice::from_raw_parts_mut(mapa.base() as *mut u8, VISTO as usize) };
    let pila = unsafe { core::slice::from_raw_parts_mut(mapa.base().add(VISTO as usize) as *mut u32, PILA as usize) };
    let r = radiografiar(bytes, visto, pila);

    s.with_ink(INK_ECHO);
    s.text(b"  == POR DENTRO: ");
    s.text(ruta);
    s.text(b"  (");
    s.dec(r.bytes);
    s.text(b" bytes, leido con la puerta, los papeles y la lente del kernel)\n");
    s.with_ink(INK_PLAIN);
    if let Some(f) = r.falta {
        s.with_ink(INK_ERR);
        s.text(b"  la PUERTA dice que NO: ");
        s.text(f.nombre().as_bytes());
        s.text(b"\n  el kernel no lo dejaria entrar; no hay mas que mirar\n");
        s.with_ink(INK_PLAIN);
        return After::Settle;
    }
    // Las regiones: lo que se carga en memoria.
    s.text(b"  regiones   ");
    for (i, c) in Cual::TODAS.iter().enumerate() {
        if let Some(reg) = r.regiones[i] {
            s.text(c.nombre().as_bytes());
            s.byte(b' ');
            s.dec(reg.mem_size);
            s.text(b" B   ");
        }
    }
    s.byte(b'\n');
    s.text(b"  entrada    +0x");
    s.hex(r.entrada, 4);
    s.text(b" del codigo   xcr0 0x");
    s.hex(r.xcr0, 2);
    s.text(b"   relocaciones ");
    s.dec(r.relocs as u64);
    s.byte(b'\n');
    // Los anexos: lo que viaja pegado al programa.
    s.text(b"  anexos     ");
    for a in &r.anexos[..r.cuantos_anexos.min(bmo_radiografia::ANEXOS_MAX)] {
        s.text(nombre_anexo(a.tipo).as_bytes());
        s.byte(b' ');
        s.dec(a.bytes);
        s.text(b" B; ");
    }
    s.byte(b'\n');
    // Los papeles: lo que pide.
    if r.cuantos_requisitos > 0 {
        s.text(b"  pide       ");
        for q in &r.requisitos[..r.cuantos_requisitos.min(bmo_radiografia::REQUISITOS_MAX)] {
            s.text(nombre_clase(q.clase).as_bytes());
            s.byte(b' ');
            s.dec(q.cantidad);
            s.text(if q.obligatorio { b"; " as &[u8] } else { b" (si hay); " });
        }
        s.byte(b'\n');
    }
    s.text(b"  cpu        ");
    let mut alguna = false;
    for nom in nombres_instrucciones(r.declara) {
        s.text(nom.as_bytes());
        s.byte(b' ');
        alguna = true;
    }
    if !alguna {
        s.text(b"solo x86-64 base");
    }
    s.byte(b'\n');
    // La lente: el codigo, instruccion a instruccion.
    match r.lente {
        None => {
            s.with_ink(INK_ECHO);
            s.text(b"  lente      no se paso (sin codigo, o el codigo pasa de 8 MiB)\n");
        }
        Some(l) => {
            s.text(b"  lente      ");
            s.dec(l.leidas as u64);
            s.text(b" instrucciones, ");
            s.dec(l.bytes as u64);
            s.text(b" bytes de codigo alcanzables; ");
            s.dec(l.indirectos as u64);
            s.text(b" saltos indirectos\n");
            if l.demostrado() {
                s.with_ink(INK_GOOD);
                s.text(b"             DEMOSTRADO: todo lo alcanzable se leyo, es de lo declarado y no se sale\n");
            } else if let Some((en, ext)) = l.no_declarada {
                s.with_ink(INK_ERR);
                s.text(b"             NO: en +0x");
                s.hex(en as u64, 4);
                s.text(b" usa ");
                s.text(ext.as_bytes());
                s.text(b" sin declararlo (el kernel lo rechaza)\n");
            } else {
                s.with_ink(INK_ECHO);
                s.text(b"             sin demostrar (no es un NO): ");
                if l.no_sabe.is_some() {
                    s.text(b"algo que la lente no sabe leer; ");
                }
                if l.fuera.is_some() {
                    s.text(b"un salto que se sale del codigo; ");
                }
                if l.desborda {
                    s.text(b"el mapa se lleno");
                }
                s.byte(b'\n');
            }
        }
    }
    s.with_ink(INK_PLAIN);
    // El manifiesto: lo que el compilador declara que es (texto).
    if let Some((o, m)) = r.manifiesto {
        let fin = (o + m) as usize;
        if fin <= bytes.len() {
            s.with_ink(INK_ECHO);
            s.text(b"  -- el manifiesto (lo que el compilador dice que es) --\n");
            s.with_ink(INK_PLAIN);
            for linea in bytes[o as usize..fin].split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
                s.text(b"    ");
                s.text(linea);
                s.byte(b'\n');
            }
        }
    }
    s.with_ink(if r.admisible() { INK_GOOD } else { INK_ERR });
    s.text(if r.admisible() { b"  el kernel lo ADMITIRIA\n" as &[u8] } else { b"  el kernel NO lo admitiria\n" });
    s.with_ink(INK_PLAIN);
    drop(mapa);
    drop(fichero);
    After::Settle
}
