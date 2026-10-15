//! **`dentro JUEGO.EXE`: un programa de WINDOWS por dentro** (LX1 de
//! `PLAN_PROTON_X.md`, 15-10).
//!
//! > El propietario: *"quizas nos sorprenda que PROTON-X no deja claro aunque
//! > printf ayude [...] el binario PRECISO en 0 y 1 diga algo"*.
//!
//! [consumo] NADA      corre cuando alguien lo pide (la orden o el clic derecho)
//!
//! Lo que PROTON-X tiene que saber de un `.exe` antes de correrlo, leido del
//! fichero por `bmo_radiografia::pe` (el mismo veredicto que su cargador):
//! si lo correria, sus secciones, lo que PIDE a Windows (cada DLL y cada
//! funcion: lo que PROTON-X tiene que dar), la lente sobre su codigo -- desde
//! la entrada y desde cada funcion de `.pdata` -- con las extensiones de la
//! CPU que usa de verdad, y las primeras instrucciones en castellano, con
//! NOMBRE cuando llaman a Windows.

use bmo_radiografia::{lente, pe};

use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};

/// Funciones que se nombran por DLL (las demas se cuentan).
const POR_DLL: usize = 6;

pub(crate) fn exe(s: &mut Output, ruta: &[u8], bytes: &[u8], visto: &mut [u8], pila: &mut [u32]) {
    s.with_ink(INK_ECHO);
    s.text(b"  == POR DENTRO de un programa de WINDOWS: ");
    s.text(ruta);
    s.text(b"  (");
    s.dec(bytes.len() as u64);
    s.text(b" bytes, leido como lo leeria PROTON-X)\n");
    s.with_ink(INK_PLAIN);
    let p = match pe::leer(bytes) {
        Ok(p) => p,
        Err(no) => {
            s.with_ink(INK_ERR);
            s.text(b"  PROTON-X NO lo correria: ");
            s.text(no.nombre().as_bytes());
            s.byte(b'\n');
            s.with_ink(INK_PLAIN);
            return;
        }
    };
    s.with_ink(INK_GOOD);
    s.text(b"  PROTON-X lo correria: x86-64 (PE32+), sin .NET, nada que escriba y ejecute\n");
    s.with_ink(INK_PLAIN);
    s.text(if p.es_dll { b"  es una DLL" as &[u8] } else { b"  forma     " });
    s.text(b" base 0x");
    s.hex(p.base, 8);
    s.text(b"   entrada +0x");
    s.hex(p.entrada as u64, 4);
    s.text(b"   imagen ");
    s.dec(p.tam_imagen as u64);
    s.text(b" B\n  secciones ");
    for sec in p.secciones() {
        s.text(sec.nombre());
        s.text(if sec.ejecuta() {
            b" (codigo) " as &[u8]
        } else if sec.escribe() {
            b" (datos) "
        } else {
            b" (lectura) "
        });
    }
    s.byte(b'\n');
    pide(s, &p, bytes);
    // La lente: desde la entrada y desde cada funcion de `.pdata`.
    let mut usadas = pe::Extensiones::default();
    match (p.codigo().copied(), p.lente(bytes, visto, pila, &mut usadas)) {
        (Some(c), Some(l)) => {
            s.text(b"  lente     ");
            s.dec(l.leidas as u64);
            s.text(b" instrucciones, ");
            s.dec(l.bytes as u64);
            s.text(b" de ");
            s.dec(c.tam_fichero as u64);
            s.text(b" bytes de codigo (desde la entrada y desde cada funcion de .pdata); ");
            s.dec(l.indirectos as u64);
            s.text(b" saltos indirectos\n            usa de la CPU: ");
            if usadas.is_empty() {
                s.text(b"solo x86-64 base");
            }
            for e in usadas.iter() {
                s.text(e.as_bytes());
                s.byte(b' ');
            }
            s.byte(b'\n');
            if l.no_sabe.is_some() || l.fuera.is_some() || l.desborda {
                s.with_ink(INK_ECHO);
                s.text(b"            sin leer entero: ");
                if l.no_sabe.is_some() {
                    s.text(b"algo que la lente no sabe leer; ");
                }
                if l.fuera.is_some() {
                    s.text(b"un salto que se sale de .text; ");
                }
                if l.desborda {
                    s.text(b"el mapa se lleno");
                }
                s.byte(b'\n');
                s.with_ink(INK_PLAIN);
            }
            primeras(s, &p, bytes, &c);
        }
        _ => s.text(b"  lente     no se paso (sin seccion de codigo en la entrada, o pasa de 8 MiB)\n"),
    }
}

/// **Lo que pide a Windows**: DLL a DLL, sus primeras funciones y cuantas.
fn pide(s: &mut Output, p: &pe::Pe, bytes: &[u8]) {
    s.with_ink(INK_ECHO);
    s.text(b"  -- lo que PIDE a Windows (lo que PROTON-X tiene que dar) --\n");
    s.with_ink(INK_PLAIN);
    let mut dll: &[u8] = &[];
    let (mut en_esta, mut dlls) = (0usize, 0usize);
    let cierra = |s: &mut Output, en_esta: usize| {
        if en_esta > POR_DLL {
            s.text(b" (+");
            s.dec((en_esta - POR_DLL) as u64);
            s.text(b" mas)");
        }
        if en_esta > 0 {
            s.byte(b'\n');
        }
    };
    let r = p.importaciones(bytes, |i| {
        if i.dll != dll {
            cierra(s, en_esta);
            dll = i.dll;
            en_esta = 0;
            dlls += 1;
            s.text(b"    ");
            s.text(i.dll);
            s.text(b":");
        }
        en_esta += 1;
        if en_esta <= POR_DLL {
            s.byte(b' ');
            match i.nombre {
                Some(n) => s.text(n),
                None => {
                    s.text(b"#");
                    s.dec(i.ordinal as u64);
                }
            }
        }
    });
    cierra(s, en_esta);
    match r {
        Ok(n) => {
            s.text(b"    en total ");
            s.dec(n as u64);
            s.text(b" funciones de ");
            s.dec(dlls as u64);
            s.text(b" DLL\n");
        }
        Err(no) => {
            s.with_ink(INK_ERR);
            s.text(b"    la tabla de importaciones no se lee entera: ");
            s.text(no.nombre().as_bytes());
            s.byte(b'\n');
            s.with_ink(INK_PLAIN);
        }
    }
}

/// **Las primeras instrucciones desde la entrada, en castellano.** Las que
/// llaman a Windows por la IAT (`call [rip+x]`), con su nombre.
fn primeras(s: &mut Output, p: &pe::Pe, bytes: &[u8], c: &pe::SeccionPe) {
    let code = c.crudos(bytes);
    s.with_ink(INK_ECHO);
    s.text(b"  -- las primeras instrucciones desde la entrada, en castellano --\n");
    s.with_ink(INK_PLAIN);
    let mut o = (p.entrada - c.rva) as usize;
    for _ in 0..16 {
        if o >= code.len() {
            break;
        }
        let Ok(i) = lente::lee(&code[o..]) else {
            s.text(b"    (la lente no sabe leer lo que sigue)\n");
            break;
        };
        let rva = c.rva as u64 + o as u64;
        let en = p.base + rva;
        s.text(b"    0x");
        s.hex(en, 8);
        s.text(b"  ");
        s.text(i.texto(en).as_str().as_bytes());
        s.byte(b'\n');
        s.with_ink(INK_ECHO);
        s.text(b"               ");
        // A quien llama: un `call`/`jmp [rip+x]` que cae en una ranura de la
        // IAT es una funcion de Windows, y se dice cual.
        let windows = i
            .memoria()
            .filter(|m| m.rip_relativa && (i.fila.nombre == "call" || i.fila.nombre == "jmp"))
            .and_then(|m| p.importada_en(bytes, (rva + i.largo as u64).wrapping_add(m.disp as u64) as u32));
        match windows {
            Some(w) => {
                s.text(if i.fila.nombre == "call" { b"llama a " as &[u8] } else { b"salta a " });
                s.text(w.dll);
                s.byte(b'!');
                match w.nombre {
                    Some(n) => s.text(n),
                    None => {
                        s.text(b"#");
                        s.dec(w.ordinal as u64);
                    }
                }
                s.text(b" (de Windows: en BMO-X la da PROTON-X)");
            }
            None => s.text(lente::castellano::explica(&i, en).as_str().as_bytes()),
        }
        s.byte(b'\n');
        s.with_ink(INK_PLAIN);
        // Un `ret` o un salto sin vuelta acaba lo que se lee en linea.
        if i.fila.nombre == "ret" || i.fila.nombre == "jmp" || i.fila.nombre == "ud2" {
            break;
        }
        o += i.largo;
    }
}
