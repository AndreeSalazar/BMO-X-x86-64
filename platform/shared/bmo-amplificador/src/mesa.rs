//! **LA MESA: N pistas con nombre, y un maestro.**
//!
//! El paso M0 de `docs/plan/PLAN_LA_MESA.md`, y la mitad de abajo de lo que el
//! propietario pidio el 2026-09-22: *"que el audio se divida por completo, el
//! audio de juegos, de apps, de cualquiera, por si quiero editar ese proximo
//! video"*.
//!
//! # Por que esto cabe en este crate y no en uno nuevo
//!
//! Una PISTA es una [`Ganancia`] con su [`Medidor`]; la MESA es una
//! [`sumar`] de todas ellas y un [`Amplificador`] al final. O sea que no hay
//! pieza nueva que inventar: **hay un arreglo de las que ya estan**. Meterlo
//! en otro crate seria partir la misma cuenta en dos sitios.
//!
//! # El modelo, en cuatro reglas
//!
//! ```text
//!    1. una pista tiene NOMBRE          un pid cambia cada arranque, "juego" no
//!    2. MUDO calla esa, SOLO calla las OTRAS
//!    3. el medidor es de la PISTA       el maestro dice si te pasas,
//!                                       la pista dice QUIEN se pasa
//!    4. el que no pide pista, va a `otros`
//! ```
//!
//! *** LA REGLA 3 ES LA QUE LO HACE UTIL, y es la que ningun escritorio da de
//! serie. Un mezclador corriente muestra un pico por aplicacion; aqui cada
//! pista dice su pico, su RMS y --cuando se derive-- puede irse a un fichero
//! con su ganancia puesta y ANTES del maestro, porque el maestro es para los
//! oidos y la grabacion es para despues.
//!
//! # Lo que NO hace
//!
//! No abre nada, no habla con el aparato y no sabe de ficheros: entran
//! muestras por pista y sale un bloque. Quien lo llene y donde lo mande es
//! cosa del productor (M1).

use crate::{sumar, Amplificador, Ganancia, Medidor, MilesimasDb};

/// Cuantas pistas caben. Ocho es lo que usa DOOM para sus efectos y lo que un
/// escritorio necesita para separar juego, musica, sistema y voz con sitio de
/// sobra.
pub const PISTAS: usize = 8;

/// Lo que cabe en el nombre de una pista. Doce bastan para `sistema` o
/// `navegador` y caben en una columna de la ventana sin partirse.
pub const NOMBRE: usize = 12;

/// **Una pista: una fuente con nombre y sus perillas.**
#[derive(Clone, Copy, Debug)]
pub struct Pista {
    nombre: [u8; NOMBRE],
    nombre_n: u8,
    /// Esta abierta? Una pista cerrada no suena y no aparece en la ventana.
    abierta: bool,
    pub ganancia: Ganancia,
    /// Callada a mano.
    pub muda: bool,
    /// En SOLO: si alguna lo esta, las que no lo esten no suenan.
    pub solo: bool,
    /// Lo que salio DE ESTA pista, ya con su ganancia y antes del maestro.
    pub medidor: Medidor,
}

impl Pista {
    pub const VACIA: Pista = Pista {
        nombre: [0; NOMBRE],
        nombre_n: 0,
        abierta: false,
        ganancia: Ganancia::UNIDAD,
        muda: false,
        solo: false,
        medidor: Medidor::nuevo(),
    };

    /// El nombre, tal como se puso.
    pub fn nombre(&self) -> &[u8] {
        &self.nombre[..self.nombre_n as usize]
    }

    pub fn abierta(&self) -> bool {
        self.abierta
    }
}

/// **La mesa entera.**
#[derive(Clone, Copy, Debug)]
pub struct Mesa {
    pistas: [Pista; PISTAS],
    /// El maestro: la ganancia que se aplica a la suma, su limite y su medidor.
    pub maestro: Amplificador,
}

impl Mesa {
    /// Una mesa para un aparato de `hz`, con todas las pistas cerradas.
    pub fn nueva(hz: u32) -> Mesa {
        Mesa { pistas: [Pista::VACIA; PISTAS], maestro: Amplificador::nuevo(hz) }
    }

    /// **Abrir una pista con ese nombre.** Si ya hay una con ese nombre, se
    /// devuelve LA MISMA: dos programas que dicen "juego" comparten pista, que
    /// es lo que un propietario espera al verlo en la ventana.
    ///
    /// `None` = no quedan pistas. No se roba una ajena para hacer sitio: la
    /// respuesta correcta a "no cabes" es decirlo.
    pub fn abrir(&mut self, nombre: &[u8]) -> Option<usize> {
        if let Some(i) = self.buscar(nombre) {
            return Some(i);
        }
        let libre = self.pistas.iter().position(|p| !p.abierta)?;
        let n = nombre.len().min(NOMBRE);
        let p = &mut self.pistas[libre];
        *p = Pista::VACIA;
        p.nombre[..n].copy_from_slice(&nombre[..n]);
        p.nombre_n = n as u8;
        p.abierta = true;
        Some(libre)
    }

    /// La pista con ese nombre, si esta abierta.
    pub fn buscar(&self, nombre: &[u8]) -> Option<usize> {
        let n = nombre.len().min(NOMBRE);
        self.pistas
            .iter()
            .position(|p| p.abierta && p.nombre() == &nombre[..n])
    }

    /// Cerrarla. Sus perillas se olvidan: una pista que vuelve es nueva.
    pub fn cerrar(&mut self, i: usize) {
        if i < PISTAS {
            self.pistas[i] = Pista::VACIA;
        }
    }

    pub fn pista(&self, i: usize) -> Option<&Pista> {
        self.pistas.get(i).filter(|p| p.abierta)
    }

    pub fn pista_mut(&mut self, i: usize) -> Option<&mut Pista> {
        self.pistas.get_mut(i).filter(|p| p.abierta)
    }

    /// Cuantas hay abiertas.
    pub fn abiertas(&self) -> usize {
        self.pistas.iter().filter(|p| p.abierta).count()
    }

    /// **Hay alguna en SOLO?** Si la hay, las demas no suenan.
    pub fn hay_solo(&self) -> bool {
        self.pistas.iter().any(|p| p.abierta && p.solo)
    }

    /// **Suena esta pista ahora mismo?** Es la regla de mudo y solo en un
    /// sitio, para que la ventana y la mezcla contesten lo mismo.
    pub fn suena(&self, i: usize) -> bool {
        match self.pista(i) {
            None => false,
            Some(p) => {
                if p.muda {
                    return false;
                }
                if self.hay_solo() {
                    return p.solo;
                }
                true
            }
        }
    }

    /// Poner la ganancia de una pista, en 1/256 de dB.
    pub fn subir(&mut self, i: usize, db: MilesimasDb) -> Option<Ganancia> {
        let g = Ganancia::db(db);
        let p = self.pista_mut(i)?;
        p.ganancia = g;
        Some(g)
    }

    /// **Echar las muestras de una pista al acumulador**, con su ganancia y su
    /// medidor.
    ///
    /// Se llama una vez por pista y por bloque; el orden da igual, porque
    /// sumar es sumar. Si la pista no suena (muda, o hay otra en solo), no
    /// toca el acumulador **y su medidor se queda a cero**: una pista callada
    /// que mostrara barras seria un instrumento que miente.
    pub fn echar(&mut self, i: usize, muestras: &[i16], acumulador: &mut [i32]) {
        if !self.suena(i) {
            return;
        }
        let Some(p) = self.pista_mut(i) else { return };
        let g = p.ganancia;
        // El medidor de la pista mira lo que ELLA aporta, ya con su ganancia:
        // por eso se mide aqui y no en el acumulador, donde ya esta mezclado
        // con las demas y no se sabria de quien es el pico.
        let n = muestras.len().min(acumulador.len());
        for &m in &muestras[..n] {
            let y = g.aplicar(m as i32);
            p.medidor.mirar_uno(y);
        }
        sumar(acumulador, &muestras[..n], g);
    }

    /// **El maestro: del acumulador a lo que sale al aparato.** Ganancia,
    /// limite y medidor, en una pasada.
    pub fn maestro(&mut self, acumulador: &[i32], salida: &mut [i16]) {
        self.maestro.bloque(acumulador, salida);
    }

    /// Los medidores de todas las pistas a cero, para el tramo siguiente. El
    /// del maestro NO se toca: sus cuentas de `sujetadas` y `dobladas` son de
    /// la sesion entera y decirlas por tramos las haria inutiles.
    pub fn olvidar_medidas(&mut self) {
        for p in self.pistas.iter_mut() {
            p.medidor.olvidar();
        }
    }
}

#[cfg(test)]
mod pruebas {
    extern crate std;
    use super::*;
    use crate::{a_dbfs, DB, PLENO};

    fn mesa() -> Mesa {
        Mesa::nueva(48_000)
    }

    #[test]
    fn abrir_dos_veces_el_mismo_nombre_da_la_misma_pista() {
        let mut m = mesa();
        let a = m.abrir(b"juego").unwrap();
        let b = m.abrir(b"juego").unwrap();
        assert_eq!(a, b, "dos programas que dicen `juego` comparten pista");
        assert_eq!(m.abiertas(), 1);
        assert_eq!(m.pista(a).unwrap().nombre(), b"juego");
    }

    #[test]
    fn cuando_no_caben_mas_se_dice_y_no_se_roba_una() {
        let mut m = mesa();
        for i in 0..PISTAS {
            let n = [b'p', b'0' + i as u8];
            assert!(m.abrir(&n).is_some(), "no cupo la {}", i);
        }
        assert_eq!(m.abrir(b"otra"), None);
        // Y las que habia siguen ahi: no se robo ninguna.
        assert_eq!(m.abiertas(), PISTAS);
        assert_eq!(m.pista(0).unwrap().nombre(), b"p0");
    }

    #[test]
    fn un_nombre_largo_se_recorta_y_no_desborda() {
        let mut m = mesa();
        let i = m.abrir(b"un-nombre-larguisimo-de-verdad").unwrap();
        assert_eq!(m.pista(i).unwrap().nombre().len(), NOMBRE);
        // Y se encuentra por su forma recortada.
        assert_eq!(m.buscar(b"un-nombre-larguisimo"), Some(i));
    }

    #[test]
    fn mezclar_dos_pistas_es_sumarlas() {
        let mut m = mesa();
        let a = m.abrir(b"juego").unwrap();
        let b = m.abrir(b"musica").unwrap();
        let mut acc = [0i32; 4];
        m.echar(a, &[1000, 1000, 1000, 1000], &mut acc);
        m.echar(b, &[500, -500, 500, -500], &mut acc);
        assert_eq!(acc, [1500, 500, 1500, 500]);
    }

    #[test]
    fn muda_calla_esa_y_solo_calla_las_otras() {
        let mut m = mesa();
        let juego = m.abrir(b"juego").unwrap();
        let musica = m.abrir(b"musica").unwrap();
        let voz = m.abrir(b"voz").unwrap();

        // Mudo: calla la suya y nada mas.
        m.pista_mut(musica).unwrap().muda = true;
        assert!(m.suena(juego) && !m.suena(musica) && m.suena(voz));

        // Solo: calla TODAS las demas, incluida una que no estaba muda.
        m.pista_mut(musica).unwrap().muda = false;
        m.pista_mut(voz).unwrap().solo = true;
        assert!(!m.suena(juego), "el solo de `voz` tenia que callar a `juego`");
        assert!(!m.suena(musica));
        assert!(m.suena(voz));

        // Y muda gana a solo sobre la MISMA pista: si la callas, calla.
        m.pista_mut(voz).unwrap().muda = true;
        assert!(!m.suena(voz));
    }

    #[test]
    fn una_pista_callada_no_ensucia_el_acumulador_ni_su_medidor() {
        let mut m = mesa();
        let i = m.abrir(b"juego").unwrap();
        m.pista_mut(i).unwrap().muda = true;
        let mut acc = [7i32; 3];
        m.echar(i, &[30_000, 30_000, 30_000], &mut acc);
        assert_eq!(acc, [7, 7, 7], "una pista muda toco el acumulador");
        assert_eq!(m.pista(i).unwrap().medidor.pico(), 0, "y ademas mostraba barras");
    }

    #[test]
    fn el_medidor_de_la_pista_dice_quien_se_pasa() {
        // Es la regla 3, y la razon de que la mesa sirva para algo: el maestro
        // solo sabe que la suma se paso; la pista dice cual fue.
        let mut m = mesa();
        let flojo = m.abrir(b"musica").unwrap();
        let fuerte = m.abrir(b"juego").unwrap();
        let mut acc = [0i32; 64];
        m.echar(flojo, &[300; 64], &mut acc);
        m.echar(fuerte, &[30_000; 64], &mut acc);
        assert_eq!(m.pista(flojo).unwrap().medidor.pico(), 300);
        assert_eq!(m.pista(fuerte).unwrap().medidor.pico(), 30_000);
        assert!(m.pista(fuerte).unwrap().medidor.pico_dbfs() > m.pista(flojo).unwrap().medidor.pico_dbfs());
    }

    #[test]
    fn la_ganancia_de_la_pista_entra_en_su_medidor() {
        // Lo que el medidor de la pista tiene que decir es lo que esa pista
        // APORTA, no lo que traia: con +6 dB, el doble.
        let mut m = mesa();
        let i = m.abrir(b"juego").unwrap();
        m.subir(i, 6 * DB);
        let mut acc = [0i32; 8];
        m.echar(i, &[10_000; 8], &mut acc);
        assert_eq!(m.pista(i).unwrap().medidor.pico(), 19_952);
        assert_eq!(acc[0], 19_952);
    }

    #[test]
    fn cuatro_pistas_a_tope_no_rompen_nada() {
        // El caso que rompe un mezclador casero: cuatro fuentes fuertes a la
        // vez. El acumulador de 32 bits las aguanta enteras y el maestro
        // decide; lo que sale cabe siempre.
        let mut m = mesa();
        let mut acc = [0i32; 256];
        for k in 0..4 {
            let n = [b'p', b'0' + k];
            let i = m.abrir(&n).unwrap();
            let onda: [i16; 256] = core::array::from_fn(|j| if (j / 8) % 2 == 0 { 30_000 } else { -30_000 });
            m.echar(i, &onda, &mut acc);
        }
        assert_eq!(acc[0], 120_000, "la suma tiene que estar ENTERA antes del maestro");
        let mut salida = [0i16; 256];
        m.maestro(&acc, &mut salida);
        for (j, &y) in salida.iter().enumerate() {
            assert!(y as i32 <= PLENO && (y as i32) >= -PLENO, "muestra {} salio {}", j, y);
        }
        // Y el maestro CUENTA lo que tuvo que sujetar: no calla.
        assert!(m.maestro.limite.sujetadas() > 0);
    }

    #[test]
    fn una_pista_cerrada_vuelve_limpia() {
        let mut m = mesa();
        let i = m.abrir(b"juego").unwrap();
        m.subir(i, -20 * DB);
        m.pista_mut(i).unwrap().muda = true;
        m.cerrar(i);
        let j = m.abrir(b"juego").unwrap();
        assert_eq!(j, i);
        assert_eq!(m.pista(j).unwrap().ganancia, Ganancia::UNIDAD);
        assert!(!m.pista(j).unwrap().muda);
    }

    #[test]
    fn la_mesa_entera_con_el_maestro_subido() {
        // El caso del propietario: tres pistas flojas y +12 dB en el maestro.
        let mut m = mesa();
        for nombre in [b"juego".as_slice(), b"musica", b"sistema"] {
            let i = m.abrir(nombre).unwrap();
            let mut acc = [0i32; 0];
            let _ = &mut acc;
            let _ = i;
        }
        let mut acc = [0i32; 480];
        for k in 0..3 {
            let i = k;
            let onda: [i16; 480] = core::array::from_fn(|j| if (j / 24) % 2 == 0 { 1_000 } else { -1_000 });
            m.echar(i, &onda, &mut acc);
        }
        // Tres a 1.000 son 3.000: -20,8 dBFS. Con +12 dB, -8,8.
        assert_eq!(acc[0], 3_000);
        m.maestro.subir(12 * DB);
        let mut salida = [0i16; 480];
        m.maestro(&acc, &mut salida);
        let pico = m.maestro.medidor.pico_dbfs();
        assert!((-2_400..=-2_000).contains(&pico), "quedo en {} (1/256 dB)", pico);
        assert_eq!(m.maestro.limite.dobladas(), 0, "doblego sin necesidad");
        // Y la cuenta cuadra con lo que dice el medidor.
        assert_eq!(a_dbfs(m.maestro.medidor.pico()), pico);
    }

    #[test]
    fn olvidar_medidas_no_borra_las_cuentas_del_maestro() {
        let mut m = mesa();
        let i = m.abrir(b"juego").unwrap();
        let mut acc = [0i32; 64];
        m.echar(i, &[200_000i32 as i16; 64], &mut acc);
        let mut salida = [0i16; 64];
        m.maestro(&acc, &mut salida);
        let sujetadas = m.maestro.limite.sujetadas();
        m.olvidar_medidas();
        assert_eq!(m.pista(i).unwrap().medidor.muestras(), 0);
        assert_eq!(m.maestro.limite.sujetadas(), sujetadas, "las del maestro son de la sesion");
    }

    #[test]
    fn con_listas_vacias_y_pistas_que_no_existen_no_explota() {
        let mut m = mesa();
        let mut acc = [0i32; 4];
        m.echar(99, &[1, 2, 3], &mut acc);
        m.echar(0, &[1, 2, 3], &mut acc);
        assert_eq!(acc, [0; 4], "una pista que no esta abierta no suena");
        assert_eq!(m.subir(99, 0), None);
        assert!(m.pista(99).is_none());
        let i = m.abrir(b"x").unwrap();
        m.echar(i, &[], &mut acc);
        m.echar(i, &[1], &mut []);
        m.maestro(&[], &mut []);
    }
}
