//! **EL TABLERO DE LA CAJA NEGRA** (07-10): una casilla por nucleo que dice
//! QUE estaba haciendo, en RAM que sobrevive al reinicio.
//!
//! [carril]  AMARILLO  es un instrumento: lo que diga manda a buscar
//! [consumo] NADA      cuatro escrituras cuando alguien marca una etapa
//! [cuesta]  NADA      sin cerrojo y sin disco: una casilla mal escrita es un
//!                     dato raro en el CAIDA, no un fallo
//!
//! El metal (07-10): `smp all` y Cyberpunk, y el PC se reinicia de golpe,
//! sin pantalla azul. El anillo de texto de la caja negra (`caida.rs`) guarda
//! lo que la maquina DIJO; un nucleo que muere en Ring 3 o a mitad de una
//! entrada no dice nada. El tablero no espera a que nadie hable: cada nucleo
//! deja su etapa en SU casilla al pasar por los sitios que importan (repartir,
//! entrar en Ring 3, volver, la alarma), con la hora del TSC. Al arrancar
//! otra vez, `caida::abrir` lo lee ANTES de limpiarlo y lo pone al final de
//! lo recuperado: el `CAIDA.TXT` siguiente termina con una linea por nucleo
//! y cuanto antes del final paso por ahi.
//!
//! ```text
//!    +0     magia   u64
//!    +8     hz      u64   el TSC por segundo (lo pone quien sabe: 0 = no)
//!    +16    casillas de 32 B: [etapa, a, b, tsc]
//! ```
//!
//! Vive justo detras del anillo (`caida::BASE + caida::BYTES`), en la misma
//! reserva.

/// Lo que mide.
pub const BYTES: u64 = 4096;
const MAGIA: u64 = 0x5442_4C45_524F_0001;
const CAB: u64 = 16;
/// Cuantas casillas caben.
pub const CASILLAS: usize = ((BYTES - CAB) / 32) as usize;

/// Las casillas con propietario: el sub-director del BSP, la alarma, y un obrero
/// por casilla desde [`OBREROS`].
pub const SUB_BSP: usize = 0;
pub const ALARMA: usize = 1;
pub const OBREROS: usize = 2;

/// Las etapas (el numero que se guarda) y como se dicen.
pub const REPARTE: u64 = 1;
pub const ACABO: u64 = 2;
pub const SALTA: u64 = 3;
pub const ENTRA_RING3: u64 = 10;
pub const VUELVE: u64 = 11;

fn nombre(etapa: u64) -> &'static str {
    match etapa {
        REPARTE => "el BSP REPARTE una faena (a = faena, b = partes)",
        ACABO => "el BSP CERRO la faena (a = faena, b = mascara de malas)",
        SALTA => "la ALARMA mando a la puerta (a = rip, b = tid)",
        ENTRA_RING3 => "ENTRA en Ring 3 (a = faena, b = parte)",
        VUELVE => "VOLVIO de Ring 3 (a = faena, b = vector<<48 + rip)",
        _ => "etapa desconocida",
    }
}

static mut VIRT: u64 = 0;

fn ptr(off: u64) -> *mut u64 {
    // SAFETY de quien llama: `VIRT` no es cero.
    unsafe { (VIRT + off) as *mut u64 }
}

fn tsc() -> u64 {
    // SAFETY: RDTSC no tiene efectos.
    unsafe { core::arch::x86_64::_rdtsc() }
}

/// **Marcar una etapa** en la casilla `c`. Desde cualquier nucleo, sin
/// cerrojo (cada casilla tiene un solo escritor); la etapa se escribe la
/// ULTIMA: una casilla a medias dice la etapa de antes.
#[inline]
pub fn marcar(c: usize, etapa: u64, a: u64, b: u64) {
    // SAFETY: VIRT es el physmap de la reserva (`abrir`), y `c` cae dentro.
    unsafe {
        if VIRT == 0 || c >= CASILLAS {
            return;
        }
        let o = CAB + c as u64 * 32;
        core::ptr::write_volatile(ptr(o + 8), a);
        core::ptr::write_volatile(ptr(o + 16), b);
        core::ptr::write_volatile(ptr(o + 24), tsc());
        core::ptr::write_volatile(ptr(o), etapa);
        // A la RAM ya: un reinicio de golpe borra la cache sin escribirla
        // (`caida::a_la_ram`).
        super::caida::a_la_ram_linea(ptr(o) as *const u8);
    }
}

/// El TSC por segundo, para que el CAIDA diga milisegundos.
pub fn poner_hz(hz: u64) {
    // SAFETY: como `marcar`.
    unsafe {
        if VIRT != 0 && core::ptr::read_volatile(ptr(8)) != hz {
            core::ptr::write_volatile(ptr(8), hz);
            super::caida::a_la_ram_linea(ptr(0) as *const u8);
        }
    }
}

/// **Abrir**: si hay un tablero de la sesion anterior, cada casilla con algo,
/// como una linea de texto a `decir`; despues, limpio para esta sesion.
pub(super) fn abrir(virt: u64, decir: &mut dyn FnMut(&str)) {
    // SAFETY: `virt` es el physmap de una reserva de RAM de `BYTES` (lo
    // comprobo `mm::phys`).
    unsafe {
        VIRT = virt;
        if core::ptr::read_volatile(ptr(0)) == MAGIA {
            let hz = core::ptr::read_volatile(ptr(8));
            let mut fin = 0u64;
            for c in 0..CASILLAS {
                let o = CAB + c as u64 * 32;
                if core::ptr::read_volatile(ptr(o)) != 0 {
                    fin = fin.max(core::ptr::read_volatile(ptr(o + 24)));
                }
            }
            for c in 0..CASILLAS {
                let o = CAB + c as u64 * 32;
                let etapa = core::ptr::read_volatile(ptr(o));
                if etapa == 0 {
                    continue;
                }
                let (a, b, t) = (core::ptr::read_volatile(ptr(o + 8)), core::ptr::read_volatile(ptr(o + 16)), core::ptr::read_volatile(ptr(o + 24)));
                let mut l = super::format::Buf::new();
                l.txt("[tablero] ");
                match c {
                    SUB_BSP => l.txt("BSP sub-dir: "),
                    ALARMA => l.txt("BSP alarma: "),
                    _ => {
                        l.txt("obrero ");
                        l.dec((c - OBREROS) as u64);
                        l.txt(": ");
                    }
                }
                l.txt_max(nombre(etapa), 52);
                decir(l.as_str());
                let mut l = super::format::Buf::new();
                l.txt("[tablero]    a=0x");
                l.hex(a, 16);
                l.txt(" b=0x");
                l.hex(b, 16);
                l.txt(" antes del final: ");
                match hz {
                    0 => {
                        l.dec((fin - t) / 1000);
                        l.txt(" kciclos");
                    }
                    hz => {
                        l.dec((fin - t) as u128 as u64 / (hz / 1000).max(1));
                        l.txt(" ms");
                    }
                }
                decir(l.as_str());
            }
        }
        core::ptr::write_bytes(VIRT as *mut u8, 0, BYTES as usize);
        core::ptr::write_volatile(ptr(0), MAGIA);
        let mut o = 0;
        while o < BYTES {
            super::caida::a_la_ram_linea((VIRT + o) as *const u8);
            o += 64;
        }
    }
}
