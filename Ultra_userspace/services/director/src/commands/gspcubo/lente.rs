//! **`gpu verrano derivadas` -- LA LENTE (L1/L2, 14-10)**: la 3060 dibuja la
//! pantalla entera con un programa de pixeles que saca `ddx` y `ddy` de la
//! posicion de cada pixel (`bmo_gpu_ga10x::derivadas`), y aqui se cuenta, en
//! una rejilla de pixeles, que salio en cada canal: lo que tiene que dar no se
//! discute (`ddx(x) = 1`, `ddy(y) = 1`, lo cruzado 0). Lo que no cuadre lo
//! dice canal a canal: es lo que Mesa no sabia (que hilo del cuadro es cada
//! uno en la pantalla, y el signo de FSWZADD).
//!
//! [consumo] NADA      corre cuando el propietario lo teclea: dos dibujos

use bmo_gpu_ga10x::cubo as cu;
use bmo_gpu_ga10x::derivadas::{esperado, receta, valor, Prueba, BYTES_DATOS};
use bmo_userland as bmo;

use super::super::After;
use super::Texto;
use crate::desktop::Desktop;
use crate::scene::output::{INK_ERR, INK_GOOD, INK_PLAIN};

/// La rejilla: un pixel de cada `PASO` en x y en y (los de un cuadro entero:
/// lejos de la diagonal no importa; cerca, el cuadro lleva ayudantes y tiene
/// que dar lo mismo).
const PASO: u32 = 23;

/// Lo que se vio de un canal: cuantos 1, 0, -1 y otra cosa.
#[derive(Default, Clone, Copy)]
struct Cuenta {
    uno: u32,
    cero: u32,
    menos: u32,
    raro: u32,
}

impl Cuenta {
    fn poner(&mut self, b: u8) {
        match valor(b) {
            Some(1) => self.uno += 1,
            Some(0) => self.cero += 1,
            Some(-1) => self.menos += 1,
            _ => self.raro += 1,
        }
    }
    fn total(&self) -> u32 {
        self.uno + self.cero + self.menos + self.raro
    }
    fn de(&self, v: i8) -> u32 {
        match v {
            1 => self.uno,
            0 => self.cero,
            _ => self.menos,
        }
    }
}

const NOMBRES: [&[u8]; 2] = [b"GRUESA (SHFL.IDX)", b"FSWZADD"];
const CANALES: [[&[u8]; 4]; 2] = [[b"ddx(x)", b"ddy(y)", b"ddx(y)", b"ddy(x)"], [b"fswz ddx(x)", b"fswz ddy(y)", b"pares ddx(x)", b"cero"]];

/// **`gpu verrano derivadas`**.
pub(super) fn derivadas(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    use bmo_gpu_ga10x::receta as rc;
    let (w, h) = (cu::ANCHO, cu::ALTO);
    if p.ancho < w || p.alto < h {
        return linea(dsk, b"  NO  la pantalla es mas chica que 1280x720", INK_ERR);
    }
    if super::super::verificar::preparar_hasta(dsk, p, b"lienzo").is_err() {
        dsk.field.n = 0;
        return After::Settle;
    }
    if let Err(m) = super::super::gspcomputo::ficha_del_gr() {
        return motivo(dsk, m);
    }
    let pixeles = (w * h) as usize;
    let (Some(destino), Some(cajab)) = (bmo::Memoria::request(4 * pixeles as u64), bmo::Memoria::request((rc::MAX_RECETA as u64).div_ceil(4096) * 4096)) else {
        return linea(dsk, b"  NO  sin memoria para el destino y la receta", INK_ERR);
    };
    // SAFETY: dos bloques de este proceso, alineados a pagina, de las medidas
    // pedidas, que solo se usan aqui y no se pisan.
    let (dst, caja) = unsafe { (core::slice::from_raw_parts_mut(destino.base() as *mut u32, pixeles), core::slice::from_raw_parts_mut(cajab.base(), rc::MAX_RECETA)) };
    let mut datos = [0u8; BYTES_DATOS];
    let mut cuentas = [[Cuenta::default(); 4]; 2];
    let mut bien = [true; 2];
    for (k, prueba) in [Prueba::Gruesa, Prueba::Fswzadd].into_iter().enumerate() {
        if receta(prueba, destino.base() as u64, &mut datos, caja).is_none() {
            return linea(dsk, b"  NO  la receta de la lente no se sostiene (el banco la escribe: esto no deberia pasar)", INK_ERR);
        }
        match bmo::iommu_orden_con(bmo::IOMMU_OP_GPU_DIBUJAR, caja.as_ptr() as u64) {
            Ok(v) if cu::sano(v) => {}
            Ok(v) => {
                let (us, tris, etapas, lanzado) = cu::desempaquetar(v);
                let mut t = Texto::nuevo();
                t.t(b"  NO  la 3060 no pago la lente ").t(NOMBRES[k]).t(b": lanzado ").d(lanzado as u64).t(b", escalera ").d(etapas as u64).t(b", ").d(tris as u64).t(b" triangulos, ").d(us as u64).t(b" us (`gpu aviso` dira si fue un Xid; `gpu revivir` la levanta)");
                return linea(dsk, t.s(), INK_ERR);
            }
            Err(m) => return motivo(dsk, m),
        }
        let mut y = PASO / 2;
        while y < h {
            let mut x = PASO / 2;
            while x < w {
                // SAFETY: dentro del destino; lo escribio la 3060 por DMA.
                let c = unsafe { core::ptr::read_volatile(&dst[(y * w + x) as usize]) };
                for (j, cuenta) in cuentas[k].iter_mut().enumerate() {
                    cuenta.poner((c >> (8 * j)) as u8);
                }
                x += PASO;
            }
            y += PASO;
        }
        let e = esperado(prueba);
        bien[k] = (0..4).all(|j| cuentas[k][j].de(e[j]) == cuentas[k][j].total());
    }
    // Lo ultimo que pinto, en la ventana (R y G: el verde y el rojo claros si
    // todo es 1).
    let (x0, y0) = super::sm86::ventana(p);
    p.marcar(x0, y0, w, h);
    for y in 0..h {
        for x in 0..w {
            let c = dst[(y * w + x) as usize];
            p.punto_ya_marcado(x0 + x, y0 + y, (c & 0xFF) << 16 | (c & 0xFF00) | (c >> 16 & 0xFF));
        }
    }
    p.vaciar();
    let g = &mut dsk.out.grid;
    for k in 0..2 {
        g.with_ink(if bien[k] { INK_GOOD } else { INK_ERR });
        let mut t = Texto::nuevo();
        t.t(b"  lente ").t(NOMBRES[k]).t(if bien[k] { b": IGUAL a lo que se supone" as &[u8] } else { b": DISTINTO" }).t(b"\n");
        g.text(t.s());
        g.with_ink(INK_PLAIN);
        let e = esperado([Prueba::Gruesa, Prueba::Fswzadd][k]);
        for j in 0..4 {
            let c = cuentas[k][j];
            let mut t = Texto::nuevo();
            t.t(b"           ").t(CANALES[k][j]).t(b": tiene que dar ");
            if e[j] < 0 {
                t.t(b"-1");
            } else {
                t.d(e[j] as u64);
            }
            t.t(b"; dio 1 en ").d(c.uno as u64).t(b", 0 en ").d(c.cero as u64).t(b", -1 en ").d(c.menos as u64).t(b", otra cosa en ").d(c.raro as u64).t(b" de ").d(c.total() as u64).t(b"\n");
            g.text(t.s());
        }
    }
    let n = (bien[0] as u64) | (bien[1] as u64) << 1;
    super::super::datos::anotar(b"gpu verrano derivadas", n, b"bit 0 la gruesa, bit 1 FSWZADD");
    dsk.field.n = 0;
    After::Settle
}

fn motivo(dsk: &mut Desktop, m: u32) -> After {
    let g = &mut dsk.out.grid;
    g.with_ink(INK_ERR);
    g.text(b"  NO  la lente no dibujo: motivo ");
    g.dec(m as u64);
    g.text(b" (`gpu` lo explica)\n");
    g.with_ink(INK_PLAIN);
    dsk.field.n = 0;
    After::Settle
}

fn linea(dsk: &mut Desktop, texto: &[u8], tinta: u8) -> After {
    let g = &mut dsk.out.grid;
    g.with_ink(tinta);
    g.text(texto);
    g.byte(b'\n');
    g.with_ink(INK_PLAIN);
    dsk.field.n = 0;
    After::Settle
}
