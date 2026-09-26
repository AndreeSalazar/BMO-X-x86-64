//! **VERRANO V1b: EL ANILLO** -- el banco ORQUESTADO. La CPU deja el
//! fotograma N+1 en su ranura mientras la 3060 dibuja el N, y no espera a
//! ninguno: solo espera, si acaso, a que una ranura quede libre.
//!
//! capa: puro -- las ranuras, las ordenes de cada una y lo que se escribe
//! por fotograma; la RAM, el prestamo y los registros los toca el kernel (L8)
//!
//! [eje]     RENDIMIENTO -- sin tocar los programas del BSF (los mismos
//!           bytes, el mismo juez) ni el estado de V1 `ligero`: las ordenes
//!           de una ranura son las de `ligero` con UN semaforo mas
//!
//! # Por que (metal 26-09 07:54, `gpu verrano banco ligero`)
//!
//! ```text
//!    1726 fps de pared = ~580 us por fotograma, y D3D12 ~3780 (~265 us)
//!    preparar  168 us  en caliente: ~300 palabras por la ventana PRAMIN
//!                      (los vertices, los semaforos, la entrada), ~0,5 us
//!                      cada una -- escribir en la VRAM por BAR0 es LENTO
//!    la 3060   281 us  del timbre al semaforo, con la invalidacion de la
//!                      MMU de `lanzar` y el sondeo por la ventana
//!    y la CPU  esperando CADA fotograma: nada se solapa
//! ```
//!
//! # Lo que cambia
//!
//! ```text
//!    los vertices   en RAM del PC: UNA pagina (IOVA 0x3A20_0000, tras el
//!                   MiB del fractal), prestada a la 3060 SOLO LECTURA y
//!                   vista en la VA 0x2_0013_0000 (la entrada 304 de la PT
//!                   del tramo, tras las del fractal). La CPU escribe a
//!                   velocidad de RAM, no de PCIe
//!    las ranuras    RANURAS = 4. La ranura k: sus vertices (1 KiB de esa
//!                   pagina) y sus ordenes (1 KiB de la pagina EMPUJE)
//!    la tabla       la direccion de los vertices de la ranura la escribe
//!                   la PROPIA 3060 en TABLA (un semaforo con esa direccion
//!                   y WAIT_FOR_IDLE, antes del dibujo): el programa de
//!                   vertice del BSF la lee como en V0, y no cambia un bit
//!    la valla       UN semaforo (SEMAFOROS + 0x170) con el NUMERO del
//!                   fotograma. La ranura k se reusa cuando la 3060 ya pago
//!                   el fotograma de hace RANURAS
//!    por fotograma  los vertices (RAM); la COLA de sus ordenes (el dibujo,
//!                   su espera y la valla: 14 palabras), la entrada del
//!                   GPFIFO y GP_PUT, TODO en una sola apertura de la
//!                   ventana y sin releer; y el timbre. Sin invalidar la
//!                   MMU: el mapa no cambia (se invalida al ARMAR)
//! ```
//!
//! # Armar
//!
//! El primer fotograma (y cualquiera que no pueda ir en el anillo) va EN
//! FRIO: los programas, la tabla, las CUATRO ranuras de ordenes enteras y
//! todo releido, y el kernel ESPERA a que se pague. Solo entonces el anillo
//! queda armado. Quien decide si se puede seguir en el anillo es el kernel
//! (`gpu_trabajo/cubo.rs`): los mismos programas y la misma ventana, y nadie
//! mas por el GR desde el ultimo fotograma.

use crate::canal::GR;
use crate::copia::{entrada, escribir, leer32, GP_PUT, TIMBRE};
use crate::cubo::{self as cu, Ventana, CABEN};
use crate::lienzo::sombreador_va;
use crate::raster::{ESCALONES, N_ESCALONES, SUSTITUTO};
use crate::sombreador::{EMPUJE, PROGRAMA, SALIDA, SEMAFOROS};
use crate::tresde as td;
use crate::tuberia::{escribir_bytes, Paquete, BYTES_VERTICE, MAX_VERTICES, PS, TABLA, VS};
use crate::vram::{a_cero, escribir64, leer64, ventana, TRAMO_VA, VENTANA, VENTANA_REG};
use crate::Registros;

/// Cuantos fotogramas puede haber en vuelo a la vez.
pub const RANURAS: u32 = 4;
/// Lo que mide la ranura de vertices en la pagina de RAM.
pub const PASO: u64 = 1024;
pub const PAGINAS: u64 = 1;
/// Donde la ve la IOMMU: tras el MiB del fractal (y antes del booter).
pub const IOVA: u64 = 0x3A20_0000;
/// La entrada de la PT del tramo: tras las 256 del fractal (48..303).
pub const PT_PRIMERA: usize = 304;
/// Donde la ve la 3060.
pub const VA: u64 = TRAMO_VA + PT_PRIMERA as u64 * 4096;
/// La valla: el numero del ultimo fotograma pagado.
pub const SEMAFORO: u64 = SEMAFOROS + 0x170;
/// Lo que caben las ordenes de una ranura (un cuarto de EMPUJE).
pub const PALABRAS_RANURA: usize = 256;

/// La ranura del fotograma `numero`.
pub const fn ranura(numero: u32) -> u32 {
    numero % RANURAS
}

/// Donde lee la 3060 los vertices de la ranura `k`.
pub const fn vertices_va(k: u32) -> u64 {
    VA + PASO * k as u64
}

/// Donde van (en la VRAM) las ordenes de la ranura `k`.
pub const fn empuje(k: u32) -> u64 {
    EMPUJE + 4 * PALABRAS_RANURA as u64 * k as u64
}

/// Donde va la PTE de la pagina del anillo.
pub const fn pte_en() -> u64 {
    crate::vram::TABLAS[3] + 8 * PT_PRIMERA as u64
}

/// **Ya se pago el fotograma `numero`**, si la valla dice `pagado` (los
/// numeros dan la vuelta: se compara la distancia, no el valor).
pub const fn ya(pagado: u32, numero: u32) -> bool {
    pagado.wrapping_sub(numero) as i32 >= 0
}

/// **Las ordenes** del fotograma `numero`, con `n` triangulos, en la ranura
/// `k`: las de V1 `ligero` hasta el dibujo; la direccion de los vertices de
/// la ranura en TABLA (un semaforo) y su espera; el dibujo, su espera, y la
/// valla con el numero. Y DESDE QUE PALABRA cambian de un fotograma a otro
/// de la misma ranura (la cola): lo de antes solo depende de la ventana y
/// de `k`.
pub fn ordenes(v: &Ventana, k: u32, n: usize, numero: u32) -> Option<(cu::Ordenes, usize)> {
    if n == 0 || n > CABEN || k >= RANURAS {
        return None;
    }
    let mut e = cu::hasta_el_dibujo_con(v, false);
    e.semaforo(TABLA, vertices_va(k) as u32);
    e.m(td::WAIT_FOR_IDLE, &[0]);
    let cola = e.n;
    e.dibujo_de(3 * n as u32);
    e.m(td::WAIT_FOR_IDLE, &[0]);
    e.semaforo(SEMAFORO, numero);
    (e.n <= PALABRAS_RANURA).then_some((e, cola))
}

/// **Mapear la pagina del anillo**: su PTE de SISTEMA (solo si estaba vacia
/// o ya era esta), releida.
pub fn mapear<R: Registros>(r: &mut R) -> Option<(u32, u32)> {
    let v = crate::mmu::pte_sistema(IOVA);
    let hay = leer64(r, pte_en());
    if hay != 0 && hay != v {
        return None;
    }
    escribir64(r, pte_en(), v);
    Some((1, (leer64(r, pte_en()) == v) as u32))
}

/// **Armar** (en frio, todo releido): la valla y los escalones a cero, los
/// DOS programas tal como llegan, la tabla, las ordenes ENTERAS de las
/// cuatro ranuras y la entrada `e` del fotograma `numero` (el primero). Sus
/// vertices ya estan en su ranura de RAM (los pone el kernel). Lo lanza el
/// kernel con `cubo::lanzar`, que SI invalida la MMU.
pub fn armar<R: Registros>(r: &mut R, e: u32, v: &Ventana, p: &Paquete, numero: u32) -> bool {
    let n = p.vertices.len() / BYTES_VERTICE;
    if !crate::blur::entrada_valida(e) || n == 0 || n % 3 != 0 || n > MAX_VERTICES {
        return false;
    }
    let va = vertices_va(ranura(numero));
    let mut bien = escribir(r, SEMAFORO, &[0]) == 1
        && escribir(r, ESCALONES, &[0; N_ESCALONES as usize]) == N_ESCALONES as usize
        && a_cero(r, SALIDA) as usize == crate::vram::PALABRAS
        && a_cero(r, PROGRAMA) as usize == crate::vram::PALABRAS
        && a_cero(r, SUSTITUTO) as usize == crate::vram::PALABRAS
        && escribir_bytes(r, VS, p.vs)
        && escribir_bytes(r, PS, p.ps)
        && escribir(r, TABLA, &[va as u32, (va >> 32) as u32]) == 2;
    // Las cuatro, enteras: la cabeza de cada una no se vuelve a escribir.
    // (Una `Ordenes` a la vez: son 4 KiB y esto corre en la pila del
    // syscall.)
    let mut primera = 0;
    for k in 0..RANURAS {
        let Some((o, _)) = ordenes(v, k, n / 3, if k == ranura(numero) { numero } else { 0 }) else { return false };
        bien = bien && escribir(r, empuje(k), &o.o[..o.n]) == o.n;
        if k == ranura(numero) {
            primera = o.n;
        }
    }
    let en = entrada(sombreador_va(empuje(ranura(numero))), primera as u32);
    bien && escribir(r, GR.gpfifo + 8 * e as u64, &[en as u32, (en >> 32) as u32]) == 2
}

/// Escribir `p` en la VRAM `dir` con la ventana YA abierta en su sitio.
fn poner<R: Registros>(r: &mut R, dir: u64, p: &[u32]) {
    let off = ventana(dir).1;
    for (i, &w) in p.iter().enumerate() {
        r.escribir(VENTANA + off + 4 * i as u32, w);
    }
}

/// **Enviar el fotograma `numero`** con `n` triangulos por la entrada `e`,
/// EN EL ANILLO: la cola de sus ordenes, la entrada del GPFIFO y GP_PUT, en
/// una sola apertura de la ventana y sin releer; y el timbre. Ni invalida la
/// MMU ni espera. Sus vertices ya estan en su ranura de RAM, y el kernel ya
/// vio pagado el fotograma `numero - RANURAS` (el que usaba la ranura).
pub fn enviar<R: Registros>(r: &mut R, e: u32, v: &Ventana, n: usize, numero: u32, ficha: u32) -> bool {
    if !crate::blur::entrada_valida(e) {
        return false;
    }
    let k = ranura(numero);
    let Some((o, cola)) = ordenes(v, k, n, numero) else { return false };
    let en = entrada(sombreador_va(empuje(k)), o.n as u32);
    let antes = r.leer(VENTANA_REG);
    r.escribir(VENTANA_REG, ventana(EMPUJE).0);
    poner(r, empuje(k) + 4 * cola as u64, &o.o[cola..o.n]);
    poner(r, GR.gpfifo + 8 * e as u64, &[en as u32, (en >> 32) as u32]);
    poner(r, GR.userd + GP_PUT, &[crate::blur::siguiente(e)]);
    r.escribir(VENTANA_REG, antes);
    r.escribir(TIMBRE, ficha);
    true
}

/// **La huella de lo FIJO del anillo**: los dos programas y la ventana --
/// lo que `armar` escribe y `enviar` no vuelve a escribir. NO los
/// triangulos: su numero va en la cola. FNV-1a de 64 bits.
pub fn huella(v: &Ventana, p: &Paquete) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64 ^ RANURAS as u64;
    let mut mezclar = |b: &[u8]| {
        for &x in b {
            h = (h ^ x as u64).wrapping_mul(0x0100_0000_01b3);
        }
    };
    mezclar(p.vs);
    mezclar(&[0xA5]);
    mezclar(p.ps);
    for x in [v.x0 as u64, v.y0 as u64, v.va, v.fila as u64, v.rgb as u64] {
        mezclar(&x.to_le_bytes());
    }
    h
}

/// **La valla**: el numero del ultimo fotograma que la 3060 pago.
pub fn pagado<R: Registros>(r: &mut R) -> u32 {
    leer32(r, SEMAFORO)
}

// Todo lo que `enviar` escribe cae en la MISMA ventana: la del tramo.
const _: () = assert!(ventana(EMPUJE).0 == ventana(GR.gpfifo).0 && ventana(EMPUJE).0 == ventana(GR.userd).0);
const _: () = assert!(ventana(EMPUJE + 4096 - 4).0 == ventana(EMPUJE).0);
// Las ranuras caben: vertices en su pagina, ordenes en EMPUJE.
const _: () = assert!(MAX_VERTICES * BYTES_VERTICE <= PASO as usize && RANURAS as u64 * PASO <= PAGINAS * 4096);
const _: () = assert!(RANURAS as usize * PALABRAS_RANURA * 4 <= 4096);
// La direccion alta de los vertices no cambia de una ranura a otra (el
// semaforo de la tabla solo escribe la baja) ni respecto de la de V0.
const _: () = assert!(VA >> 32 == (VA + PAGINAS * 4096 - 1) >> 32 && VA >> 32 == sombreador_va(crate::tuberia::VERTICES) >> 32);
// Tras el fractal, en la PT y en la IOVA; la valla, tras la del cubo.
const _: () = assert!(PT_PRIMERA as u64 >= crate::fractal::PT_PRIMERA as u64 + crate::fractal::PAGINAS && PT_PRIMERA as u64 + PAGINAS <= 512);
const _: () = assert!(IOVA >= crate::fractal::IOVA + crate::fractal::PAGINAS * 4096 && IOVA + PAGINAS * 4096 <= crate::volcado::IOVA);
const _: () = assert!(SEMAFORO >= cu::SEMAFORO_FIN + 16 && SEMAFORO + 4 <= SEMAFOROS + 0x200);

#[cfg(test)]
mod pruebas {
    extern crate std;

    use std::collections::BTreeMap;
    use std::vec::Vec;

    use super::*;
    use crate::copia::cabecera_en;

    /// La VRAM por la ventana, y los demas registros aparte.
    #[derive(Default)]
    struct Placa {
        vram: BTreeMap<u64, u32>,
        base: u32,
        regs: Vec<(u32, u32)>,
        lecturas: u32,
    }

    impl Registros for Placa {
        fn leer(&mut self, reg: u32) -> u32 {
            self.lecturas += 1;
            if reg == VENTANA_REG {
                return self.base;
            }
            assert!((VENTANA..VENTANA + crate::vram::VENTANA_MEDIDA as u32).contains(&reg));
            *self.vram.get(&(((self.base as u64) << 16) + (reg - VENTANA) as u64)).unwrap_or(&0)
        }
        fn escribir(&mut self, reg: u32, v: u32) {
            if reg == VENTANA_REG {
                self.base = v;
            } else if (VENTANA..VENTANA + crate::vram::VENTANA_MEDIDA as u32).contains(&reg) {
                self.vram.insert(((self.base as u64) << 16) + (reg - VENTANA) as u64, v);
            } else {
                self.regs.push((reg, v));
            }
        }
    }

    impl Placa {
        fn palabras(&self, dir: u64, n: usize) -> Vec<u32> {
            (0..n).map(|i| *self.vram.get(&(dir + 4 * i as u64)).unwrap_or(&0)).collect()
        }
    }

    fn la_ventana() -> Ventana {
        let gop = crate::pantalla::Pantalla { vram: 0x100_0000, pitch: 1920, ancho: 1920, alto: 1080, rgb: false };
        crate::cubo::ventana(&gop).unwrap()
    }

    fn programas() -> (&'static [u8], &'static [u8]) {
        let mut vs = std::vec![0u8; 4 * crate::tuberia::PALABRAS_VS];
        let mut ps = std::vec![0u8; 4 * crate::tuberia::PALABRAS_PS];
        crate::tuberia::bytes(&crate::tuberia::vertice(), &mut vs);
        crate::tuberia::bytes(&crate::tuberia::pixel(), &mut ps);
        (vs.leak(), ps.leak())
    }

    #[test]
    fn donde_vive() {
        assert_eq!(crate::mmu::indices(VA)[4], PT_PRIMERA);
        assert_eq!(VA, 0x2_0013_0000);
        assert_eq!(vertices_va(3), VA + 3 * 1024);
        assert_eq!(empuje(1), EMPUJE + 1024);
        let v = crate::mmu::pte_sistema(IOVA);
        assert_eq!((v >> 8) << 12, IOVA);
    }

    /// Las ordenes de una ranura son las de V1 `ligero` con el semaforo de
    /// la tabla (y su espera) delante del dibujo, y la valla en vez del
    /// semaforo del cubo.
    #[test]
    fn son_las_de_ligero_con_la_tabla() {
        let v = la_ventana();
        let (o, cola) = ordenes(&v, 2, 6, 77).unwrap();
        let l = crate::tuberia::ordenes_con(&v, 6, true);
        let (o, l) = (&o.o[..o.n], &l.o[..l.n]);
        // La cabeza de ligero, tal cual; y detras la tabla y su espera.
        let cabeza = cola - 7;
        assert_eq!(&o[..cabeza], &l[..cabeza]);
        let t = sombreador_va(TABLA);
        assert_eq!(&o[cabeza..cola], &[cabecera_en(0, td::SET_REPORT_SEMAPHORE_A, 4), (t >> 32) as u32, t as u32, vertices_va(2) as u32, td::INFORME, cabecera_en(0, td::WAIT_FOR_IDLE, 1), 0]);
        // La cola: el dibujo y la espera de ligero, y la valla con el numero.
        let s = sombreador_va(SEMAFORO);
        assert_eq!(&o[cola..o.len() - 4], &l[cabeza..l.len() - 4]);
        assert_eq!(&o[o.len() - 4..], &[(s >> 32) as u32, s as u32, 77, td::INFORME]);
        assert_eq!(o.len() - cola, 14);
        // Y caben las de mas triangulos.
        assert!(ordenes(&v, 3, CABEN, 1).is_some());
        assert!(ordenes(&v, 4, 1, 1).is_none() && ordenes(&v, 0, CABEN + 1, 1).is_none());
    }

    /// La cabeza no depende ni de los triangulos ni del numero: por eso
    /// basta escribir la cola en cada fotograma.
    #[test]
    fn la_cabeza_es_fija() {
        let v = la_ventana();
        let (a, ca) = ordenes(&v, 1, 4, 5).unwrap();
        let (b, cb) = ordenes(&v, 1, 6, 9).unwrap();
        assert_eq!(ca, cb);
        assert_eq!(&a.o[..ca], &b.o[..cb]);
        let (c, _) = ordenes(&v, 0, 4, 5).unwrap();
        assert_ne!(&a.o[..ca], &c.o[..ca], "otra ranura, otra tabla");
    }

    #[test]
    fn los_numeros_dan_la_vuelta() {
        assert!(ya(5, 5) && ya(6, 5) && !ya(4, 5));
        assert!(ya(2, u32::MAX - 1), "pagado dio la vuelta");
        assert!(!ya(u32::MAX, 3));
        assert!(!ya(0, 1), "la valla a cero: nada pagado");
    }

    /// **Armar y luego enviar = armar con ese fotograma**: lo que queda en
    /// la VRAM de la ranura tras la cola es EXACTAMENTE las ordenes enteras
    /// de ese fotograma, y `enviar` no relee nada ni toca otra cosa.
    #[test]
    fn enviar_completa_lo_que_armar_dejo() {
        let v = la_ventana();
        let (vs, ps) = programas();
        let vert = std::vec![3u8; 12 * BYTES_VERTICE].leak();
        let p = Paquete { ficha: 7, vs, ps, vertices: vert };
        let mut r = Placa::default();
        assert!(armar(&mut r, 10, &v, &p, 1));
        assert_eq!(r.palabras(SEMAFORO, 1), [0]);
        assert_eq!(r.palabras(TABLA, 2), [vertices_va(1) as u32, (vertices_va(1) >> 32) as u32]);
        for k in 0..RANURAS {
            let (o, _) = ordenes(&v, k, 4, if k == 1 { 1 } else { 0 }).unwrap();
            assert_eq!(r.palabras(empuje(k), o.n), &o.o[..o.n], "ranura {k}");
        }
        let (o1, _) = ordenes(&v, 1, 4, 1).unwrap();
        let en = entrada(sombreador_va(empuje(1)), o1.n as u32);
        assert_eq!(r.palabras(GR.gpfifo + 80, 2), [en as u32, (en >> 32) as u32]);

        // El fotograma 6 (ranura 2), con 3 triangulos, por la entrada 11.
        r.base = 0x42;
        let (lecturas, regs) = (r.lecturas, r.regs.len());
        assert!(enviar(&mut r, 11, &v, 3, 6, 0xF1C4));
        assert_eq!(r.lecturas - lecturas, 1, "solo la ventana de antes");
        assert_eq!(r.base, 0x42, "la ventana, como estaba");
        assert_eq!(&r.regs[regs..], &[(TIMBRE, 0xF1C4)]);
        let (o6, _) = ordenes(&v, 2, 3, 6).unwrap();
        assert_eq!(r.palabras(empuje(2), o6.n), &o6.o[..o6.n]);
        let en = entrada(sombreador_va(empuje(2)), o6.n as u32);
        assert_eq!(r.palabras(GR.gpfifo + 88, 2), [en as u32, (en >> 32) as u32]);
        assert_eq!(r.palabras(GR.userd + GP_PUT, 1), [12]);
        // Y la ranura 1 (la del primero) sigue entera.
        assert_eq!(r.palabras(empuje(1), o1.n), &o1.o[..o1.n]);
    }

    #[test]
    fn armar_no_acepta_lo_que_no_cabe() {
        let v = la_ventana();
        let (vs, ps) = programas();
        let mut r = Placa::default();
        let cuatro = std::vec![0u8; 4 * BYTES_VERTICE].leak();
        assert!(!armar(&mut r, 0, &v, &Paquete { ficha: 1, vs, ps, vertices: cuatro }, 1));
        let seis = std::vec![0u8; 6 * BYTES_VERTICE].leak();
        assert!(!armar(&mut r, crate::canal::GPFIFO_ENTRADAS, &v, &Paquete { ficha: 1, vs, ps, vertices: seis }, 1));
        assert!(!enviar(&mut r, crate::canal::GPFIFO_ENTRADAS, &v, 2, 1, 1));
    }

    /// La huella de lo fijo: la misma con otros vertices (y otro numero de
    /// triangulos); otra con otros programas o con la otra ventana.
    #[test]
    fn la_huella_de_lo_fijo() {
        let v = la_ventana();
        let (vs, ps) = programas();
        let (a, b) = (std::vec![1u8; 6 * BYTES_VERTICE].leak(), std::vec![2u8; 12 * BYTES_VERTICE].leak());
        let h = huella(&v, &Paquete { ficha: 1, vs, ps, vertices: a });
        assert_eq!(h, huella(&v, &Paquete { ficha: 9, vs, ps, vertices: b }));
        assert_ne!(h, huella(&v, &Paquete { ficha: 1, vs: ps, ps: vs, vertices: a }));
        let otra = Ventana { rgb: !v.rgb, ..v };
        assert_ne!(h, huella(&otra, &Paquete { ficha: 1, vs, ps, vertices: a }));
    }

    #[test]
    fn el_mapa() {
        let mut r = Placa::default();
        assert_eq!(mapear(&mut r), Some((1, 1)));
        assert_eq!(mapear(&mut r), Some((1, 1)), "otra vez, la misma: vale");
        escribir64(&mut r, pte_en(), crate::mmu::pte_sistema(0x1234_5000));
        assert_eq!(mapear(&mut r), None, "otra cosa ahi: no se pisa");
    }
}
