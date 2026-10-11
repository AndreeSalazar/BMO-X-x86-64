//! **ESTUDIO VIVO** (11-10): la luz de ESTUDIO, en TIEMPO REAL, porque se
//! RECUERDA.
//!
//! El propietario, el 11-10: *"si no siempre expone a la pantalla sino que
//! solo refleje la pantalla y los calculo se quede la CPU en tiempo real
//! [...] solo es como planos imaginacion que la CPU lo deja la ram que se
//! acuerda y lo rellena en tiempo real, con la GPU que solo completa y
//! educando mas y mas path tracing con renderman ambos fusionado"*.
//!
//! ```text
//!    NO COMER TODOS LOS PIXELES   cada fotograma calcula una PARTE de la
//!                                 pantalla (`Parte::Cuarto`: uno de cada
//!                                 2x2, rotando); lo demas se rellena con
//!                                 lo que ya habia en su cuadro
//!    LA RAM QUE SE ACUERDA        lo calculado se ACUMULA: con la camara
//!                                 quieta, cada fotograma suma muestras y la
//!                                 imagen va hacia la de ESTUDIO entero
//!    LOS PLANOS DE IMAGINACION    la MEMORIA DE LA LUZ: una tabla de la luz
//!                                 que sale de cada trocito mate del MUNDO
//!                                 (no de la pantalla: un hash de su sitio y
//!                                 su cara). La luz de una pared mate no
//!                                 depende de quien la mira, asi que la
//!                                 memoria SOBREVIVE a la camara que se
//!                                 mueve; los caminos terminan en ella (mas
//!                                 cortos, menos ruido) y cada camino nuevo
//!                                 la EDUCA
//!    LA GPU SOLO COMPLETA         lee la imagen y la memoria (el quirofano
//!                                 de RAM) y pinta: el paso de la 3060 (VE5)
//! ```
//!
//! Es lo que hacia la *irradiance cache* de RenderMan y lo que hacen las
//! caches de luz de los motores con path tracing en tiempo real. El JUEZ
//! sigue siendo ESTUDIO entero (`estudio.rs`): lo vivo se mide contra el.
//!
//! [carril]  VERDE     cuentas; la memoria se LEE en paralelo y se ESCRIBE
//!                     al final de cada fotograma (`junta`), sin cerrojos
//! [riesgo]  SESGO     la memoria promedia un trocito (`lado`): un poco de
//!                     luz se corre en las esquinas. Dicho y medido

use alloc::vec;
use alloc::vec::Vec;

use crate::estudio::{mas, por, Azar, Escena, V3};

/// Lo que un camino aprendio de un sitio mate: la luz que sale de el.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Apunte {
    pub p: V3,
    pub n: V3,
    pub l: V3,
}

#[derive(Clone, Copy, Debug, Default)]
struct Celda {
    /// 0, vacia.
    clave: u64,
    suma: V3,
    n: u32,
}

/// **La memoria de la luz**: una tabla hash de celdas del MUNDO (un cubo de
/// `lado` y una de las seis caras) con la luz media que sale de ellas.
#[derive(Clone, Debug)]
pub struct Memoria {
    celdas: Vec<Celda>,
    /// El lado de una celda, en unidades del mundo.
    pub lado: f32,
    /// Cuantas muestras tiene que tener una celda para que se use.
    pub minimo: u32,
}

const BUSCA: usize = 16;

impl Memoria {
    /// Una memoria de `2^bits` celdas.
    pub fn nueva(bits: u32, lado: f32, minimo: u32) -> Memoria {
        Memoria { celdas: vec![Celda::default(); 1 << bits], lado, minimo }
    }

    fn clave(&self, p: V3, n: V3) -> u64 {
        let q = |x: f32| (bmo_prometeo::mates::suelo(x / self.lado) as i64 & 0x1F_FFFF) as u64;
        let (ax, ay, az) = (n[0].abs(), n[1].abs(), n[2].abs());
        let cara = if ax >= ay && ax >= az {
            (n[0] > 0.0) as u64
        } else if ay >= az {
            2 + (n[1] > 0.0) as u64
        } else {
            4 + (n[2] > 0.0) as u64
        };
        // nunca 0 (la de una celda vacia)
        (q(p[0]) | q(p[1]) << 21 | q(p[2]) << 42) ^ (cara + 1) << 61 | 1
    }

    fn sitio(&self, clave: u64) -> usize {
        let h = clave.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        (h >> 32) as usize & (self.celdas.len() - 1)
    }

    /// La luz que sale de `p` (cara `n`), si su celda ya sabe bastante.
    pub fn mirar(&self, p: V3, n: V3) -> Option<V3> {
        let c = self.clave(p, n);
        let i = self.sitio(c);
        for k in 0..BUSCA {
            let celda = &self.celdas[(i + k) & (self.celdas.len() - 1)];
            if celda.clave == c {
                return (celda.n >= self.minimo).then(|| por(celda.suma, 1.0 / celda.n as f32));
            }
            if celda.clave == 0 {
                return None;
            }
        }
        None
    }

    /// Educarla: lo que aprendio un camino.
    pub fn apuntar(&mut self, a: &Apunte) {
        let c = self.clave(a.p, a.n);
        let i = self.sitio(c);
        let m = self.celdas.len() - 1;
        for k in 0..BUSCA {
            let celda = &mut self.celdas[(i + k) & m];
            if celda.clave == 0 || celda.clave == c {
                celda.clave = c;
                celda.suma = mas(celda.suma, a.l);
                celda.n += 1;
                return;
            }
        }
        // llena alrededor: no se apunta (la memoria es una ayuda, no la verdad)
    }

    /// Cuantas celdas saben algo.
    pub fn llenas(&self) -> usize {
        self.celdas.iter().filter(|c| c.clave != 0).count()
    }
}

/// **Que pixeles calcula un fotograma.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Parte {
    /// Todos.
    Todos,
    /// Uno de cada cuadro de 2x2, rotando: en cuatro fotogramas, todos.
    Cuarto,
}

/// **Un cuaderno de apuntes SIN monton** (VE2: los obreros de BMO-X no
/// piden memoria ni llaman al sistema): hojas que ya existen; lo que no
/// cabe no se apunta (la memoria es una ayuda, no la verdad).
pub struct Cuaderno<'a> {
    pub hojas: &'a mut [Apunte],
    pub n: usize,
}

impl Cuaderno<'_> {
    pub fn apunta(&mut self, a: Apunte) {
        if let Some(h) = self.hojas.get_mut(self.n) {
            *h = a;
            self.n += 1;
        }
    }
}

/// Los apuntes de un camino como mucho (sus sitios mate).
pub const APUNTES_POR_CAMINO: usize = 4;

/// Lo que calculo un trozo de filas en un fotograma: cada pixel con su
/// muestra nueva, y lo aprendido para la memoria.
#[derive(Debug, Default)]
pub struct Trozo {
    pub pixeles: Vec<(usize, V3)>,
    pub apuntes: Vec<Apunte>,
    /// Los caminos que se calcularon.
    pub caminos: u64,
}

/// **La imagen viva**: la luz acumulada de cada pixel y cuantas muestras
/// lleva, el fotograma, y la memoria (si se usa).
#[derive(Clone, Debug)]
pub struct Vivo {
    pub ancho: u32,
    pub alto: u32,
    suma: Vec<V3>,
    cuenta: Vec<u32>,
    pub fotograma: u32,
    pub memoria: Option<Memoria>,
}

impl Vivo {
    pub fn nuevo(ancho: u32, alto: u32, memoria: Option<Memoria>) -> Vivo {
        let n = (ancho * alto) as usize;
        Vivo { ancho, alto, suma: vec![[0.0; 3]; n], cuenta: vec![0; n], fotograma: 0, memoria }
    }

    /// La camara se movio: la imagen acumulada ya no vale (la memoria del
    /// MUNDO, si; esa se queda).
    pub fn olvida_imagen(&mut self) {
        self.suma.fill([0.0; 3]);
        self.cuenta.fill(0);
    }

    /// Si el pixel (x, y) toca en este fotograma.
    pub fn toca(&self, x: u32, y: u32, parte: Parte) -> bool {
        match parte {
            Parte::Todos => true,
            Parte::Cuarto => (x & 1) + 2 * (y & 1) == self.fotograma % 4,
        }
    }

    /// **Lo que calculan las filas `y0..y1` en este fotograma**: un camino
    /// por cada pixel que toca, con la memoria (solo LEIDA: los nucleos van
    /// en paralelo) y apuntando lo aprendido.
    pub fn calcula(&self, e: &Escena, y0: u32, y1: u32, parte: Parte) -> Trozo {
        let mut t = Trozo::default();
        for y in y0..y1.min(self.alto) {
            for x in 0..self.ancho {
                if !self.toca(x, y, parte) {
                    continue;
                }
                let i = (y * self.ancho + x) as usize;
                let mut az = Azar::nuevo((i as u32).wrapping_mul(2_654_435_761) ^ self.fotograma.wrapping_mul(97_531) ^ 0x5bd1_e995);
                let (o, d) = e.rayo(x, y, self.ancho, self.alto, &mut az);
                let mut apunta = |a: Apunte| t.apuntes.push(a);
                let apuntes: Option<&mut dyn FnMut(Apunte)> = if self.memoria.is_some() { Some(&mut apunta) } else { None };
                let l = e.camino_con(o, d, &mut az, self.memoria.as_ref(), apuntes);
                t.pixeles.push((i, l));
                t.caminos += 1;
            }
        }
        t
    }

    /// **Lo que calculan las filas `y0..y1`, SIN MONTON** (VE2, los obreros
    /// de BMO-X): la muestra nueva de cada pixel que toca, en `nuevas` (las
    /// de esas filas, fila a fila desde `y0`; los que no tocan, sin tocar) y
    /// lo aprendido en `cuaderno`. Devuelve los caminos.
    pub fn calcula_en(&self, e: &Escena, y0: u32, y1: u32, parte: Parte, nuevas: &mut [V3], cuaderno: &mut Cuaderno) -> u64 {
        let mut caminos = 0;
        for y in y0..y1.min(self.alto) {
            for x in 0..self.ancho {
                if !self.toca(x, y, parte) {
                    continue;
                }
                let i = (y * self.ancho + x) as usize;
                let mut az = Azar::nuevo((i as u32).wrapping_mul(2_654_435_761) ^ self.fotograma.wrapping_mul(97_531) ^ 0x5bd1_e995);
                let (o, d) = e.rayo(x, y, self.ancho, self.alto, &mut az);
                let mut apunta = |a: Apunte| cuaderno.apunta(a);
                let apuntes: Option<&mut dyn FnMut(Apunte)> = if self.memoria.is_some() { Some(&mut apunta) } else { None };
                nuevas[((y - y0) * self.ancho + x) as usize] = e.camino_con(o, d, &mut az, self.memoria.as_ref(), apuntes);
                caminos += 1;
            }
        }
        caminos
    }

    /// Lo de [`Vivo::calcula_en`], a la imagen y a la memoria (uno solo
    /// escribe). El fotograma lo cierra [`Vivo::cierra`].
    pub fn pon_filas(&mut self, y0: u32, y1: u32, parte: Parte, nuevas: &[V3], apuntes: &[Apunte]) {
        for y in y0..y1.min(self.alto) {
            for x in 0..self.ancho {
                if self.toca(x, y, parte) {
                    let i = (y * self.ancho + x) as usize;
                    self.suma[i] = mas(self.suma[i], nuevas[((y - y0) * self.ancho + x) as usize]);
                    self.cuenta[i] += 1;
                }
            }
        }
        if let Some(m) = self.memoria.as_mut() {
            for a in apuntes {
                m.apuntar(a);
            }
        }
    }

    /// El fin de un fotograma hecho con [`Vivo::pon_filas`].
    pub fn cierra(&mut self) {
        self.fotograma += 1;
    }

    /// **El fin del fotograma**: lo de cada trozo, a la imagen y a la
    /// memoria (aqui, uno solo escribe).
    pub fn junta(&mut self, trozos: impl IntoIterator<Item = Trozo>) {
        for t in trozos {
            for (i, l) in t.pixeles {
                self.suma[i] = mas(self.suma[i], l);
                self.cuenta[i] += 1;
            }
            if let Some(m) = self.memoria.as_mut() {
                for a in &t.apuntes {
                    m.apuntar(a);
                }
            }
        }
        self.fotograma += 1;
    }

    /// Un fotograma entero en un nucleo (las pruebas; el ejemplo reparte).
    pub fn paso(&mut self, e: &Escena, parte: Parte) -> u64 {
        let t = self.calcula(e, 0, self.alto, parte);
        let caminos = t.caminos;
        self.junta([t]);
        caminos
    }

    /// Cuantas muestras lleva el pixel `i`.
    pub fn muestras(&self, i: usize) -> u32 {
        self.cuenta[i]
    }

    /// **Lo que se ve**: la media de cada pixel; el que aun no tiene
    /// muestras, la de los de su cuadro de 2x2 que si (la RAM que rellena).
    pub fn imagen(&self) -> Vec<V3> {
        let (w, h) = (self.ancho as usize, self.alto as usize);
        let media = |i: usize| por(self.suma[i], 1.0 / self.cuenta[i] as f32);
        (0..w * h)
            .map(|i| {
                if self.cuenta[i] > 0 {
                    return media(i);
                }
                let (x, y) = (i % w & !1, i / w & !1);
                let mut s = [0.0; 3];
                let mut k = 0;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let j = (y + dy).min(h - 1) * w + (x + dx).min(w - 1);
                    if self.cuenta[j] > 0 {
                        s = mas(s, media(j));
                        k += 1;
                    }
                }
                if k == 0 {
                    [0.0; 3]
                } else {
                    por(s, 1.0 / k as f32)
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::estudio::{caja_de_cornell, Objeto};

    const W: u32 = 32;
    const H: u32 = 24;

    fn escena(o: &[Objeto]) -> Escena<'_> {
        let (_, luz, camara) = caja_de_cornell();
        Escena { objetos: o, luz, camara }
    }

    /// El JUEZ: ESTUDIO entero, muchas muestras.
    fn referencia(e: &Escena) -> Vec<V3> {
        (0..W * H).map(|i| e.pixel(i % W, i / W, W, H, 256)).collect()
    }

    /// El error medio contra el juez (la luz, con un techo: la luz de la
    /// lampara no manda sobre todo lo demas).
    fn error(a: &[V3], b: &[V3]) -> f32 {
        let c = |x: f32| x.min(1.5);
        a.iter().zip(b).map(|(p, q)| (0..3).map(|k| (c(p[k]) - c(q[k])).abs()).sum::<f32>()).sum::<f32>() / (a.len() * 3) as f32
    }

    /// ** LA RAM QUE SE ACUERDA: con la camara quieta, cada fotograma suma
    /// una muestra por pixel y la imagen va hacia la de ESTUDIO entero.
    #[test]
    fn a_still_camera_converges_to_the_studio_image() {
        let (o, _, _) = caja_de_cornell();
        let e = escena(&o);
        let juez = referencia(&e);
        let mut v = Vivo::nuevo(W, H, None);
        v.paso(&e, Parte::Todos);
        let al_principio = error(&v.imagen(), &juez);
        for _ in 0..31 {
            v.paso(&e, Parte::Todos);
        }
        let despues = error(&v.imagen(), &juez);
        assert!(despues < al_principio * 0.4, "1 muestra {al_principio}, 32 muestras {despues}");
    }

    /// ** NO COMER TODOS LOS PIXELES: con `Cuarto`, cada fotograma calcula
    /// uno de cada cuatro; en cuatro, todos tienen su muestra -- y desde el
    /// primero no hay agujeros: los rellena su cuadro --.
    #[test]
    fn a_quarter_per_frame_fills_every_pixel_in_four() {
        let (o, _, _) = caja_de_cornell();
        let e = escena(&o);
        let mut v = Vivo::nuevo(W, H, None);
        let caminos = v.paso(&e, Parte::Cuarto);
        assert_eq!(caminos, (W * H / 4) as u64, "un cuarto de los pixeles");
        // cada cuadro de 2x2: UNO calculado, y los otros tres lo muestran a el
        let img = v.imagen();
        for by in (0..H).step_by(2) {
            for bx in (0..W).step_by(2) {
                let cuadro = [(bx, by), (bx + 1, by), (bx, by + 1), (bx + 1, by + 1)].map(|(x, y)| (y * W + x) as usize);
                let hechos: Vec<usize> = cuadro.iter().copied().filter(|&i| v.muestras(i) == 1).collect();
                assert_eq!(hechos.len(), 1, "el cuadro ({bx}, {by})");
                assert!(cuadro.iter().all(|&i| img[i] == img[hechos[0]]), "el cuadro ({bx}, {by}) se rellena con el suyo");
            }
        }
        for _ in 0..3 {
            v.paso(&e, Parte::Cuarto);
        }
        assert!((0..(W * H) as usize).all(|i| v.muestras(i) == 1));
    }

    /// ** SIN MONTON, LO MISMO (VE2): los obreros de BMO-X calculan sus
    /// filas en buferes que ya existen y apuntan en un cuaderno; repartido
    /// en tres trozos, la imagen y la memoria salen IGUAL que con `paso`.
    #[test]
    fn without_a_heap_in_slices_it_is_the_same() {
        let (o, _, _) = caja_de_cornell();
        let e = escena(&o);
        let mut a = Vivo::nuevo(W, H, Some(Memoria::nueva(12, 0.08, 4)));
        let mut b = a.clone();
        for _ in 0..6 {
            a.paso(&e, Parte::Cuarto);
            let mut nuevas = vec![[0.0f32; 3]; (W * H) as usize];
            let mut hojas = vec![Apunte { p: [0.0; 3], n: [0.0; 3], l: [0.0; 3] }; (W * H) as usize * APUNTES_POR_CAMINO];
            let cortes = [0, 7, 15, H];
            let (mut trozos, mut resto_n, mut resto_h) = (Vec::new(), &mut nuevas[..], &mut hojas[..]);
            for k in 0..3 {
                let filas = (cortes[k + 1] - cortes[k]) as usize;
                let (n, r) = resto_n.split_at_mut(filas * W as usize);
                let (h, rh) = resto_h.split_at_mut(filas * W as usize * APUNTES_POR_CAMINO);
                resto_n = r;
                resto_h = rh;
                let mut c = Cuaderno { hojas: h, n: 0 };
                b.calcula_en(&e, cortes[k], cortes[k + 1], Parte::Cuarto, n, &mut c);
                trozos.push((cortes[k], cortes[k + 1], n.to_vec(), c.hojas[..c.n].to_vec()));
            }
            for (y0, y1, n, h) in trozos {
                b.pon_filas(y0, y1, Parte::Cuarto, &n, &h);
            }
            b.cierra();
        }
        assert_eq!(a.imagen(), b.imagen());
        assert_eq!(a.memoria.as_ref().unwrap().llenas(), b.memoria.as_ref().unwrap().llenas());
    }

    /// *** LOS PLANOS DE IMAGINACION: la memoria de la luz, educada unos
    /// fotogramas, SOBREVIVE a la camara que se mueve (`olvida_imagen`
    /// tira la imagen, no la memoria): con ella, los primeros fotogramas
    /// despues de moverse se parecen MAS al juez que sin ella (lo que cuesta,
    /// lo mide el ejemplo `estudio_vivo`).
    #[test]
    fn the_light_memory_survives_the_camera_and_helps() {
        let (o, _, _) = caja_de_cornell();
        let e = escena(&o);
        let juez = referencia(&e);
        let mut con = Vivo::nuevo(W, H, Some(Memoria::nueva(14, 0.08, 8)));
        for _ in 0..24 {
            con.paso(&e, Parte::Todos);
        }
        assert!(con.memoria.as_ref().unwrap().llenas() > 100);
        con.olvida_imagen();
        let mut sin = Vivo::nuevo(W, H, None);
        sin.fotograma = con.fotograma;
        for _ in 0..4 {
            con.paso(&e, Parte::Todos);
            sin.paso(&e, Parte::Todos);
        }
        let (ec, es) = (error(&con.imagen(), &juez), error(&sin.imagen(), &juez));
        assert!(ec < es, "con memoria {ec}, sin ella {es}");
    }
}
