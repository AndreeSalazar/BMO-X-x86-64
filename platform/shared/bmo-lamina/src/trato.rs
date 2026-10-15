//! **EL TRATO** -- lo que cumple todo bloque que un lado escribe y otro lee sin
//! cerrojo: la cabecera comun, el SELLO por ranura y las DOS ranuras.
//!
//! [carril]  VERDE     aritmetica sobre palabras atomicas que da quien llama
//! [consumo] NADA      se usa cuando alguien publica o lee
//!
//! # LA LEY: la palabra 0. Y la forma completa, para las que nacen ahora
//!
//! Lo que cumplen las OCHO clases del censo es una sola cosa: **la palabra 0
//! es su magia**. La palabra 1 cada una la puso a su manera antes de que
//! hubiera trato (una version entera, media, una medida, la mitad alta de una
//! magia de 64 bits) y no se mueve: el censo dice cual es
//! (`censo::Version`) y `censo::reconocer` la juzga.
//!
//! La FORMA COMPLETA -- la de BVER, y la de toda lamina que nazca desde hoy
//! (A4) -- son tres palabras:
//!
//! ```text
//!    palabra  campo           quien la escribe
//!    0        MAGIA           el propietario, al crearla: dice la CLASE (censo)
//!    1        VERSION         el propietario, al crearla
//!    2        CABECERA        el propietario, al crearla: bytes antes de los datos
//!    3..      lo de la clase  (BVER: la capacidad, la secuencia, los sellos...)
//! ```
//!
//! # El sello y las dos ranuras (de `bmo_verrano::lamina`, 26-09)
//!
//! Un ESCRITOR y un LECTOR, y ninguno para al otro:
//!
//! ```text
//!    escribir   s = secuencia; k = (s + 1) % 2 -- la ranura que NO es la publicada
//!               sello k + 1 (IMPAR: "estoy escribiendo aqui")   [`empezar`]
//!               los datos de la ranura k
//!               sello k + 1 (PAR otra vez: "acabe")             [`acabar`]
//!               y AL FINAL secuencia = s + 1: la publicacion    [`publicar`]
//!    leer       s = secuencia (0 = aun nada); k = s % 2
//!               e1 = sello k; impar: el escritor esta dentro    [`mirar`]
//!               copiar
//!               e1 = sello k otra vez: nadie la toco            [`intacto`]
//!               secuencia = s todavia: no es uno POSTERIOR      [`vigente`]
//! ```
//!
//! [!] Mirar solo la SECUENCIA no basta: justo despues de publicar, el escritor
//! ya empieza la ranura que el lector esta copiando, y la secuencia no ha
//! cambiado todavia. La prueba de dos hilos de VERRANO lo cazo el 26-09; el
//! sello por ranura es el que lo ve, siempre.
//!
//! [!] Y el sello SOLO no basta tampoco (14-10, la prueba castigada de este
//! fichero, una de cada ~60 corridas): entre leer `s` y mirar el sello, el
//! escritor puede publicar `s + 1` (en la otra ranura) y escribir ENTERA la
//! ranura `k` con el siguiente, sin publicarlo aun. El sello es par y no se
//! mueve mientras se copia: lo copiado es un dato ENTERO, pero POSTERIOR a
//! `s`, y la vuelta siguiente lee `s + 1` -- uno mas viejo: el fotograma va
//! hacia atras --. Por eso, despues de copiar, la secuencia tiene que seguir
//! siendo `s` ([`vigente`]): el escritor solo vuelve a la ranura `k` despues
//! de publicar `s + 1`, y la barrera de [`intacto`] hace que, si se copio
//! algo suyo de despues, esa publicacion ya se vea.
//!
//! # La palabra de VUELTA: un lector que NO copia (C1, 15-10)
//!
//! La 3060 lee la ranura EN SU SITIO -- prestada por la IOMMU, sin copia -- y
//! no sabe mirar un sello. Asi que quien la manda (el escritorio) RESERVA la
//! ranura antes, en una palabra de la clase, y el escritor la RESPETA:
//!
//! ```text
//!    reservar   s = secuencia; k = s % 2                            [`reservar`]
//!               VUELTA = k + 1 ("la ranura k esta en vuelo")
//!               barrera SeqCst
//!               sello k par y secuencia = s todavia? si no: VUELTA = 0 y nada
//!               ... la 3060 lee la ranura k, el tiempo que haga falta ...
//!               VUELTA = 0 (tras la valla de la 3060)              [`soltar`]
//!    escribir   k = (s + 1) % 2; sello k + 1 (IMPAR)                [`empezar_si_libre`]
//!               barrera SeqCst
//!               VUELTA = k + 1? si: el sello como estaba y NO se escribe
//!               (ese fotograma se pierde: gana el ultimo, como siempre)
//! ```
//!
//! ** Por que no hay hueco: las dos barreras SeqCst. El lector ESCRIBE la
//! vuelta y luego LEE el sello; el escritor ESCRIBE el sello y luego LEE la
//! vuelta. Con una barrera SeqCst entre la escritura y la lectura de cada uno,
//! al menos uno ve lo del otro (es el de Dekker): o el lector ve el sello
//! impar y se aparta, o el escritor ve la vuelta y se aparta. Nunca los dos
//! dentro de la misma ranura.
//!
//! [!] Lo que el lector reserva puede ser un fotograma ENTERO pero POSTERIOR a
//! `s` (el escritor acabo la ranura `k` con el siguiente y aun no lo
//! publico): para quien dibuja no es medio dato ni va hacia atras -- es el que
//! iba a publicarse --, y por eso aqui basta con el sello y la secuencia.
//!
//! En x86-64 las escrituras se ven en orden y las lecturas tambien (TSO): un
//! escritor en INTI o TITAN++ solo tiene que ESCRIBIR en este orden, sin
//! barreras. Aqui las barreras se dicen para que valga en cualquier maquina.

use core::sync::atomic::{fence, AtomicU32, Ordering};

pub const CAMPO_MAGIA: usize = 0;
pub const CAMPO_VERSION: usize = 1;
pub const CAMPO_CABECERA: usize = 2;
/// Las palabras que el trato fija; lo de la clase empieza aqui.
pub const PALABRAS_COMUNES: usize = 3;

/// Por que un bloque no cumple la cabecera comun.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoEs {
    /// Menos bytes que la cabecera.
    Corta,
    /// La primera palabra no es la magia pedida: es otra clase, o nada.
    Magia,
    /// Otra version, u otra medida de cabecera.
    Version,
}

/// **Crear la cabecera** (el propietario, una vez): la cabecera entera a
/// cero y las tres palabras comunes. `false` si `cabecera` no cabe en el
/// bloque o no deja sitio para las tres palabras.
pub fn crear(w: &[AtomicU32], magia: u32, version: u32, cabecera: usize) -> bool {
    if cabecera < PALABRAS_COMUNES * 4 || cabecera % 4 != 0 || w.len() * 4 < cabecera {
        return false;
    }
    for x in &w[..cabecera / 4] {
        x.store(0, Ordering::Relaxed);
    }
    w[CAMPO_MAGIA].store(magia, Ordering::Relaxed);
    w[CAMPO_VERSION].store(version, Ordering::Relaxed);
    w[CAMPO_CABECERA].store(cabecera as u32, Ordering::Relaxed);
    true
}

/// **Abrir** (el que la toma): la cabecera comun contra lo prestado. NO se
/// cree nada mas; lo de la clase lo juzga la clase.
pub fn abrir(w: &[AtomicU32], magia: u32, version: u32, cabecera: usize) -> Result<(), NoEs> {
    if w.len() * 4 < cabecera || w.len() < PALABRAS_COMUNES {
        return Err(NoEs::Corta);
    }
    if w[CAMPO_MAGIA].load(Ordering::Acquire) != magia {
        return Err(NoEs::Magia);
    }
    if w[CAMPO_VERSION].load(Ordering::Relaxed) != version || w[CAMPO_CABECERA].load(Ordering::Relaxed) != cabecera as u32 {
        return Err(NoEs::Version);
    }
    Ok(())
}

// -- Las dos ranuras ----------------------------------------------------------

/// La ranura donde se ESCRIBE el siguiente, con la secuencia publicada `s`:
/// la que NO se esta leyendo.
pub const fn ranura_para_escribir(s: u32) -> usize {
    (s.wrapping_add(1) % 2) as usize
}

/// La ranura que se LEE con la secuencia publicada `s`.
pub const fn ranura_publicada(s: u32) -> usize {
    (s % 2) as usize
}

/// La secuencia que el escritor va a publicar (la suya, sin barrera: solo
/// el la escribe).
pub fn secuencia_propia(w: &[AtomicU32], campo: usize) -> u32 {
    w[campo].load(Ordering::Relaxed)
}

/// La SECUENCIA publicada (0 = aun nada). Mirarla cuesta UNA palabra.
pub fn secuencia(w: &[AtomicU32], campo: usize) -> u32 {
    w[campo].load(Ordering::Acquire)
}

/// **Publicar** (el escritor, LO ULTIMO): todo lo de antes se ve antes que
/// este numero.
pub fn publicar(w: &[AtomicU32], campo: usize, s: u32) {
    w[campo].store(s.wrapping_add(1), Ordering::Release);
}

// -- El sello -----------------------------------------------------------------

/// **Empezar a escribir una ranura** (el escritor): su sello pasa a IMPAR
/// antes de tocar nada de ella. Da el sello de antes, el que se pasa a
/// [`acabar`].
pub fn empezar(w: &[AtomicU32], campo_sello: usize) -> u32 {
    let sello = w[campo_sello].load(Ordering::Relaxed);
    w[campo_sello].store(sello.wrapping_add(1), Ordering::Relaxed);
    fence(Ordering::Release);
    sello
}

/// **Acabar la ranura** (el escritor): el sello, PAR otra vez -- la ranura
/// esta entera.
pub fn acabar(w: &[AtomicU32], campo_sello: usize, sello: u32) {
    w[campo_sello].store(sello.wrapping_add(2), Ordering::Release);
}

/// **Mirar el sello antes de copiar** (el lector): `None` si es impar -- el
/// escritor esta dentro y lo que se copiara seria medio dato --.
pub fn mirar(w: &[AtomicU32], campo_sello: usize) -> Option<u32> {
    let e1 = w[campo_sello].load(Ordering::Acquire);
    (e1 % 2 == 0).then_some(e1)
}

/// **Despues de copiar** (el lector): lo copiado vale si el sello sigue
/// siendo `e1` -- nadie toco la ranura mientras tanto --.
pub fn intacto(w: &[AtomicU32], campo_sello: usize, e1: u32) -> bool {
    fence(Ordering::Acquire);
    w[campo_sello].load(Ordering::Relaxed) == e1
}

/// **Y la secuencia sigue siendo `s`** (el lector, despues de [`intacto`]):
/// lo copiado es el publicado con `s` y no uno que el escritor dejo despues
/// en la misma ranura. Si no, se descarta y se vuelve a mirar.
pub fn vigente(w: &[AtomicU32], campo: usize, s: u32) -> bool {
    secuencia(w, campo) == s
}

// -- La palabra de VUELTA (C1) -------------------------------------------------

/// **Reservar la ranura publicada** para un lector que no copia (la 3060):
/// `Some((s, k))` si la ranura `k` del publicado `s` queda en vuelo -- el
/// escritor no la tocara hasta [`soltar`] --; `None` si no hay nada publicado
/// o el escritor estaba dentro (se vuelve a intentar la vuelta siguiente).
pub fn reservar(w: &[AtomicU32], campo_vuelta: usize, campo_secuencia: usize, campo_sello: usize) -> Option<(u32, usize)> {
    let s = secuencia(w, campo_secuencia);
    if s == 0 {
        return None;
    }
    let k = ranura_publicada(s);
    w[campo_vuelta].store(k as u32 + 1, Ordering::Relaxed);
    fence(Ordering::SeqCst);
    let par = w[campo_sello + k].load(Ordering::Relaxed) % 2 == 0;
    if !par || secuencia(w, campo_secuencia) != s {
        soltar(w, campo_vuelta);
        return None;
    }
    fence(Ordering::Acquire);
    Some((s, k))
}

/// **Soltar la reserva** (cuando la 3060 acabo: tras su valla).
pub fn soltar(w: &[AtomicU32], campo_vuelta: usize) {
    w[campo_vuelta].store(0, Ordering::Release);
}

/// **Empezar a escribir la ranura `k`, si no esta en vuelo** (el escritor que
/// respeta la vuelta): como [`empezar`], pero si el lector la reservo, deja
/// el sello como estaba y da `None` -- ese fotograma no se escribe --.
pub fn empezar_si_libre(w: &[AtomicU32], campo_sello: usize, campo_vuelta: usize, k: usize) -> Option<u32> {
    let sello = w[campo_sello + k].load(Ordering::Relaxed);
    w[campo_sello + k].store(sello.wrapping_add(1), Ordering::Relaxed);
    fence(Ordering::SeqCst);
    if w[campo_vuelta].load(Ordering::Relaxed) == k as u32 + 1 {
        // No se toco nada de la ranura: el sello vuelve a ser el de antes.
        w[campo_sello + k].store(sello, Ordering::Release);
        return None;
    }
    Some(sello)
}

#[cfg(test)]
mod pruebas {
    extern crate std;

    use super::*;
    use std::sync::Arc;
    use std::vec::Vec;

    fn bloque(palabras: usize) -> Vec<AtomicU32> {
        (0..palabras).map(|_| AtomicU32::new(0)).collect()
    }

    // Una clase de mentira, solo para estas pruebas (no es una magia del
    // arbol: el guardian `laminas` no la busca con este nombre).
    const PRUEBA: u32 = u32::from_le_bytes(*b"PRUE");

    #[test]
    fn la_cabecera_comun_se_crea_y_se_abre() {
        let b = bloque(32);
        b[10].store(7, Ordering::Relaxed);
        assert!(crear(&b, PRUEBA, 1, 64));
        assert_eq!(b[10].load(Ordering::Relaxed), 0, "la cabecera entera nace a cero");
        assert_eq!(abrir(&b, PRUEBA, 1, 64), Ok(()));
        assert_eq!(abrir(&b, u32::from_le_bytes(*b"BSUP"), 1, 64), Err(NoEs::Magia), "otra clase no es esta");
        assert_eq!(abrir(&b, PRUEBA, 2, 64), Err(NoEs::Version));
        assert_eq!(abrir(&b, PRUEBA, 1, 256), Err(NoEs::Corta));
        assert_eq!(abrir(&b[..2], PRUEBA, 1, 8), Err(NoEs::Corta));
    }

    #[test]
    fn crear_no_se_sale_del_bloque() {
        let b = bloque(4);
        assert!(!crear(&b, PRUEBA, 1, 32), "no cabe");
        assert!(!crear(&b, PRUEBA, 1, 8), "sin sitio para las tres palabras comunes");
        assert!(!crear(&b, PRUEBA, 1, 14), "no es multiplo de 4");
        assert!(crear(&b, PRUEBA, 1, 16));
    }

    #[test]
    fn las_dos_ranuras_se_turnan() {
        // Con nada publicado se escribe la 1 y, publicada, se lee la 1.
        assert_eq!(ranura_para_escribir(0), 1);
        assert_eq!(ranura_publicada(1), 1);
        assert_eq!(ranura_para_escribir(1), 0);
        assert_eq!(ranura_publicada(2), 0);
        for s in 0..10u32 {
            assert_ne!(ranura_para_escribir(s), ranura_publicada(s), "nunca se escribe la que se lee");
        }
    }

    /// El sello: impar mientras se escribe, y el lector lo ve antes y despues.
    #[test]
    fn el_sello_ve_al_escritor_dentro() {
        let b = bloque(4);
        assert_eq!(mirar(&b, 0), Some(0));
        let s = empezar(&b, 0);
        assert_eq!(mirar(&b, 0), None, "a medio escribir");
        acabar(&b, 0, s);
        let e1 = mirar(&b, 0).unwrap();
        assert!(intacto(&b, 0, e1));
        let s = empezar(&b, 0);
        acabar(&b, 0, s);
        assert!(!intacto(&b, 0, e1), "la pisaron mientras se copiaba");
    }

    /// El caso que cazo la castigada, paso a paso: el lector lee `s`, el
    /// escritor publica el siguiente y deja ENTERO el de despues en la ranura
    /// de `s`. El sello no lo ve; la secuencia si.
    #[test]
    fn uno_posterior_en_la_misma_ranura_no_se_da_por_el_publicado() {
        const SEC: usize = 3;
        const SELLO: usize = 4;
        let b = bloque(8);
        // Escribir una ranura entera (sin datos: lo que importa es el sello).
        let escribe = || {
            let s = secuencia_propia(&b, SEC);
            let k = ranura_para_escribir(s);
            let sello = empezar(&b, SELLO + k);
            acabar(&b, SELLO + k, sello);
            (s, k)
        };
        let (s0, _) = escribe();
        publicar(&b, SEC, s0);
        // El lector: la secuencia publicada.
        let s = secuencia(&b, SEC);
        let k = ranura_publicada(s);
        // El escritor: publica el 2 (la otra ranura)...
        let (s1, k1) = escribe();
        assert_ne!(k1, k);
        publicar(&b, SEC, s1);
        // ...y deja ENTERO el 3 en la ranura que el lector va a copiar.
        let (_, k2) = escribe();
        assert_eq!(k2, k);
        // El lector mira el sello: par. Copia. Sigue igual. Pero NO es el de s.
        let e1 = mirar(&b, SELLO + k).expect("par: el escritor ya salio");
        assert!(intacto(&b, SELLO + k, e1), "el sello solo no lo ve");
        assert!(!vigente(&b, SEC, s), "la secuencia si");
    }

    /// La vuelta, paso a paso: reservada, el escritor no entra en esa ranura;
    /// soltada, si.
    #[test]
    fn la_vuelta_aparta_al_escritor() {
        const SEC: usize = 3;
        const SELLO: usize = 4;
        const VUELTA: usize = 6;
        let b = bloque(8);
        assert_eq!(reservar(&b, VUELTA, SEC, SELLO), None, "nada publicado");
        // publicar el 1 en la ranura 1
        let s = empezar_si_libre(&b, SELLO, VUELTA, 1).unwrap();
        acabar(&b, SELLO + 1, s);
        publicar(&b, SEC, 0);
        assert_eq!(reservar(&b, VUELTA, SEC, SELLO), Some((1, 1)));
        // el escritor va a la ranura 0 (libre) y publica el 2...
        let s = empezar_si_libre(&b, SELLO, VUELTA, 0).unwrap();
        acabar(&b, SELLO, s);
        publicar(&b, SEC, 1);
        // ...y la siguiente es la 1, EN VUELO: no entra, y el sello no cambia
        let antes = b[SELLO + 1].load(Ordering::Relaxed);
        assert_eq!(empezar_si_libre(&b, SELLO, VUELTA, 1), None);
        assert_eq!(b[SELLO + 1].load(Ordering::Relaxed), antes);
        soltar(&b, VUELTA);
        assert!(empezar_si_libre(&b, SELLO, VUELTA, 1).is_some(), "soltada, entra");
        // y con el escritor DENTRO (sello impar) no se reserva
        publicar(&b, SEC, 2);
        let k = ranura_publicada(3);
        b[SELLO + k].store(1, Ordering::Relaxed);
        assert_eq!(reservar(&b, VUELTA, SEC, SELLO), None);
        assert_eq!(b[VUELTA].load(Ordering::Relaxed), 0, "y la reserva fallida no se queda puesta");
    }

    /// ***Castigado***: la 3060 de mentira. Un escritor que respeta la vuelta
    /// y un lector que NO copia: reserva y mira la ranura VARIAS veces,
    /// despacio, como la leeria la 3060. Mientras esta en vuelo no cambia ni
    /// una palabra, y lo que ve es una ranura entera.
    #[test]
    fn la_tres_mil_sesenta_de_mentira_nunca_ve_cambiar_su_ranura() {
        const SEC: usize = 3;
        const SELLO: usize = 4;
        const VUELTA: usize = 6;
        const DATOS: usize = 8;
        const ANCHO: usize = 16;
        let b: Arc<Vec<AtomicU32>> = Arc::new(bloque(DATOS + 2 * ANCHO));
        assert!(crear(&b, PRUEBA, 1, DATOS * 4));
        let basta = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let escritor = {
            let b = Arc::clone(&b);
            let basta = Arc::clone(&basta);
            std::thread::spawn(move || {
                let mut ocupadas = 0u32;
                let mut n = 0u32;
                // escribe hasta que el lector diga basta: los dos a la vez
                while !basta.load(Ordering::Relaxed) {
                    n += 1;
                    let s = secuencia_propia(&b, SEC);
                    let k = ranura_para_escribir(s);
                    let Some(sello) = empezar_si_libre(&b, SELLO, VUELTA, k) else {
                        ocupadas += 1;
                        continue;
                    };
                    for x in &b[DATOS + k * ANCHO..DATOS + (k + 1) * ANCHO] {
                        x.store(n, Ordering::Relaxed);
                    }
                    acabar(&b, SELLO + k, sello);
                    publicar(&b, SEC, s);
                }
                ocupadas
            })
        };
        let (mut dibujados, mut ultimo) = (0u32, 0u32);
        while dibujados < 20_000 {
            let Some((_, k)) = reservar(&b, VUELTA, SEC, SELLO) else { continue };
            let ranura = &b[DATOS + k * ANCHO..DATOS + (k + 1) * ANCHO];
            let primera = ranura[0].load(Ordering::Relaxed);
            // "la 3060 lee": cuatro pasadas, con tiempo entre medias
            for _ in 0..4 {
                for x in ranura {
                    assert_eq!(x.load(Ordering::Relaxed), primera, "la ranura en vuelo cambio");
                }
                std::hint::spin_loop();
            }
            assert!(primera >= ultimo, "no va hacia atras: {primera} tras {ultimo}");
            ultimo = primera;
            dibujados += 1;
            soltar(&b, VUELTA);
        }
        basta.store(true, Ordering::Relaxed);
        let ocupadas = escritor.join().unwrap();
        std::eprintln!("vuelta: {dibujados} dibujados sin copia, {ocupadas} fotogramas que el escritor no escribio (ranura en vuelo), el ultimo {ultimo}");
        assert!(ocupadas > 0, "con dos hilos a toda velocidad, el escritor tiene que haberse apartado alguna vez");
    }

    /// ***Castigado***: un escritor y un lector en dos hilos, con el trato
    /// entero. Lo que el lector da por bueno es una ranura ENTERA -- todas sus
    /// palabras del mismo numero --.
    #[test]
    fn un_escritor_y_un_lector_nunca_ven_medio_dato() {
        // 0 magia, 1 version, 2 cabecera, 3 secuencia, 4 y 5 sellos; ranuras
        // de 8 palabras desde la 8.
        const SEC: usize = 3;
        const SELLO: usize = 4;
        const DATOS: usize = 8;
        const ANCHO: usize = 8;
        let b: Arc<Vec<AtomicU32>> = Arc::new(bloque(DATOS + 2 * ANCHO));
        assert!(crear(&b, PRUEBA, 1, DATOS * 4));
        let escritor = {
            let b = Arc::clone(&b);
            std::thread::spawn(move || {
                for n in 1..=200_000u32 {
                    let s = secuencia_propia(&b, SEC);
                    let k = ranura_para_escribir(s);
                    let sello = empezar(&b, SELLO + k);
                    for x in &b[DATOS + k * ANCHO..DATOS + (k + 1) * ANCHO] {
                        x.store(n, Ordering::Relaxed);
                    }
                    acabar(&b, SELLO + k, sello);
                    publicar(&b, SEC, s);
                }
            })
        };
        let (mut buenos, mut rotos, mut ultimo) = (0u32, 0u32, 0u32);
        let mut copia = [0u32; ANCHO];
        while !escritor.is_finished() || buenos == 0 {
            let s = secuencia(&b, SEC);
            if s == 0 {
                continue;
            }
            let k = ranura_publicada(s);
            let Some(e1) = mirar(&b, SELLO + k) else {
                rotos += 1;
                continue;
            };
            for (c, x) in copia.iter_mut().zip(&b[DATOS + k * ANCHO..DATOS + (k + 1) * ANCHO]) {
                *c = x.load(Ordering::Relaxed);
            }
            if !intacto(&b, SELLO + k, e1) || !vigente(&b, SEC, s) {
                rotos += 1;
                continue;
            }
            assert!(copia.iter().all(|&x| x == copia[0]), "medio dato leido: {copia:?}");
            assert!(copia[0] >= ultimo, "no va hacia atras");
            ultimo = copia[0];
            buenos += 1;
        }
        escritor.join().unwrap();
        std::eprintln!("trato: {buenos} ranuras enteras, {rotos} descartadas, la ultima {ultimo}");
    }
}
