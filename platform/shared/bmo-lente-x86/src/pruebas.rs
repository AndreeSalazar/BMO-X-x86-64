//! La IDA Y VUELTA con el codificador de los emisores (`bmo_lower::x86`):
//! lo que escriben, la lente lo lee entero -- su largo exacto -- y lo dice
//! como lo diria el manual. Lo demas lo juzga el oraculo del `metro`, con
//! los programas corriendo.

extern crate std;

use super::*;
use bmo_lower::x86;
use std::string::String;
use std::vec::Vec;

fn bytes(f: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
    let mut v = Vec::new();
    f(&mut v);
    v
}

/// Lee `b` entero como UNA instruccion y da su texto en la direccion 0x1000.
fn una(b: &[u8]) -> String {
    let i = lee(b).unwrap_or_else(|e| panic!("{:02x?}: {:?}", b, e));
    assert_eq!(i.largo, b.len(), "{:02x?}: la lente mide {} y el emisor escribio {}", b, i.largo, b.len());
    String::from(i.texto(0x1000).as_str())
}

/// ** Cada codificador del emisor, con los dieciseis registros: el largo
/// que la lente lee es el que el emisor escribio. (Lo que se codifica con
/// un registro FIJO por esquema -- `rcx` en los desplazamientos por `cl`,
/// `rdx:rax` en la division -- va igual: el registro es el operando.)
#[test]
fn every_encoder_with_every_register_reads_whole() {
    for a in 0..16u8 {
        for b in 0..16u8 {
            for v in [
                bytes(|o| x86::mov_r64_r64(o, a, b)),
                bytes(|o| x86::xor_r64_r64(o, a, b)),
                bytes(|o| x86::or_r64_r64(o, a, b)),
                bytes(|o| x86::add_r64_r64(o, a, b)),
                bytes(|o| x86::sub_r64_r64(o, a, b)),
                bytes(|o| x86::test_r64_r64(o, a, b)),
                bytes(|o| x86::mov_r32_r32(o, a, b)),
                bytes(|o| x86::imul_r64_r64(o, a, b)),
                bytes(|o| x86::cmp_r64_r64(o, a, b)),
                bytes(|o| x86::xchg_r64_r64(o, a, b)),
                bytes(|o| x86::mov_r64_at_reg_disp32(o, a, b, -0x40)),
                bytes(|o| x86::mov_at_reg_disp32_from_r64(o, a, 0x1234, b)),
                bytes(|o| x86::mov_r64_at_reg(o, a, b)),
                bytes(|o| x86::mov_at_reg_from_r64(o, a, b)),
                bytes(|o| x86::mov_r32_at_reg(o, a, b)),
                bytes(|o| x86::mov_at_reg_from_r32(o, a, b)),
                bytes(|o| x86::movzx_r32_word_at_reg(o, a, b)),
                bytes(|o| x86::mov_word_at_reg_from_r16(o, a, b)),
                bytes(|o| x86::movzx_r32_byte_at_reg(o, a, b)),
                bytes(|o| x86::movzx_r32_byte_at_reg_disp(o, a, b, 7)),
                bytes(|o| x86::mov_byte_at_reg_from_low(o, a, b)),
                bytes(|o| x86::mov_byte_at_reg_disp_from_low(o, a, 9, b)),
                bytes(|o| x86::movsxd_r64_r32(o, a, b)),
                bytes(|o| x86::movsx_r64_r16(o, a, b)),
                bytes(|o| x86::movsx_r64_r8(o, a, b)),
                bytes(|o| x86::movzx_r64_low(o, a, b)),
                bytes(|o| x86::and_low_low(o, a, b)),
                bytes(|o| x86::or_low_low(o, a, b)),
                bytes(|o| x86::movq_xmm_de_r64(o, a, b)),
                bytes(|o| x86::movq_r64_de_xmm(o, a, b)),
            ] {
                let i = lee(&v).unwrap_or_else(|e| panic!("{:02x?}: {:?}", v, e));
                assert_eq!(i.largo, v.len(), "{:02x?} ({a}, {b})", v);
            }
            if b == 0 {
                for v in [
                    bytes(|o| x86::mov_r64_imm64(o, a, 0x1122_3344_5566_7788)),
                    bytes(|o| x86::mov_r32_imm32(o, a, 0x55)),
                    bytes(|o| x86::zero_r32(o, a)),
                    bytes(|o| x86::cmp_r64_imm8(o, a, -3)),
                    bytes(|o| x86::cmp_r64_imm32(o, a, 70000)),
                    bytes(|o| x86::dec_r64(o, a)),
                    bytes(|o| x86::inc_r64(o, a)),
                    bytes(|o| x86::neg_r64(o, a)),
                    bytes(|o| x86::shl_r64_imm8(o, a, 3)),
                    bytes(|o| x86::shr_r64_imm8(o, a, 1)),
                    bytes(|o| x86::shl_r64_cl(o, a)),
                    bytes(|o| x86::shr_r64_cl(o, a)),
                    bytes(|o| x86::sar_r64_cl(o, a)),
                    bytes(|o| x86::and_r64_imm32(o, a, 0xFF)),
                    bytes(|o| x86::mul_r64(o, a)),
                    bytes(|o| x86::idiv_r64(o, a)),
                    bytes(|o| x86::div_r64(o, a)),
                    bytes(|o| x86::add_r64_imm8(o, a, 8)),
                    bytes(|o| x86::sub_r64_imm8(o, a, 8)),
                    bytes(|o| x86::lea_r64_rsp_disp8(o, a, 16)),
                    bytes(|o| x86::mov_byte_at_reg_imm8(o, a, 0x41)),
                    bytes(|o| x86::setcc_low(o, 0x94, a)),
                    bytes(|o| x86::push_r64(o, a)),
                    bytes(|o| x86::pop_r64(o, a)),
                    bytes(|o| x86::cvtsi2sd_de_r64(o, a)),
                    bytes(|o| x86::cvttsd2si_r64(o, a)),
                ] {
                    let i = lee(&v).unwrap_or_else(|e| panic!("{:02x?}: {:?}", v, e));
                    assert_eq!(i.largo, v.len(), "{:02x?} ({a})", v);
                }
            }
        }
    }
    for f in [x86::cqo, x86::syscall, x86::addsd, x86::subsd, x86::mulsd, x86::divsd, x86::comisd] {
        let v = bytes(f);
        assert_eq!(lee(&v).unwrap().largo, v.len(), "{:02x?}", v);
    }
}

/// ** Y lo dice como el manual.
#[test]
fn says_it_like_the_manual() {
    let casos: Vec<(Vec<u8>, &str)> = std::vec![
        (bytes(|o| x86::mov_r64_r64(o, 0, 12)), "mov rax, r12"),
        (bytes(|o| x86::mov_r64_at_reg_disp32(o, 0, 5, -16)), "mov rax, qword [rbp-0x10]"),
        (bytes(|o| x86::mov_at_reg_disp32_from_r64(o, 5, 8, 3)), "mov qword [rbp+0x8], rbx"),
        (bytes(|o| x86::mov_r64_at_reg(o, 1, 4)), "mov rcx, qword [rsp]"),
        (bytes(|o| x86::mov_r64_at_reg(o, 1, 13)), "mov rcx, qword [r13]"),
        (bytes(|o| x86::mov_r64_imm64(o, 9, 0x10)), "mov r9, 0x10"),
        (bytes(|o| x86::zero_r32(o, 0)), "xor eax, eax"),
        (bytes(|o| x86::cmp_r64_imm8(o, 2, -1)), "cmp rdx, -0x1"),
        (bytes(|o| x86::shl_r64_imm8(o, 0, 6)), "shl rax, 0x6"),
        (bytes(|o| x86::sar_r64_cl(o, 3)), "sar rbx, cl"),
        (bytes(|o| x86::movzx_r32_byte_base_index(o, 0, 6, 1)), "movzx eax, byte [rsi+rcx*1]"),
        (bytes(|o| x86::movsx_r64_r8(o, 0, 6)), "movsx rax, sil"),
        (bytes(|o| x86::setcc_low(o, 0x94, 0)), "sete al"),
        (bytes(|o| x86::push_r64(o, 15)), "push r15"),
        (bytes(x86::cqo), "cqo"),
        (bytes(x86::syscall), "syscall"),
        (bytes(x86::addsd), "addsd xmm0, xmm1"),
        (bytes(|o| x86::cvtsi2sd_de_r64(o, 3)), "cvtsi2sd xmm0, rbx"),
        (bytes(|o| x86::movq_xmm_de_r64(o, 2, 7)), "movq xmm2, rdi"),
        (std::vec![0xC5, 0xFC, 0x58, 0xC2], "vaddps ymm0, ymm0, ymm2"),
        (std::vec![0xC4, 0xE2, 0x75, 0xB8, 0xC2], "vfmadd231ps ymm0, ymm1, ymm2"),
        (std::vec![0xC4, 0xE2, 0xF5, 0xB8, 0x02], "vfmadd231pd ymm0, ymm1, [rdx]"),
        (std::vec![0xC5, 0xFD, 0x58, 0xC1], "vaddpd ymm0, ymm0, ymm1"),
        (std::vec![0x0F, 0x01, 0xF9], "rdtscp"),
        (std::vec![0x0F, 0xC7, 0xF0], "rdrand eax"),
        (std::vec![0xF3, 0x0F, 0xB8, 0xC0], "popcnt eax, eax"),
        (std::vec![0xC5, 0xF8, 0x77], "vzeroupper"),
        (std::vec![0xF3, 0x90], "pause"),
        (std::vec![0xF3, 0x48, 0xAB], "rep stosq"),
        (std::vec![0x0F, 0xAE, 0xF0], "mfence"),
        (std::vec![0x48, 0x8D, 0x05, 0x10, 0, 0, 0], "lea rax, qword [rip+0x10]"),
    ];
    for (b, want) in casos {
        assert_eq!(una(&b), want, "{:02x?}", b);
    }
}

/// ** Los saltos: su destino, ya sumado; y que puede venir despues.
#[test]
fn jumps_say_where_they_go() {
    // jne rel8 -6, en 0x1000: cae en 0x1002 o salta a 0x0FFC
    let i = lee(&[0x75, 0xFA]).unwrap();
    assert_eq!(i.texto(0x1000).as_str(), "jne 0xffc");
    assert_eq!(i.sigue(0x1000), Sigue::Condicional { cae: 0x1002, salta: 0xFFC });
    // call rel32 +0x20
    let i = lee(&[0xE8, 0x20, 0, 0, 0]).unwrap();
    assert_eq!(i.sigue(0x1000), Sigue::Llama { destino: 0x1025, vuelve: 0x1005 });
    // jmp rel32, je rel32, ret, call rax, ud2
    assert_eq!(lee(&[0xE9, 0, 1, 0, 0]).unwrap().sigue(0), Sigue::Salta(0x105));
    assert_eq!(lee(&[0x0F, 0x84, 4, 0, 0, 0]).unwrap().sigue(0), Sigue::Condicional { cae: 6, salta: 10 });
    assert_eq!(lee(&[0xC3]).unwrap().sigue(0), Sigue::Vuelve);
    assert_eq!(lee(&[0xFF, 0xD0]).unwrap().sigue(0), Sigue::Indirecto);
    assert_eq!(lee(&[0x0F, 0x0B]).unwrap().sigue(0), Sigue::Para);
    assert_eq!(lee(&[0x90]).unwrap().sigue(7), Sigue::Siguiente(8));
}

/// ** Lo que NO sabe, lo dice -- y no inventa un largo.
#[test]
fn what_it_does_not_know_it_says() {
    // `prefetch` (0F 0D): no lo emite nadie, no esta en la tabla
    assert_eq!(lee(&[0x0F, 0x0D, 0x00]), Err(NoSe::FueraDeLaTabla { mapa: Mapa::Dos, op: 0x0D, ext: None, prefijos: 0, vex: false }));
    // `F6 /1` no existe en la tabla: dice la extension
    assert!(matches!(lee(&[0xF6, 0xC8, 1]), Err(NoSe::FueraDeLaTabla { op: 0xF6, ext: Some(1), .. })));
    // un `fs:` (64): fuera de la tabla, no un prefijo que se salte
    assert!(matches!(lee(&[0x64, 0x48, 0x8B, 0x04, 0x25, 0, 0, 0, 0]), Err(NoSe::FueraDeLaTabla { op: 0x64, .. })));
    // cortada a medias
    assert_eq!(lee(&[0x48, 0x8B]), Err(NoSe::Corta));
    assert_eq!(lee(&[0xE8, 1, 2]), Err(NoSe::Corta));
    // mas de 15 bytes de prefijos
    assert_eq!(lee(&[0x66; 20]), Err(NoSe::Larga));
    // `ldmxcsr` es solo memoria: con un registro no existe
    assert!(lee(&[0x0F, 0xAE, 0xD0]).is_err());
}

/// ** A6: LA COLUMNA `cpu` DE `intrinsics.toml` DICE LA VERDAD. Cada fila se
/// lee entera con la lente, y lo que piden sus instrucciones -- la columna
/// `pide` de la tabla -- es EXACTAMENTE lo que la fila declara: ni una
/// extension de menos (un `.bex` que no avisa y muere de `#UD`) ni una de mas
/// (un `.bex` que no arranca donde podria).
///
/// Las filas que la lente no sabe leer se nombran abajo, y son de x86-64 base
/// (los puertos, los registros de control, las tablas de descriptores): una
/// fila nueva que no lea, o que pida algo, rompe esta prueba hasta que se
/// mire.
#[test]
fn the_cpu_column_of_the_intrinsics_tells_the_truth() {
    use bmo_carga_juicio::instrucciones::{nombres, por_nombre};
    const SIN_LEER: &[&str] = &[
        "clflush", "cr0", "cr2", "cr3", "cr4", "inb", "inl", "invd", "invlpg", "inw", "lgdt", "lidt", "lldt", "ltr",
        "outb", "outl", "outw", "rdmsr", "set_cr0", "set_cr3", "set_cr4", "sgdt", "sidt", "swapgs", "wbinvd", "wrmsr",
    ];
    let tabla = bmo_sem_asm::Intrinsics::load_x86_64().unwrap();
    let mut sin_leer = Vec::new();
    let mut mal = Vec::new();
    for nombre in tabla.names() {
        let def = tabla.get(nombre).unwrap();
        let (mut at, mut pide, mut leida) = (0, 0u64, true);
        while at < def.bytes.len() {
            match lee(&def.bytes[at..]) {
                Ok(i) => {
                    if !i.fila.pide.is_empty() {
                        pide |= por_nombre(i.fila.pide).unwrap_or_else(|| panic!("la fila {} pide {:?}, que no esta en el catalogo", i.fila.nombre, i.fila.pide)).bit;
                    }
                    at += i.largo;
                }
                Err(_) => {
                    leida = false;
                    break;
                }
            }
        }
        if !leida {
            sin_leer.push(nombre);
            if def.cpu != 0 {
                mal.push(std::format!("{nombre}: declara {:?} y la lente no la sabe leer", nombres(def.cpu).collect::<Vec<_>>()));
            }
        } else if pide != def.cpu {
            mal.push(std::format!(
                "{nombre}: declara {:?} y sus bytes piden {:?}",
                nombres(def.cpu).collect::<Vec<_>>(),
                nombres(pide).collect::<Vec<_>>()
            ));
        }
    }
    assert!(mal.is_empty(), "\n  {}", mal.join("\n  "));
    assert_eq!(sin_leer, SIN_LEER, "las filas que la lente no lee cambiaron: mirar si piden algo");
}

// -------------------------------------------------------------------
//  B1: el recorrido desde la entrada
// -------------------------------------------------------------------

/// Recorre `c` con lo que se declara y unas semillas.
fn recorre_con(c: &[u8], entrada: usize, semillas: &[u64], declara: &[&str]) -> (recorre::Recorrido, Vec<u8>) {
    let mut visto = std::vec![0u8; c.len().div_ceil(8)];
    let mut pila = std::vec![0u32; 64];
    let r = recorre::recorre(
        c.len(),
        entrada,
        semillas.iter().copied(),
        |en, b| {
            let n = (c.len() - en).min(b.len());
            b[..n].copy_from_slice(&c[en..en + n]);
            n
        },
        |n| declara.contains(&n),
        &mut visto,
        &mut pila,
    );
    (r, visto)
}

fn leido(visto: &[u8], o: usize) -> bool {
    visto[o / 8] >> (o % 8) & 1 != 0
}

/// ** EL CASO DEL PLAN: un `popcnt` que el `.bex` no declara es un NO, con su
/// sitio; declarado, queda demostrado.
#[test]
fn walk_an_undeclared_extension_is_a_no() {
    // 0: mov eax, 7 / 5: popcnt eax, eax / 9: ret
    let c = [0xB8, 7, 0, 0, 0, 0xF3, 0x0F, 0xB8, 0xC0, 0xC3];
    let (r, _) = recorre_con(&c, 0, &[], &[]);
    assert_eq!(r.no_declarada, Some((5, "POPCNT")));
    assert!(!r.demostrado());
    let (r, _) = recorre_con(&c, 0, &[], &["POPCNT"]);
    assert!(r.demostrado(), "{r:?}");
    assert_eq!((r.leidas, r.bytes), (3, 10));
}

/// ** Los DATOS metidos en el codigo no se leen: un salto los rodea. Una
/// lectura lineal desde el byte 0 se habria tragado `0F 0D` (que la tabla
/// no tiene) y lo de detras, desalineado.
#[test]
fn walk_jumps_around_data_in_the_code() {
    // 0: jmp +3 / 2: [0F 0D FF] datos / 5: ret
    let c = [0xEB, 0x03, 0x0F, 0x0D, 0xFF, 0xC3];
    let (r, visto) = recorre_con(&c, 0, &[], &[]);
    assert!(r.demostrado(), "{r:?}");
    assert!(leido(&visto, 0) && leido(&visto, 5) && !leido(&visto, 2));
}

/// ** Las llamadas: se lee el destino Y lo de detras; `call rax` no dice
/// adonde va, pero vuelve; `jmp rax`, no. Lo que solo se alcanza por un
/// puntero lo trae la SEMILLA (una relocacion hacia el codigo); un `lea` de
/// `rip` que cae dentro NO siembra (puede ser un dato).
#[test]
fn walk_follows_calls_seeds_and_lea() {
    // 0: call +6 (a 11)        5: call rax       7: jmp rax
    // 9: ret (solo por semilla) 10: int3          11: lea rcx, [rip-0xb] (a 7) / 18: ret
    let c = [
        0xE8, 0x06, 0, 0, 0, //
        0xFF, 0xD0, //
        0xFF, 0xE0, //
        0xC3, //
        0xCC, //
        0x48, 0x8D, 0x0D, 0xF5, 0xFF, 0xFF, 0xFF, //
        0xC3,
    ];
    let (r, visto) = recorre_con(&c, 0, &[], &[]);
    assert!(r.demostrado(), "{r:?}");
    assert_eq!(r.indirectos, 2);
    assert!(leido(&visto, 11) && leido(&visto, 5) && leido(&visto, 7), "destino y detras del call indirecto");
    assert!(!leido(&visto, 9), "detras de `jmp rax` no se sabe");
    assert!(!leido(&visto, 10), "el `lea` apunta a 7, que ya se leyo por el camino; a nada mas");
    let (r, visto) = recorre_con(&c, 0, &[9], &[]);
    assert!(r.demostrado() && leido(&visto, 9), "la semilla lo trae: {r:?}");
}

/// ** Y lo que no puede demostrar, lo dice: lo que no sabe leer, un salto
/// que sale del codigo, una semilla fuera, y una pila que no da.
#[test]
fn walk_says_what_it_cannot_prove() {
    let (r, _) = recorre_con(&[0x0F, 0x0D, 0x00], 0, &[], &[]);
    assert!(matches!(r.no_sabe, Some((0, NoSe::FueraDeLaTabla { .. }))));
    let (r, _) = recorre_con(&[0xEB, 0x10], 0, &[], &[]);
    assert_eq!(r.fuera, Some((0, 0x12)));
    let (r, _) = recorre_con(&[0x90], 0, &[], &[]);
    assert_eq!(r.fuera, Some((0, 1)), "caer por el final tambien es salir");
    let (r, _) = recorre_con(&[0xC3], 0, &[40], &[]);
    assert_eq!(r.fuera, Some((0, 40)));
    for r in [r, recorre_con(&[0xC3; 4], 0, &[], &[]).0] {
        assert!(!r.desborda);
    }
    let mut visto = [0u8; 1];
    let r = recorre::recorre(100, 0, [], |_, _| 0, |_| true, &mut visto, &mut []);
    assert!(r.desborda && !r.demostrado(), "un mapa que no cubre el codigo no demuestra nada");
}

// -------------------------------------------------------------------
//  B2: la direccion de memoria, para la autopsia
// -------------------------------------------------------------------

/// ** El caso de Cyberpunk (02-10): `mov eax, [rdi+0x18c0]` con un `rdi` que
/// no era de nadie. La lente tiene que decir que la direccion salio de `rdi`.
/// Y los demas: SIB con indice, `r13` (que con REX.B parece `rbp`), `rsp`
/// como base (que obliga a SIB), `rip`, sin base, y un registro solo.
#[test]
fn says_which_register_held_the_address() {
    let m = |b: &[u8]| lee(b).unwrap().memoria();
    let d = m(&[0x8B, 0x87, 0xC0, 0x18, 0, 0]).unwrap();
    assert_eq!((d.base, d.indice, d.disp), (Some(7), None, 0x18C0));
    assert_eq!(nombre_r64(d.base.unwrap()), "rdi");
    // movzx eax, byte [rsi+rcx*1]
    let d = m(&bytes(|o| x86::movzx_r32_byte_base_index(o, 0, 6, 1))).unwrap();
    assert_eq!((d.base, d.indice, d.escala), (Some(6), Some(1), 1));
    // mov rcx, [r13]: mod 01 con disp 0, base 13
    let d = m(&bytes(|o| x86::mov_r64_at_reg(o, 1, 13))).unwrap();
    assert_eq!(d.base, Some(13));
    // mov rcx, [rsp]: SIB sin indice
    let d = m(&bytes(|o| x86::mov_r64_at_reg(o, 1, 4))).unwrap();
    assert_eq!((d.base, d.indice), (Some(4), None));
    // mov r9, [r12+r10*8+0x10]: REX.WRXB
    let d = m(&[0x4F, 0x8B, 0x4C, 0xD4, 0x10]).unwrap();
    assert_eq!((d.base, d.indice, d.escala, d.disp), (Some(12), Some(10), 8, 0x10));
    // lea rax, [rip+0x10]
    assert!(m(&[0x48, 0x8D, 0x05, 0x10, 0, 0, 0]).unwrap().rip_relativa);
    // mov eax, [rcx*4+0x100]: SIB sin base
    let d = m(&[0x8B, 0x04, 0x8D, 0, 1, 0, 0]).unwrap();
    assert_eq!((d.base, d.indice, d.escala, d.disp), (None, Some(1), 4, 0x100));
    // mov rax, r12: no toca memoria
    assert_eq!(m(&bytes(|o| x86::mov_r64_r64(o, 0, 12))), None);
}

/// ** Un salto a NULO: el `rip` no se lee, y la autopsia busca el `call` que
/// dejo el retorno en la pila. Lo de antes del retorno se lee desde el final.
#[test]
fn finds_the_call_before_a_return_address() {
    // ... mov rax, [rbx+8] / call qword [rax+0x18]  <- retorno
    let antes = [0x48, 0x8B, 0x43, 0x08, 0xFF, 0x50, 0x18];
    let (i, k) = el_call_antes(&antes).unwrap();
    assert_eq!(k, 3);
    assert_eq!(i.texto(0).as_str(), "call qword [rax+0x18]");
    assert_eq!(i.memoria().unwrap().base, Some(0));
    // call rel32: 5 bytes
    let antes = [0x90, 0xE8, 0x10, 0, 0, 0];
    assert_eq!(el_call_antes(&antes).map(|(_, k)| k), Some(5));
    // sin call al final: nada (no se inventa uno)
    assert!(el_call_antes(&[0x48, 0x89, 0xC8]).is_none());
}
