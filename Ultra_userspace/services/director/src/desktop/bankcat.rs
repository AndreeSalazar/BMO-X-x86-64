//! **BANK CAT, del lado del ESCRITORIO** (BC3 de `docs/plan/PLAN_BANK_CAT.md`,
//! 03-10).
//!
//! [consumo] NADA      solo corre mientras espera una respuesta del motor;
//!                     sin ordenes, no mira nada (L6h)
//!
//! La cara de BANK CAT (F5, `sys/bankcat.bex`) no puede lanzar el motor: un
//! proceso que lanza el escritorio nace SIN autoridad (`task/autoridad.rs`:
//! "la autoridad no viaja de padre a hijo"). Asi que el MOTOR es del
//! escritorio, como el de la calculadora, y la app le PIDE:
//!
//! ```text
//!    la app          0x1E bankcat pagar 19.99        (por su consola, `pide`)
//!    el escritorio   "3\n19.99\n"  ->  cobol/11/libro.bex   (bmo-bankcat)
//!    el motor        "0\n1240.03\n"                  (COBOL, x86-64)
//!    el escritorio   0x1E bank 0 124003              (a la consola de la app)
//! ```
//!
//! ** Un motor VIVO, no uno por pregunta: el libro esta en su memoria. Vive
//! lo que vive el escritorio; guardarlo en el disco es BC4. Al nacer, el
//! libro se abre con la BIENVENIDA (CAB de juego: circuito cerrado, D1).

use bmo_bankcat::{escribir, Centimos, Charla, Orden};
use bmo_userland as bmo;

use crate::desktop::Desktop;

/// El motor COBOL del libro (`toolchain/lang/cobol/examples/11-bankcat`).
pub(crate) const MOTOR: &[u8] = b"cobol/11/libro.bex";
/// Con lo que se abre un libro nuevo: 1.250,00 CAB de juego, como la maqueta.
pub(crate) const BIENVENIDA: Centimos = 125_000;

struct Banco {
    consola: Option<bmo::Consola>,
    charla: Charla,
    /// Respuestas que el motor todavia debe.
    esperando: u32,
}

static mut BANCO: Banco = Banco { consola: None, charla: Charla::nueva(), esperando: 0 };

fn banco() -> &'static mut Banco {
    // SAFETY: solo el hilo del escritorio, al atender y al drenar.
    unsafe { &mut *core::ptr::addr_of_mut!(BANCO) }
}

/// Un entero con signo en decimal, en `d`; devuelve cuantos bytes.
fn entero(v: i64, d: &mut [u8; 24]) -> usize {
    let mut t = [0u8; 24];
    let (mut i, mut u) = (t.len(), v.unsigned_abs());
    loop {
        i -= 1;
        t[i] = b'0' + (u % 10) as u8;
        u /= 10;
        if u == 0 {
            break;
        }
    }
    if v < 0 {
        i -= 1;
        t[i] = b'-';
    }
    let n = t.len() - i;
    d[..n].copy_from_slice(&t[i..]);
    n
}

fn mandar(c: &bmo::Consola, o: &Orden) {
    let mut b = [0u8; 48];
    if let Ok(n) = escribir(o, &mut b) {
        c.write(&b[..n]);
    }
}

/// **Una orden de la app**, o `None` para "hola" (que contesta el saldo).
pub(crate) fn pedir(o: Option<Orden>) -> Result<(), &'static [u8]> {
    let b = banco();
    let vivo = b.consola.as_ref().is_some_and(|c| c.has_child());
    if !vivo {
        let c = bmo::Consola::create().ok_or(&b"bankcat: no hay consola para el motor"[..])?;
        if bmo::ejecutar_en(MOTOR, c.cap).is_err() {
            return Err(b"bankcat: falta cobol/11/libro.bex (lo pone el build)");
        }
        // Un libro nuevo: se abre con la bienvenida, antes que nada.
        b.charla = Charla::nueva();
        b.esperando = 0;
        if !matches!(o, Some(Orden::Abrir(_))) {
            mandar(&c, &Orden::Abrir(BIENVENIDA));
            b.esperando += 1;
        }
        b.consola = Some(c);
    }
    let c = b.consola.as_ref().expect("recien puesta");
    mandar(c, &o.unwrap_or(Orden::Cuadrar));
    b.esperando += 1;
    Ok(())
}

/// **Lo que el motor contesto**, a la consola de la app. Se llama en cada
/// fotograma, despues de atender las peticiones; sin nada pendiente, vuelve.
pub(crate) fn drenar(dsk: &mut Desktop) {
    let b = banco();
    if b.esperando == 0 {
        return;
    }
    let Some(c) = b.consola.as_ref() else { return };
    let mut leido = false;
    for _ in 0..64 {
        let mut t = [0u8; 8];
        let n = c.read(&mut t);
        if n == 0 {
            break;
        }
        leido = true;
        for &x in &t[..n] {
            let Some(r) = b.charla.empujar(x) else { continue };
            b.esperando = b.esperando.saturating_sub(1);
            let mut linea = [0u8; 48];
            let mut k = 0;
            let mut poner = |s: &[u8]| {
                let m = s.len().min(linea.len() - k);
                linea[k..k + m].copy_from_slice(&s[..m]);
                k += m;
            };
            match r {
                Ok(r) => {
                    poner(b"\x1Ebank ");
                    poner(&[b'0' + r.estado as u8, b' ']);
                    let mut d = [0u8; 24];
                    let nd = entero(r.saldo, &mut d);
                    poner(&d[..nd]);
                }
                Err(_) => poner(b"\x1Ebank no el motor contesto algo que no se entiende"),
            }
            poner(b"\n");
            if let Some(cc) = dsk.out.console.as_ref() {
                cc.write(&linea[..k]);
            }
        }
    }
    // Un motor que se fue debiendo respuestas: se dice, y el siguiente
    // pedido lanza otro (con un libro nuevo: BC4 es guardarlo).
    if !leido && !c.has_child() {
        b.consola = None;
        b.esperando = 0;
        if let Some(cc) = dsk.out.console.as_ref() {
            cc.write(b"\x1Ebank no el motor se fue; el siguiente pedido abre un libro nuevo\n");
        }
    }
}
