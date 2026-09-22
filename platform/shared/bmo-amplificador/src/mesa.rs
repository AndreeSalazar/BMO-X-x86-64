//! **LA MESA: un ARBOL de dos niveles, y un maestro.**
//!
//! El paso M0 de `docs/plan/PLAN_LA_MESA.md`. El propietario, el 2026-09-22:
//! *"no solo voz, sistema, eso; sino que dentro del contenido: el juego EXPONE
//! TODO el audio que ofrece, en subcategorias, y lo mismo todos. Uno simple
//! MAESTRO aplica todo, pero si quieres control total se pueda"*.
//!
//! # Dos niveles, ni uno ni tres
//!
//! ```text
//!    GRUPO  juego      [ganancia][M][S][medidor]
//!      |-- PISTA  armas      [ganancia][M][S][medidor]
//!      |-- PISTA  pasos      [ganancia][M][S][medidor]
//!      |-- PISTA  musica     [ganancia][M][S][medidor]
//!      '-- PISTA  voces      [ganancia][M][S][medidor]
//!    GRUPO  sistema    [ganancia][M][S][medidor]
//!      '-- PISTA  (sin nombre: el grupo entero)
//!                                                       todo -> [MAESTRO]
//! ```
//!
//! **UNO** no basta: un juego que solo puede decir "juego" obliga a bajarlo
//! entero cuando lo unico que molestaba eran las armas. **TRES** ya es un DAW:
//! grupos dentro de grupos es jerarquia sin fin, y este repo tiene una regla
//! contra la esencia sin acotar. Dos es lo que tiene una mesa de verdad
//! --buses y canales-- y es lo que se puede terminar.
//!
//! *** Y LA LEY DEL ARBOL ES LA MISMA QUE LA DE LOS FORMATOS DEL APARATO: **lo
//! declara quien lo tiene, no lo adivina quien lo pinta.** El aparato escribe
//! su tabla de formatos (S0 de `PLAN_EL_SONIDO.md`) y el programa escribe su
//! arbol de pistas. La mesa no supone que un juego tiene "efectos y musica":
//! muestra lo que el juego EXPUSO, sea eso o sean diecisiete cosas.
//!
//! # El camino de una muestra, y donde mide cada quien
//!
//! ```text
//!    muestra
//!      x ganancia de la PISTA   -> el medidor de la pista ve ESTO
//!      sumada en el BUS del grupo
//!      x ganancia del GRUPO     -> el medidor del grupo ve el BUS ENTERO
//!      sumada en el acumulador
//!      x ganancia del MAESTRO, y su limite  -> el medidor del maestro
//!      -> 16 bits, al aparato
//! ```
//!
//! Los tres medidores dicen cosas distintas a proposito: el del maestro dice
//! *si te pasas*, el del grupo dice *que grupo*, y el de la pista dice *cual
//! de sus pistas*. Un solo medidor al final no puede contestar las dos
//! ultimas, y ese es todo el motivo del arbol.
//!
//! # Como se usa (un bus reutilizado, sin asignador)
//!
//! ```text
//!    para cada grupo g:
//!        bus.fill(0)
//!        para cada pista p de g:   mesa.echar(p, sus_muestras, &mut bus)
//!        mesa.sumar_grupo(g, &bus, &mut acumulador)
//!    mesa.maestro(&acumulador, &mut salida)
//! ```
//!
//! UN solo `bus` para todos los grupos, porque se vacia entre uno y otro: en
//! un crate sin asignador, los bufers los pone quien llama.
//!
//! # Lo que NO hace
//!
//! No abre nada, no habla con el aparato y no sabe de ficheros: entran
//! muestras y sale un bloque. Quien las trae y donde va lo que sale es del
//! productor (M1), y derivar una pista a fichero es M4.

use crate::{sumar, sumar_32, Amplificador, Ganancia, Medidor, MilesimasDb};

/// Cuantos grupos caben: `juego`, `musica`, `sistema`, `voz`, `otros` y sitio.
pub const GRUPOS: usize = 8;

/// Cuantas pistas caben EN TOTAL, repartidas entre los grupos. Dieciseis deja
/// que un juego exponga ocho subcategorias y siga habiendo sitio para todo lo
/// demas.
pub const PISTAS: usize = 16;

/// Lo que cabe en un nombre. Doce caben en una columna de la ventana sin
/// partirse.
pub const NOMBRE: usize = 12;

/// Un nombre corto, guardado sin asignador.
#[derive(Clone, Copy, Debug)]
struct Nombre {
    letras: [u8; NOMBRE],
    n: u8,
}

impl Nombre {
    const VACIO: Nombre = Nombre { letras: [0; NOMBRE], n: 0 };

    fn nuevo(de: &[u8]) -> Nombre {
        let n = de.len().min(NOMBRE);
        let mut x = Nombre::VACIO;
        x.letras[..n].copy_from_slice(&de[..n]);
        x.n = n as u8;
        x
    }

    fn como_bytes(&self) -> &[u8] {
        &self.letras[..self.n as usize]
    }

    fn es(&self, otro: &[u8]) -> bool {
        self.como_bytes() == &otro[..otro.len().min(NOMBRE)]
    }
}

/// Las perillas que tienen igual un grupo y una pista. Se dicen una vez para
/// que las dos filas de la ventana se puedan pintar con el mismo codigo.
#[derive(Clone, Copy, Debug)]
pub struct Mando {
    pub ganancia: Ganancia,
    /// Callado a mano.
    pub mudo: bool,
    /// En SOLO. Ver [`Mesa::suena`].
    pub solo: bool,
    /// Lo que paso por aqui, ya con su ganancia.
    pub medidor: Medidor,
}

impl Mando {
    pub const NUEVO: Mando = Mando {
        ganancia: Ganancia::UNIDAD,
        mudo: false,
        solo: false,
        medidor: Medidor::nuevo(),
    };
}

/// **Un grupo: `juego`, `sistema`, `musica`...** Lo abre el primer programa
/// que dice ese nombre.
#[derive(Clone, Copy, Debug)]
pub struct Grupo {
    nombre: Nombre,
    abierto: bool,
    pub mando: Mando,
}

impl Grupo {
    const VACIO: Grupo = Grupo { nombre: Nombre::VACIO, abierto: false, mando: Mando::NUEVO };

    pub fn nombre(&self) -> &[u8] {
        self.nombre.como_bytes()
    }
    pub fn abierto(&self) -> bool {
        self.abierto
    }
}

/// **Una pista: una subcategoria dentro de un grupo.** Un nombre vacio
/// significa *"el grupo entero"*, que es lo que abre un programa que no tiene
/// subcategorias que exponer.
#[derive(Clone, Copy, Debug)]
pub struct Pista {
    nombre: Nombre,
    abierta: bool,
    grupo: u8,
    pub mando: Mando,
}

impl Pista {
    const VACIA: Pista = Pista {
        nombre: Nombre::VACIO,
        abierta: false,
        grupo: 0,
        mando: Mando::NUEVO,
    };

    pub fn nombre(&self) -> &[u8] {
        self.nombre.como_bytes()
    }
    pub fn abierta(&self) -> bool {
        self.abierta
    }
    /// A que grupo pertenece.
    pub fn grupo(&self) -> usize {
        self.grupo as usize
    }
    /// Es la pista "el grupo entero" (sin subcategoria)?
    pub fn es_el_grupo(&self) -> bool {
        self.nombre.n == 0
    }
}

/// **La mesa entera: grupos, pistas y el maestro.**
#[derive(Clone, Copy, Debug)]
pub struct Mesa {
    grupos: [Grupo; GRUPOS],
    pistas: [Pista; PISTAS],
    /// La ganancia, el limite y el medidor de lo que sale al aparato. **Es la
    /// perilla "simple" que el propietario pidio: mover esta aplica a todo.**
    pub maestro: Amplificador,
}

impl Mesa {
    /// Una mesa para un aparato de `hz`, vacia.
    pub fn nueva(hz: u32) -> Mesa {
        Mesa {
            grupos: [Grupo::VACIO; GRUPOS],
            pistas: [Pista::VACIA; PISTAS],
            maestro: Amplificador::nuevo(hz),
        }
    }

    // -- Abrir y buscar ---------------------------------------------------

    /// **Abrir (o encontrar) un grupo por nombre.**
    ///
    /// `None` = no quedan. No se roba uno ajeno para hacer sitio: la respuesta
    /// correcta a "no cabes" es decirlo.
    pub fn abrir_grupo(&mut self, nombre: &[u8]) -> Option<usize> {
        if let Some(i) = self.buscar_grupo(nombre) {
            return Some(i);
        }
        let libre = self.grupos.iter().position(|g| !g.abierto)?;
        self.grupos[libre] =
            Grupo { nombre: Nombre::nuevo(nombre), abierto: true, mando: Mando::NUEVO };
        Some(libre)
    }

    /// **Abrir (o encontrar) una pista dentro de un grupo**, creando el grupo
    /// si hace falta. Un `pista` vacio es *"el grupo entero"*.
    ///
    /// Esta es la operacion con la que un programa EXPONE su audio:
    /// `abrir(b"juego", b"armas")`, `abrir(b"juego", b"pasos")`...
    pub fn abrir(&mut self, grupo: &[u8], pista: &[u8]) -> Option<usize> {
        let g = self.abrir_grupo(grupo)?;
        if let Some(i) = self.buscar(g, pista) {
            return Some(i);
        }
        let libre = self.pistas.iter().position(|p| !p.abierta)?;
        self.pistas[libre] = Pista {
            nombre: Nombre::nuevo(pista),
            abierta: true,
            grupo: g as u8,
            mando: Mando::NUEVO,
        };
        Some(libre)
    }

    pub fn buscar_grupo(&self, nombre: &[u8]) -> Option<usize> {
        self.grupos.iter().position(|g| g.abierto && g.nombre.es(nombre))
    }

    /// La pista `nombre` dentro del grupo `g`.
    pub fn buscar(&self, g: usize, nombre: &[u8]) -> Option<usize> {
        self.pistas
            .iter()
            .position(|p| p.abierta && p.grupo as usize == g && p.nombre.es(nombre))
    }

    /// Cerrar una pista. Sus perillas se olvidan.
    pub fn cerrar(&mut self, i: usize) {
        if i < PISTAS {
            self.pistas[i] = Pista::VACIA;
        }
    }

    /// Cerrar un grupo **y todas sus pistas**.
    pub fn cerrar_grupo(&mut self, g: usize) {
        if g >= GRUPOS {
            return;
        }
        for p in self.pistas.iter_mut() {
            if p.abierta && p.grupo as usize == g {
                *p = Pista::VACIA;
            }
        }
        self.grupos[g] = Grupo::VACIO;
    }

    // -- Mirar -------------------------------------------------------------

    pub fn grupo(&self, g: usize) -> Option<&Grupo> {
        self.grupos.get(g).filter(|x| x.abierto)
    }
    pub fn grupo_mut(&mut self, g: usize) -> Option<&mut Grupo> {
        self.grupos.get_mut(g).filter(|x| x.abierto)
    }
    pub fn pista(&self, i: usize) -> Option<&Pista> {
        self.pistas.get(i).filter(|p| p.abierta)
    }
    pub fn pista_mut(&mut self, i: usize) -> Option<&mut Pista> {
        self.pistas.get_mut(i).filter(|p| p.abierta)
    }
    pub fn grupos_abiertos(&self) -> usize {
        self.grupos.iter().filter(|g| g.abierto).count()
    }
    pub fn pistas_abiertas(&self) -> usize {
        self.pistas.iter().filter(|p| p.abierta).count()
    }
    /// Cuantas pistas cuelgan de ese grupo. Es lo que la ventana necesita para
    /// saber cuantas columnas pintar debajo.
    pub fn pistas_de(&self, g: usize) -> usize {
        self.pistas.iter().filter(|p| p.abierta && p.grupo as usize == g).count()
    }

    // -- Mudo y solo -------------------------------------------------------

    /// **Suena esta pista ahora mismo?** La regla entera, en un sitio, para
    /// que la ventana y la mezcla contesten siempre lo mismo:
    ///
    /// ```text
    ///    si la pista esta muda            -> no
    ///    si su grupo esta mudo            -> no
    ///    si hay alguna PISTA en solo      -> solo suenan esas
    ///    si no, y hay algun GRUPO en solo -> solo suenan las de esos grupos
    ///    si no                            -> si
    /// ```
    ///
    /// El solo de pista gana al de grupo a proposito: es lo mas fino que se
    /// puede pedir, y quien pulsa el solo de una pista quiere OIR ESA.
    pub fn suena(&self, i: usize) -> bool {
        let Some(p) = self.pista(i) else { return false };
        if p.mando.mudo {
            return false;
        }
        let Some(g) = self.grupo(p.grupo as usize) else { return false };
        if g.mando.mudo {
            return false;
        }
        if self.hay_solo_de_pista() {
            return p.mando.solo;
        }
        if self.hay_solo_de_grupo() {
            return g.mando.solo;
        }
        true
    }

    pub fn hay_solo_de_pista(&self) -> bool {
        self.pistas.iter().any(|p| p.abierta && p.mando.solo)
    }
    pub fn hay_solo_de_grupo(&self) -> bool {
        self.grupos.iter().any(|g| g.abierto && g.mando.solo)
    }

    /// Poner la ganancia de una pista, en 1/256 de dB.
    pub fn subir(&mut self, i: usize, db: MilesimasDb) -> Option<Ganancia> {
        let g = Ganancia::db(db);
        self.pista_mut(i)?.mando.ganancia = g;
        Some(g)
    }

    /// Poner la ganancia de un grupo entero.
    pub fn subir_grupo(&mut self, gi: usize, db: MilesimasDb) -> Option<Ganancia> {
        let g = Ganancia::db(db);
        self.grupo_mut(gi)?.mando.ganancia = g;
        Some(g)
    }

    // -- Mezclar -----------------------------------------------------------

    /// **Echar las muestras de una pista al BUS DE SU GRUPO**, con su ganancia
    /// y su medidor.
    ///
    /// Si la pista no suena no toca el bus **y su medidor se queda a cero**:
    /// una pista callada que mostrara barras seria un instrumento que miente.
    pub fn echar(&mut self, i: usize, muestras: &[i16], bus: &mut [i32]) {
        if !self.suena(i) {
            return;
        }
        let Some(p) = self.pista_mut(i) else { return };
        let g = p.mando.ganancia;
        let n = muestras.len().min(bus.len());
        // El medidor de la pista ve lo que ELLA aporta: por eso se mide aqui,
        // y no en el bus, donde ya esta sumada con sus hermanas.
        for &m in &muestras[..n] {
            p.mando.medidor.mirar_uno(g.aplicar(m as i32));
        }
        sumar(bus, &muestras[..n], g);
    }

    /// **El bus de un grupo al acumulador**, con la ganancia del grupo y su
    /// medidor. Se llama una vez por grupo, con el bus ya lleno.
    ///
    /// El medidor del grupo ve **el bus entero**, que es el unico sitio donde
    /// se sabe lo que ese grupo suena de verdad: la suma de sus pistas puede
    /// ser mas alta que cualquiera de ellas.
    pub fn sumar_grupo(&mut self, gi: usize, bus: &[i32], acumulador: &mut [i32]) {
        let Some(g) = self.grupo_mut(gi) else { return };
        if g.mando.mudo {
            return;
        }
        let gan = g.mando.ganancia;
        let n = bus.len().min(acumulador.len());
        for &x in &bus[..n] {
            g.mando.medidor.mirar_uno(gan.aplicar(x));
        }
        sumar_32(acumulador, &bus[..n], gan);
    }

    /// **El maestro: del acumulador a lo que sale al aparato.** La perilla
    /// simple: ganancia, limite y medidor en una pasada.
    pub fn maestro(&mut self, acumulador: &[i32], salida: &mut [i16]) {
        self.maestro.bloque(acumulador, salida);
    }

    /// Los medidores de pistas y grupos a cero, para el tramo siguiente. Las
    /// cuentas del maestro (`sujetadas`, `dobladas`) NO se tocan: son de la
    /// sesion entera y decirlas por tramos las haria inutiles.
    pub fn olvidar_medidas(&mut self) {
        for p in self.pistas.iter_mut() {
            p.mando.medidor.olvidar();
        }
        for g in self.grupos.iter_mut() {
            g.mando.medidor.olvidar();
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

    /// Un juego que EXPONE cuatro subcategorias, que es el caso que abrio
    /// este modelo.
    fn juego_expuesto(m: &mut Mesa) -> [usize; 4] {
        [
            m.abrir(b"juego", b"armas").unwrap(),
            m.abrir(b"juego", b"pasos").unwrap(),
            m.abrir(b"juego", b"musica").unwrap(),
            m.abrir(b"juego", b"voces").unwrap(),
        ]
    }

    #[test]
    fn un_juego_expone_sus_subcategorias_en_un_solo_grupo() {
        let mut m = mesa();
        let p = juego_expuesto(&mut m);
        assert_eq!(m.grupos_abiertos(), 1, "cuatro pistas, UN grupo");
        assert_eq!(m.pistas_abiertas(), 4);
        let g = m.buscar_grupo(b"juego").unwrap();
        assert_eq!(m.pistas_de(g), 4);
        for i in p {
            assert_eq!(m.pista(i).unwrap().grupo(), g);
        }
        assert_eq!(m.pista(p[0]).unwrap().nombre(), b"armas");
        assert!(!m.pista(p[0]).unwrap().es_el_grupo());
    }

    #[test]
    fn un_programa_sin_subcategorias_abre_el_grupo_entero() {
        let mut m = mesa();
        let i = m.abrir(b"musica", b"").unwrap();
        assert!(m.pista(i).unwrap().es_el_grupo());
        assert_eq!(m.pistas_de(m.buscar_grupo(b"musica").unwrap()), 1);
    }

    #[test]
    fn abrir_dos_veces_lo_mismo_da_lo_mismo() {
        let mut m = mesa();
        let a = m.abrir(b"juego", b"armas").unwrap();
        let b = m.abrir(b"juego", b"armas").unwrap();
        assert_eq!(a, b);
        assert_eq!(m.pistas_abiertas(), 1);
        // Pero el mismo nombre en OTRO grupo es otra pista.
        let c = m.abrir(b"video", b"armas").unwrap();
        assert_ne!(a, c);
        assert_eq!(m.grupos_abiertos(), 2);
    }

    #[test]
    fn cuando_no_caben_mas_se_dice_y_no_se_roba_ninguna() {
        let mut m = mesa();
        for i in 0..PISTAS {
            let n = [b'p', b'0' + (i % 10) as u8, b'a' + (i / 10) as u8];
            assert!(m.abrir(b"g", &n).is_some(), "no cupo la {}", i);
        }
        assert_eq!(m.abrir(b"g", b"una-mas"), None);
        assert_eq!(m.pistas_abiertas(), PISTAS);
        // Y los grupos igual.
        let mut m = mesa();
        for i in 0..GRUPOS {
            let n = [b'g', b'0' + i as u8];
            assert!(m.abrir_grupo(&n).is_some());
        }
        assert_eq!(m.abrir_grupo(b"otro"), None);
        assert_eq!(m.abrir(b"otro", b"x"), None, "sin grupo no hay pista");
    }

    #[test]
    fn la_ganancia_de_la_pista_y_la_del_grupo_se_multiplican() {
        // El caso de "control total": bajar solo las armas, o bajar el juego
        // entero, o las dos cosas.
        let mut m = mesa();
        let armas = m.abrir(b"juego", b"armas").unwrap();
        let g = m.buscar_grupo(b"juego").unwrap();
        m.subir(armas, -6 * DB);
        m.subir_grupo(g, -6 * DB);
        let mut bus = [0i32; 4];
        let mut acc = [0i32; 4];
        m.echar(armas, &[16_000; 4], &mut bus);
        // Tras la pista: -6 dB (x0,5012).
        assert_eq!(bus[0], 8_018);
        m.sumar_grupo(g, &bus, &mut acc);
        // Y tras el grupo, -12 dB en total: la cuarta parte.
        assert_eq!(acc[0], 4_018);
    }

    #[test]
    fn los_tres_medidores_dicen_cosas_distintas() {
        // El del maestro dice SI te pasas; el del grupo, QUE grupo; el de la
        // pista, CUAL de sus pistas. Un solo medidor no contesta las dos
        // ultimas, y ese es todo el motivo del arbol.
        let mut m = mesa();
        let armas = m.abrir(b"juego", b"armas").unwrap();
        let pasos = m.abrir(b"juego", b"pasos").unwrap();
        let g = m.buscar_grupo(b"juego").unwrap();
        let mut bus = [0i32; 16];
        let mut acc = [0i32; 16];
        m.echar(armas, &[20_000; 16], &mut bus);
        m.echar(pasos, &[3_000; 16], &mut bus);
        m.sumar_grupo(g, &bus, &mut acc);
        let mut salida = [0i16; 16];
        m.maestro(&acc, &mut salida);

        assert_eq!(m.pista(armas).unwrap().mando.medidor.pico(), 20_000);
        assert_eq!(m.pista(pasos).unwrap().mando.medidor.pico(), 3_000);
        // El del grupo ve la SUMA, que es mas alta que cualquiera de las dos.
        assert_eq!(m.grupo(g).unwrap().mando.medidor.pico(), 23_000);
        assert_eq!(m.maestro.medidor.pico(), 23_000);
    }

    #[test]
    fn mudo_de_grupo_calla_todas_sus_pistas() {
        let mut m = mesa();
        let p = juego_expuesto(&mut m);
        let otra = m.abrir(b"sistema", b"").unwrap();
        let g = m.buscar_grupo(b"juego").unwrap();
        m.grupo_mut(g).unwrap().mando.mudo = true;
        for i in p {
            assert!(!m.suena(i), "el mudo del grupo no callo la pista {}", i);
        }
        assert!(m.suena(otra), "y no tenia que tocar a las de otro grupo");
    }

    #[test]
    fn el_solo_de_una_pista_gana_al_de_un_grupo() {
        let mut m = mesa();
        let p = juego_expuesto(&mut m);
        let sis = m.abrir(b"sistema", b"").unwrap();
        let g_sis = m.buscar_grupo(b"sistema").unwrap();

        // Solo de grupo: suena el grupo entero y nada mas.
        m.grupo_mut(g_sis).unwrap().mando.solo = true;
        assert!(m.suena(sis));
        for i in p {
            assert!(!m.suena(i));
        }

        // Y ahora ademas el solo de UNA pista del otro grupo: gana el fino.
        m.pista_mut(p[0]).unwrap().mando.solo = true;
        assert!(m.suena(p[0]), "el solo de la pista tenia que ganar");
        assert!(!m.suena(p[1]));
        assert!(!m.suena(sis), "el solo de grupo ya no manda");
    }

    #[test]
    fn una_pista_callada_no_ensucia_el_bus_ni_su_medidor() {
        let mut m = mesa();
        let i = m.abrir(b"juego", b"armas").unwrap();
        m.pista_mut(i).unwrap().mando.mudo = true;
        let mut bus = [7i32; 3];
        m.echar(i, &[30_000; 3], &mut bus);
        assert_eq!(bus, [7, 7, 7]);
        assert_eq!(m.pista(i).unwrap().mando.medidor.pico(), 0);
    }

    #[test]
    fn un_grupo_mudo_no_suma_al_acumulador() {
        let mut m = mesa();
        let i = m.abrir(b"juego", b"armas").unwrap();
        let g = m.buscar_grupo(b"juego").unwrap();
        let mut bus = [0i32; 4];
        m.echar(i, &[10_000; 4], &mut bus);
        m.grupo_mut(g).unwrap().mando.mudo = true;
        let mut acc = [0i32; 4];
        m.sumar_grupo(g, &bus, &mut acc);
        assert_eq!(acc, [0; 4]);
        assert_eq!(m.grupo(g).unwrap().mando.medidor.pico(), 0);
    }

    #[test]
    fn el_maestro_es_la_perilla_simple_y_aplica_a_todo() {
        // Lo que el propietario pidio: "uno simple MAESTRO aplica todo".
        let mut m = mesa();
        let a = m.abrir(b"juego", b"armas").unwrap();
        let b = m.abrir(b"musica", b"").unwrap();
        let ga = m.buscar_grupo(b"juego").unwrap();
        let gb = m.buscar_grupo(b"musica").unwrap();
        let mut acc = [0i32; 48];
        let mut bus = [0i32; 48];
        m.echar(a, &[2_000; 48], &mut bus);
        m.sumar_grupo(ga, &bus, &mut acc);
        bus = [0; 48];
        m.echar(b, &[2_000; 48], &mut bus);
        m.sumar_grupo(gb, &bus, &mut acc);
        assert_eq!(acc[0], 4_000);
        m.maestro.subir(12 * DB);
        let mut salida = [0i16; 48];
        m.maestro(&acc, &mut salida);
        // 4.000 x 3,98 = 15.925, y sin recortar nada.
        assert!((15_800..=16_050).contains(&(salida[0] as i32)), "salio {}", salida[0]);
        assert_eq!(m.maestro.limite.dobladas(), 0);
        assert_eq!(a_dbfs(m.maestro.medidor.pico()), m.maestro.medidor.pico_dbfs());
    }

    #[test]
    fn ocho_pistas_a_tope_en_dos_grupos_no_rompen_nada() {
        let mut m = mesa();
        let mut acc = [0i32; 64];
        for gi in 0..2u8 {
            let mut bus = [0i32; 64];
            let gn = [b'g', b'0' + gi];
            for k in 0..4u8 {
                let pn = [b'p', b'0' + k];
                let i = m.abrir(&gn, &pn).unwrap();
                let onda: [i16; 64] =
                    core::array::from_fn(|j| if (j / 4) % 2 == 0 { 30_000 } else { -30_000 });
                m.echar(i, &onda, &mut bus);
            }
            let g = m.buscar_grupo(&gn).unwrap();
            m.sumar_grupo(g, &bus, &mut acc);
        }
        assert_eq!(acc[0], 240_000, "la suma tiene que llegar ENTERA al maestro");
        let mut salida = [0i16; 64];
        m.maestro(&acc, &mut salida);
        for (j, &y) in salida.iter().enumerate() {
            assert!((y as i32).abs() <= PLENO, "muestra {} salio {}", j, y);
        }
        assert!(m.maestro.limite.sujetadas() > 0, "sujeto y no lo conto");
    }

    #[test]
    fn cerrar_un_grupo_se_lleva_sus_pistas() {
        let mut m = mesa();
        let p = juego_expuesto(&mut m);
        let otra = m.abrir(b"sistema", b"").unwrap();
        let g = m.buscar_grupo(b"juego").unwrap();
        m.cerrar_grupo(g);
        assert_eq!(m.pistas_abiertas(), 1, "las cuatro del juego tenian que irse");
        for i in p {
            assert!(m.pista(i).is_none());
        }
        assert!(m.pista(otra).is_some());
        // Y al volver, viene limpio.
        let v = m.abrir(b"juego", b"armas").unwrap();
        assert_eq!(m.pista(v).unwrap().mando.ganancia, Ganancia::UNIDAD);
    }

    #[test]
    fn olvidar_medidas_no_borra_las_cuentas_del_maestro() {
        let mut m = mesa();
        let i = m.abrir(b"juego", b"armas").unwrap();
        let g = m.buscar_grupo(b"juego").unwrap();
        let mut bus = [0i32; 64];
        let mut acc = [0i32; 64];
        m.echar(i, &[30_000; 64], &mut bus);
        m.sumar_grupo(g, &bus, &mut acc);
        m.maestro.subir(20 * DB);
        let mut salida = [0i16; 64];
        m.maestro(&acc, &mut salida);
        let sujetadas = m.maestro.limite.sujetadas();
        assert!(sujetadas > 0);
        m.olvidar_medidas();
        assert_eq!(m.pista(i).unwrap().mando.medidor.muestras(), 0);
        assert_eq!(m.grupo(g).unwrap().mando.medidor.muestras(), 0);
        assert_eq!(m.maestro.limite.sujetadas(), sujetadas);
    }

    #[test]
    fn con_indices_absurdos_y_listas_vacias_no_explota() {
        let mut m = mesa();
        let mut bus = [0i32; 4];
        let mut acc = [0i32; 4];
        m.echar(99, &[1, 2, 3], &mut bus);
        m.sumar_grupo(99, &bus, &mut acc);
        assert_eq!(acc, [0; 4]);
        assert_eq!(m.subir(99, 0), None);
        assert_eq!(m.subir_grupo(99, 0), None);
        assert!(m.pista(99).is_none() && m.grupo(99).is_none());
        m.cerrar(99);
        m.cerrar_grupo(99);
        let i = m.abrir(b"g", b"p").unwrap();
        m.echar(i, &[], &mut bus);
        m.echar(i, &[1], &mut []);
        m.maestro(&[], &mut []);
        // Un nombre larguisimo se recorta y se sigue encontrando.
        let j = m.abrir(b"un-grupo-larguisimo", b"una-pista-larguisima").unwrap();
        assert_eq!(m.pista(j).unwrap().nombre().len(), NOMBRE);
        assert!(m.buscar_grupo(b"un-grupo-lar").is_some());
    }
}
