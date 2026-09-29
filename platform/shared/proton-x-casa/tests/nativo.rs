//! **El juez de los sombreadores nativos** (P3b3b, 27-09): el interprete.
//!
//! `bmo_proton_x::nativo` traduce un programa a x86-64 con SSE; aqui se pone
//! en memoria ejecutable (como `MEM_OP_SELLAR`: escribir, y despues R+X sin W)
//! y se corre contra `Programa::correr`, BIT A BIT: los dos sombreadores del
//! cubo con las constantes de 360 fotogramas, y un programa hecho a mano con
//! cada operacion y los valores que muerden -- NaN, infinitos, -0, subnormales,
//! negativos en una raiz. Un NaN cuenta como igual a otro NaN (su carga util
//! puede ser otra: es NaN igual).

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use bmo_proton_x::dxil::programa::{Op, Programa};
use bmo_proton_x::{dxil, nativo};

const VS: &[u8] = include_bytes!("../../proton-x/prueba/cubo_vs.dxil");
const PS: &[u8] = include_bytes!("../../proton-x/prueba/cubo_ps.dxil");
/// Los SM5 de FXC del cubo de BMOX-12 (P3c3).
const SM5_VS: &[u8] = include_bytes!("../../proton-x/prueba/sombras/f3ef42a0.cso");
const SM5_PS: &[u8] = include_bytes!("../../proton-x/prueba/sombras/4d67f5e4.cso");

type Sombreador = extern "sysv64" fn(*mut f32, *const [f32; 4], *const u8, *mut [f32; 4]);

unsafe fn syscall6(n: u64, a: u64, b: u64, c: u64, d: u64, e: u64, f: u64) -> u64 {
    let r: u64;
    core::arch::asm!("syscall", inlateout("rax") n => r, in("rdi") a, in("rsi") b, in("rdx") c,
        in("r10") d, in("r8") e, in("r9") f, lateout("rcx") _, lateout("r11") _, options(nostack));
    r
}

/// Los bytes, en memoria nueva, y despues R+X (sin W).
fn sellar(codigo: &[u8]) -> Sombreador {
    let n = codigo.len().div_ceil(4096) as u64 * 4096;
    let base = unsafe { syscall6(9, 0, n, 3, 0x22, u64::MAX, 0) };
    assert!(base < (-4096i64) as u64);
    unsafe {
        core::ptr::copy_nonoverlapping(codigo.as_ptr(), base as *mut u8, codigo.len());
        assert_eq!(syscall6(10, base, n, 5, 0, 0, 0), 0);
        core::mem::transmute::<u64, Sombreador>(base)
    }
}

fn igual(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

/// Correr los dos y comparar las salidas.
fn comparar(p: &Programa, f: Sombreador, ent: &[[f32; 4]], cb: &[u8]) -> Result<(), String> {
    let (mut s1, mut s2) = (vec![[0.0f32; 4]; p.salidas], vec![[0.0f32; 4]; p.salidas]);
    let mut regs = Vec::new();
    p.correr(ent, cb, &mut s1, &mut regs);
    let mut r2 = p.iniciales.clone();
    f(r2.as_mut_ptr(), ent.as_ptr(), cb.as_ptr(), s2.as_mut_ptr());
    for (i, (a, b)) in s1.iter().zip(&s2).enumerate() {
        for k in 0..4 {
            if !igual(a[k], b[k]) {
                return Err(format!("salida {i}.{k}: interpretado {:#010x}, nativo {:#010x} (entradas {ent:?})", a[k].to_bits(), b[k].to_bits()));
            }
        }
    }
    Ok(())
}

fn cb_de(f: u32) -> Vec<u8> {
    let c = bmo_cubo::constantes(bmo_cubo::angulo_de_fotograma(f), 1280.0 / 720.0);
    c.wvp.iter().chain(&c.world).chain(&c.luz).flat_map(|x| x.to_le_bytes()).collect()
}

/// *** Los dos sombreadores de dxc, NATIVOS, dan BIT A BIT lo que el
/// interprete (que da las huellas de la 3060): 24 vertices y sus pixeles, en
/// los 360 fotogramas.
#[test]
fn los_sombreadores_del_cubo_nativos_dan_los_bits_del_interprete() {
    cubo_nativo(VS, PS);
}

/// Lo mismo con los SM5 de FXC (P3c3): el traductor a x86-64 no sabe de
/// donde vino el programa.
#[test]
fn los_sm5_de_fxc_nativos_dan_los_bits_del_interprete() {
    cubo_nativo(SM5_VS, SM5_PS);
}

fn cubo_nativo(vs: &[u8], ps: &[u8]) {
    let (vs, ps) = (dxil::programa::compilar(&dxil::leer(vs).unwrap()).unwrap(), dxil::programa::compilar(&dxil::leer(ps).unwrap()).unwrap());
    let (fv, fp) = (sellar(&nativo::compilar(&vs).unwrap()), sellar(&nativo::compilar(&ps).unwrap()));
    for f in 0..360 {
        let cb = cb_de(f);
        for v in bmo_cubo::vertices() {
            let e = [[v.pos[0], v.pos[1], v.pos[2], 1.0], [v.normal[0], v.normal[1], v.normal[2], 0.0], v.color];
            comparar(&vs, fv, &e, &cb).unwrap();
            let c = bmo_cubo::constantes(bmo_cubo::angulo_de_fotograma(f), 1280.0 / 720.0);
            let n = bmo_cubo::mat::transformar_dir(&c.world, v.normal);
            comparar(&ps, fp, &[[0.0; 4], [n[0], n[1], n[2], 0.0], v.color], &cb).unwrap();
        }
    }
}

/// Cada operacion, con los valores que muerden.
#[test]
fn cada_operacion_nativa_da_los_bits_del_interprete_con_nan_infinitos_y_ceros() {
    // r0, r1: las dos entradas; r2..: los resultados, cada uno a una salida.
    let ops = vec![
        Op::Entrada { d: 0, elemento: 0, componente: 0 },
        Op::Entrada { d: 1, elemento: 0, componente: 1 },
        Op::Mul { d: 2, a: 0, b: 1 },
        Op::Add { d: 3, a: 0, b: 1 },
        Op::Sub { d: 4, a: 0, b: 1 },
        Op::Div { d: 5, a: 0, b: 1 },
        Op::Mad { d: 6, a: 0, b: 1, c: 0 },
        Op::Dot { d: 7, n: 2, a: [0, 1, 0, 0], b: [1, 0, 0, 0] },
        Op::Rsqrt { d: 8, a: 0 },
        Op::Sqrt { d: 9, a: 1 },
        Op::Saturate { d: 10, a: 0 },
        Op::Abs { d: 11, a: 1 },
        Op::Min { d: 12, a: 0, b: 1 },
        Op::Max { d: 13, a: 0, b: 1 },
        Op::Constantes { d: 14, fila: 1 },
    ];
    let mut ops2 = ops.clone();
    for (k, r) in (2..18u16).enumerate() {
        ops2.push(Op::Salida { s: r, elemento: (k / 4) as u8, componente: (k % 4) as u8 });
    }
    let p = Programa { ops: ops2, iniciales: vec![0.0; 18], entradas: 1, salidas: 4, lee: 1, filas_cb: 2 };
    let f = sellar(&nativo::compilar(&p).unwrap());
    let raros = [
        0.0f32, -0.0, 1.0, -1.0, 0.5, 1.5, 3.0, -2.5, 1e-40, -1e-40, f32::MIN_POSITIVE, f32::MAX, f32::MIN, f32::INFINITY,
        f32::NEG_INFINITY, f32::NAN, -f32::NAN, 0.1, 1.0 / 3.0, 16777217.0, 1e30, 1e-30,
    ];
    let cb: Vec<u8> = (0..32u32).flat_map(|i| (i as f32 * 0.25 - 1.0).to_le_bytes()).collect();
    let mut malos = Vec::new();
    for &a in &raros {
        for &b in &raros {
            if let Err(m) = comparar(&p, f, &[[a, b, 0.0, 0.0]], &cb) {
                malos.push(m);
            }
        }
    }
    // Y unos miles de valores cualesquiera (un generador fijo: siempre los mismos).
    let mut x: u64 = 0x2545_F491_4F6C_DD1D;
    for _ in 0..20000 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let (a, b) = (f32::from_bits(x as u32), f32::from_bits((x >> 32) as u32));
        if let Err(m) = comparar(&p, f, &[[a, b, 0.0, 0.0]], &cb) {
            malos.push(m);
        }
    }
    assert!(malos.is_empty(), "{} distintos; el primero: {}", malos.len(), malos[0]);
}

/// El MXCSR de quien llama no cambia las cuentas (se pone el de D3D) y vuelve
/// como estaba.
#[test]
fn el_mxcsr_de_quien_llama_no_cuenta_y_se_devuelve() {
    let p = Programa {
        ops: vec![Op::Entrada { d: 0, elemento: 0, componente: 0 }, Op::Entrada { d: 1, elemento: 0, componente: 1 }, Op::Div { d: 2, a: 0, b: 1 }, Op::Salida { s: 2, elemento: 0, componente: 0 }],
        iniciales: vec![0.0; 3],
        entradas: 1,
        salidas: 1,
        lee: 1,
        filas_cb: 0,
    };
    let f = sellar(&nativo::compilar(&p).unwrap());
    let mut regs = p.iniciales.clone();
    let mut s = [[0.0f32; 4]];
    let hacia_cero: u32 = 0x7F80;
    let (mut antes, mut despues) = (0u32, 0u32);
    unsafe {
        core::arch::asm!("stmxcsr [{}]", in(reg) &mut antes);
        core::arch::asm!("ldmxcsr [{}]", in(reg) &hacia_cero);
    }
    f(regs.as_mut_ptr(), [[1.0f32, 3.0, 0.0, 0.0]].as_ptr(), [0u8; 16].as_ptr(), s.as_mut_ptr());
    unsafe {
        core::arch::asm!("stmxcsr [{}]", in(reg) &mut despues);
        core::arch::asm!("ldmxcsr [{}]", in(reg) &antes);
    }
    assert_eq!(despues, hacia_cero, "el MXCSR de quien llama, devuelto");
    assert_eq!(s[0][0].to_bits(), (1.0f32 / 3.0).to_bits(), "1/3 al MAS CERCANO, no hacia cero");
}
