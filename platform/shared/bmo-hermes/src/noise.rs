//! **EL SALUDO** -- Noise, con los patrones XX e IK y la suite
//! `25519_AESGCM_SHA256`, sobre lo que `bmo-cripto` ya tiene.
//!
//! [carril]  AMARILLO  lee lo que manda otra maquina
//! [cuesta]  DATO      un saludo mal llevado da una conversacion que otro lee
//! [riesgo]  AJENO     cada byte del saludo lo escribe la otra punta
//!
//! # Por que Noise, y no "X25519 + AES-GCM" a secas
//!
//! El esbozo del 01-10 decia *"TCP con X25519 + AES-GCM"*. Sin mas, eso cifra
//! pero no dice CON QUIEN: el vecino de la LAN se pone en medio, hace un
//! intercambio con cada punta y lee todo. Lo que falta es que la clave fija de
//! cada maquina entre en la cuenta, y eso es exactamente lo que ordena Noise:
//!
//! ```text
//!    XX   la primera vez:  -> e
//!                          <- e, ee, s, es
//!                          -> s, se
//!    IK   con un amigo:    <- s            (su publica ya se conoce)
//!                          -> e, es, s, ss
//!                          <- e, ee, se
//! ```
//!
//! *** **Y no hace falta firmar nada.** Quien no tiene la clave privada que
//! corresponde a su publica no puede sacar el mismo secreto en `es`/`se`/`ss`,
//! y el primer mensaje cifrado que mande no abrira. Por eso la identidad de
//! HERMES es una clave X25519 y `PLAN_SEGURIDAD` C3 --BMO-X no firma-- sigue
//! entero.
//!
//! # Lo que esta pagina NO hace, a proposito
//!
//! - **No saca el azar.** La efimera ENTRA como argumento, como la hora entra en
//!   `bmo-pila`: asi el saludo es determinista y se juzga contra los vectores
//!   publicados. Quien la usa la saca de `bmo_cripto::azar::clave()`.
//! - **No decide si la otra punta es un amigo.** Devuelve su publica
//!   ([`Saludo::remota`]) y eso lo juzga quien tiene `amigos.txt`.
//! - **No borra las claves de la memoria al acabar.** Lo mismo que dice
//!   `bmo_cripto::hmac`: prometerlo pide escrituras volatiles.
//!
//! [!] **Un error mata el saludo.** Despues de cualquier `Err`, todas las
//! llamadas devuelven [`Rechazo::Roto`]: un estado a medio mezclar no se
//! reintenta, se tira y se saluda otra vez.

use bmo_cripto::{gcm, hmac, sha256, x25519};

use crate::Rechazo;

/// Bytes de una clave X25519, publica o privada.
pub const CLAVE: usize = 32;
/// Bytes que la etiqueta de AES-GCM agrega a cada cosa cifrada.
pub const ETIQUETA: usize = gcm::ETIQUETA;
/// Lo mas largo que puede ser un mensaje de Noise. Lo dice la especificacion,
/// y es tambien lo que cabe en la cabecera de dos bytes de [`crate::marco`].
pub const MENSAJE_MAX: usize = 65_535;

/// Cual de los dos saludos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Patron {
    /// La primera vez: ninguno conoce la clave fija del otro.
    XX,
    /// Con un amigo: el que llama ya sabe la clave fija del que contesta.
    IK,
}

/// Quien empieza.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Papel {
    Inicia,
    Responde,
}

impl Patron {
    /// El nombre del protocolo, que es lo primero que entra en el hash.
    pub fn nombre(self) -> &'static [u8] {
        match self {
            Patron::XX => b"Noise_XX_25519_AESGCM_SHA256",
            Patron::IK => b"Noise_IK_25519_AESGCM_SHA256",
        }
    }

    fn fichas(self) -> &'static [&'static [Ficha]] {
        match self {
            Patron::XX => FICHAS_XX,
            Patron::IK => FICHAS_IK,
        }
    }

    /// Cuantos mensajes tiene el saludo.
    pub fn pasos(self) -> usize {
        self.fichas().len()
    }

    /// **La medida exacta del mensaje `paso` con la carga VACIA.**
    ///
    /// HERMES/1 no lleva nada en la carga del saludo, asi que cada mensaje del
    /// saludo tiene UNA medida posible. La PUERTA la usa para cortar antes de
    /// leer: un primer mensaje de otra medida no es HERMES (seccion 4.2 del
    /// plan: *"antes del saludo no se lee nada"*).
    pub fn medida_vacia(self, paso: usize) -> Option<usize> {
        let fichas = self.fichas();
        if paso >= fichas.len() {
            return None;
        }
        // Hay clave en cuanto ha pasado un DH, y eso no depende de quien hable.
        let mut con_clave = false;
        for p in fichas.iter().take(paso) {
            if p.iter().any(|f| f.es_dh()) {
                con_clave = true;
            }
        }
        let mut n = 0;
        for f in fichas[paso] {
            match f {
                Ficha::E => n += CLAVE,
                Ficha::S => n += CLAVE + if con_clave { ETIQUETA } else { 0 },
                _ => con_clave = true,
            }
        }
        Some(n + if con_clave { ETIQUETA } else { 0 })
    }
}

#[derive(Clone, Copy)]
enum Ficha {
    E,
    S,
    EE,
    ES,
    SE,
    SS,
}

impl Ficha {
    fn es_dh(self) -> bool {
        !matches!(self, Ficha::E | Ficha::S)
    }
}

const FICHAS_XX: &[&[Ficha]] = &[&[Ficha::E], &[Ficha::E, Ficha::EE, Ficha::S, Ficha::ES], &[Ficha::S, Ficha::SE]];
const FICHAS_IK: &[&[Ficha]] = &[&[Ficha::E, Ficha::ES, Ficha::S, Ficha::SS], &[Ficha::E, Ficha::EE, Ficha::SE]];

// == CipherState ============================================================

/// **Una direccion de la conversacion**: su clave y su contador.
///
/// El nonce de AES-GCM es el contador: cuatro bytes a cero y ocho en big-endian,
/// como manda Noise. Sube con cada mensaje que abre o cierra bien, y no se
/// repite nunca con la misma clave (`gcm.rs` explica por que eso no es un
/// detalle). Al llegar a `u64::MAX` se corta: ese ultimo valor Noise lo reserva.
#[derive(Clone)]
struct Cifra {
    k: Option<[u8; CLAVE]>,
    n: u64,
}

impl Cifra {
    const fn vacia() -> Self {
        Cifra { k: None, n: 0 }
    }

    fn con(k: [u8; CLAVE]) -> Self {
        Cifra { k: Some(k), n: 0 }
    }

    fn nonce(n: u64) -> [u8; gcm::NONCE] {
        let mut x = [0u8; gcm::NONCE];
        x[4..].copy_from_slice(&n.to_be_bytes());
        x
    }

    /// Cifra `buf[..largo]` en su sitio y pone la etiqueta detras. Sin clave,
    /// el texto se queda como esta (es lo que pide Noise antes del primer DH).
    fn sellar(&mut self, ad: &[u8], buf: &mut [u8], largo: usize) -> Result<usize, Rechazo> {
        let Some(k) = self.k else {
            return Ok(largo);
        };
        if self.n == u64::MAX {
            return Err(Rechazo::Agotado);
        }
        if buf.len() < largo + ETIQUETA {
            return Err(Rechazo::SinSitio);
        }
        let t = gcm::sellar(&k, &Self::nonce(self.n), ad, &mut buf[..largo]).ok_or(Rechazo::Etiqueta)?;
        buf[largo..largo + ETIQUETA].copy_from_slice(&t);
        self.n += 1;
        Ok(largo + ETIQUETA)
    }

    /// Comprueba la etiqueta de `buf` (cifrado + etiqueta) y, solo si cuadra,
    /// descifra en su sitio. Devuelve cuanto mide el texto.
    fn abrir(&mut self, ad: &[u8], buf: &mut [u8]) -> Result<usize, Rechazo> {
        let Some(k) = self.k else {
            return Ok(buf.len());
        };
        if self.n == u64::MAX {
            return Err(Rechazo::Agotado);
        }
        if buf.len() < ETIQUETA {
            return Err(Rechazo::Corto);
        }
        let l = buf.len() - ETIQUETA;
        let mut t = [0u8; ETIQUETA];
        t.copy_from_slice(&buf[l..]);
        if !gcm::abrir(&k, &Self::nonce(self.n), ad, &mut buf[..l], &t) {
            return Err(Rechazo::Etiqueta);
        }
        self.n += 1;
        Ok(l)
    }
}

// == SymmetricState =========================================================

/// HKDF de Noise con dos salidas: no es el `expandir` de RFC 5869 con
/// etiqueta, es la cadena de HMAC que define la seccion 4.3 de la
/// especificacion. Se escribe aqui tal cual para que se lea contra ella.
fn hkdf2(ck: &[u8; 32], ikm: &[u8]) -> ([u8; 32], [u8; 32]) {
    let t = hmac::hmac(ck, ikm);
    let o1 = hmac::hmac(&t, &[1]);
    let mut m = [0u8; 33];
    m[..32].copy_from_slice(&o1);
    m[32] = 2;
    let o2 = hmac::hmac(&t, &m);
    (o1, o2)
}

#[derive(Clone)]
struct Simetrico {
    ck: [u8; 32],
    h: [u8; 32],
    c: Cifra,
}

impl Simetrico {
    fn nuevo(nombre: &[u8]) -> Self {
        let mut h = [0u8; 32];
        if nombre.len() <= 32 {
            h[..nombre.len()].copy_from_slice(nombre);
        } else {
            h = sha256::hash(nombre);
        }
        Simetrico { ck: h, h, c: Cifra::vacia() }
    }

    fn mezclar_hash(&mut self, d: &[u8]) {
        let mut s = sha256::Sha256::nuevo();
        s.mete(&self.h);
        s.mete(d);
        self.h = s.cierra();
    }

    fn mezclar_clave(&mut self, ikm: &[u8]) {
        let (ck, k) = hkdf2(&self.ck, ikm);
        self.ck = ck;
        self.c = Cifra::con(k);
    }

    fn cifrar_y_hash(&mut self, buf: &mut [u8], largo: usize) -> Result<usize, Rechazo> {
        let h = self.h;
        let n = self.c.sellar(&h, buf, largo)?;
        self.mezclar_hash(&buf[..n]);
        Ok(n)
    }

    /// El hash se mezcla con el CIFRADO, asi que se calcula antes de descifrar
    /// en su sitio, y solo se guarda si la etiqueta cuadra.
    fn descifrar_y_hash(&mut self, buf: &mut [u8]) -> Result<usize, Rechazo> {
        let h = self.h;
        let mut s = sha256::Sha256::nuevo();
        s.mete(&h);
        s.mete(buf);
        let nuevo = s.cierra();
        let n = self.c.abrir(&h, buf)?;
        self.h = nuevo;
        Ok(n)
    }

    fn partir(&self) -> (Cifra, Cifra) {
        let (a, b) = hkdf2(&self.ck, &[]);
        (Cifra::con(a), Cifra::con(b))
    }
}

// == HandshakeState =========================================================

/// **El saludo en curso.** Se crea, se van escribiendo y leyendo mensajes por
/// turnos, y cuando [`Saludo::terminado`] dice que si, [`Saludo::partir`] da
/// el [`Canal`] de la conversacion.
#[derive(Clone)]
pub struct Saludo {
    sim: Simetrico,
    patron: Patron,
    papel: Papel,
    paso: usize,
    roto: bool,
    s: [u8; CLAVE],
    s_pub: [u8; CLAVE],
    e: [u8; CLAVE],
    e_pub: [u8; CLAVE],
    re: Option<[u8; CLAVE]>,
    rs: Option<[u8; CLAVE]>,
}

impl Saludo {
    /// `estatica` es la clave fija de esta maquina; `efimera`, una nueva por
    /// saludo, del azar. `remota` es la publica fija del otro, y solo la lleva
    /// quien INICIA un IK (para eso es un amigo).
    pub fn nuevo(
        patron: Patron,
        papel: Papel,
        prologo: &[u8],
        estatica: &[u8; CLAVE],
        efimera: &[u8; CLAVE],
        remota: Option<&[u8; CLAVE]>,
    ) -> Result<Saludo, Rechazo> {
        let mut sim = Simetrico::nuevo(patron.nombre());
        sim.mezclar_hash(prologo);
        let s_pub = x25519::secreto_a_publico(estatica);
        let e_pub = x25519::secreto_a_publico(efimera);
        let rs = remota.copied();
        match (patron, papel) {
            (Patron::IK, Papel::Inicia) => sim.mezclar_hash(rs.as_ref().ok_or(Rechazo::FaltaRemota)?),
            (Patron::IK, Papel::Responde) => sim.mezclar_hash(&s_pub),
            (Patron::XX, _) => {}
        }
        if rs.is_some() && !(patron == Patron::IK && papel == Papel::Inicia) {
            return Err(Rechazo::SobraRemota);
        }
        Ok(Saludo { sim, patron, papel, paso: 0, roto: false, s: *estatica, s_pub, e: *efimera, e_pub, re: None, rs })
    }

    /// Ya se mandaron y leyeron todos los mensajes del saludo?
    pub fn terminado(&self) -> bool {
        self.paso >= self.patron.pasos()
    }

    /// Le toca escribir a esta punta?
    pub fn me_toca(&self) -> bool {
        !self.terminado() && ((self.paso % 2 == 0) == (self.papel == Papel::Inicia))
    }

    /// El hash de todo lo dicho hasta ahora. Al terminar, las dos puntas tienen
    /// el mismo, y eso es lo que comparan los vectores.
    pub fn hash(&self) -> [u8; 32] {
        self.sim.h
    }

    /// **La clave fija de la otra punta**, en cuanto se conoce. Quien tiene la
    /// lista de amigos decide con esto si sigue.
    pub fn remota(&self) -> Option<[u8; CLAVE]> {
        self.rs
    }

    /// Por que paso va (0, 1, 2...).
    pub fn paso(&self) -> usize {
        self.paso
    }

    fn dh(&self, f: Ficha) -> Result<[u8; 32], Rechazo> {
        let ini = self.papel == Papel::Inicia;
        let re = self.re.ok_or(Rechazo::Corto);
        let rs = self.rs.ok_or(Rechazo::FaltaRemota);
        let (mia, suya) = match f {
            Ficha::EE => (self.e, re?),
            Ficha::ES if ini => (self.e, rs?),
            Ficha::ES => (self.s, re?),
            Ficha::SE if ini => (self.s, re?),
            Ficha::SE => (self.e, rs?),
            Ficha::SS => (self.s, rs?),
            Ficha::E | Ficha::S => return Err(Rechazo::Roto),
        };
        let k = x25519::secreto_compartido(&mia, &suya);
        // RFC 7748: una publica de orden chico da ceros, y seguir seria hablar
        // con una clave que el atacante conoce.
        if x25519::es_cero(&k) {
            return Err(Rechazo::ClaveDebil);
        }
        Ok(k)
    }

    /// **Escribe el siguiente mensaje del saludo** en `dst`, con `carga`
    /// dentro (HERMES/1 la manda vacia). Devuelve cuantos bytes ocupa.
    pub fn escribir(&mut self, carga: &[u8], dst: &mut [u8]) -> Result<usize, Rechazo> {
        let r = self.escribir_dentro(carga, dst);
        if r.is_err() {
            self.roto = true;
        }
        r
    }

    fn escribir_dentro(&mut self, carga: &[u8], dst: &mut [u8]) -> Result<usize, Rechazo> {
        self.puede(true)?;
        let mut p = 0;
        for &f in self.patron.fichas()[self.paso] {
            match f {
                Ficha::E => {
                    let fin = p + CLAVE;
                    dst.get_mut(p..fin).ok_or(Rechazo::SinSitio)?.copy_from_slice(&self.e_pub);
                    let e_pub = self.e_pub;
                    self.sim.mezclar_hash(&e_pub);
                    p = fin;
                }
                Ficha::S => {
                    dst.get_mut(p..p + CLAVE).ok_or(Rechazo::SinSitio)?.copy_from_slice(&self.s_pub);
                    p += self.sim.cifrar_y_hash(&mut dst[p..], CLAVE)?;
                }
                dh => {
                    let k = self.dh(dh)?;
                    self.sim.mezclar_clave(&k);
                }
            }
        }
        dst.get_mut(p..p + carga.len()).ok_or(Rechazo::SinSitio)?.copy_from_slice(carga);
        p += self.sim.cifrar_y_hash(&mut dst[p..], carga.len())?;
        if p > MENSAJE_MAX {
            return Err(Rechazo::Largo);
        }
        self.paso += 1;
        Ok(p)
    }

    /// **Lee el siguiente mensaje del saludo** y deja su carga en `dst`.
    pub fn leer(&mut self, msg: &[u8], dst: &mut [u8]) -> Result<usize, Rechazo> {
        let r = self.leer_dentro(msg, dst);
        if r.is_err() {
            self.roto = true;
        }
        r
    }

    fn leer_dentro(&mut self, msg: &[u8], dst: &mut [u8]) -> Result<usize, Rechazo> {
        self.puede(false)?;
        if msg.len() > MENSAJE_MAX {
            return Err(Rechazo::Largo);
        }
        let mut p = 0;
        for &f in self.patron.fichas()[self.paso] {
            match f {
                Ficha::E => {
                    let mut re = [0u8; CLAVE];
                    re.copy_from_slice(msg.get(p..p + CLAVE).ok_or(Rechazo::Corto)?);
                    self.re = Some(re);
                    self.sim.mezclar_hash(&re);
                    p += CLAVE;
                }
                Ficha::S => {
                    let l = CLAVE + if self.sim.c.k.is_some() { ETIQUETA } else { 0 };
                    let mut t = [0u8; CLAVE + ETIQUETA];
                    t[..l].copy_from_slice(msg.get(p..p + l).ok_or(Rechazo::Corto)?);
                    self.sim.descifrar_y_hash(&mut t[..l])?;
                    let mut rs = [0u8; CLAVE];
                    rs.copy_from_slice(&t[..CLAVE]);
                    self.rs = Some(rs);
                    p += l;
                }
                dh => {
                    let k = self.dh(dh)?;
                    self.sim.mezclar_clave(&k);
                }
            }
        }
        let resto = &msg[p..];
        let ventana = dst.get_mut(..resto.len()).ok_or(Rechazo::SinSitio)?;
        ventana.copy_from_slice(resto);
        let n = self.sim.descifrar_y_hash(ventana)?;
        self.paso += 1;
        Ok(n)
    }

    fn puede(&self, escribir: bool) -> Result<(), Rechazo> {
        if self.roto {
            return Err(Rechazo::Roto);
        }
        if self.terminado() {
            return Err(Rechazo::SaludoTerminado);
        }
        if self.me_toca() != escribir {
            return Err(Rechazo::FueraDeTurno);
        }
        Ok(())
    }

    /// **Se acabo el saludo: empieza la conversacion.** Cada punta se queda
    /// con la clave de enviar y la de recibir, que son distintas.
    pub fn partir(self) -> Result<Canal, Rechazo> {
        if self.roto {
            return Err(Rechazo::Roto);
        }
        if !self.terminado() {
            return Err(Rechazo::SaludoSinTerminar);
        }
        let (c1, c2) = self.sim.partir();
        let (envio, recibo) = if self.papel == Papel::Inicia { (c1, c2) } else { (c2, c1) };
        Ok(Canal { envio, recibo, hash: self.sim.h, remota: self.rs })
    }
}

// == el transporte ==========================================================

/// **La conversacion ya cifrada**, una vez hecho el saludo.
#[derive(Clone)]
pub struct Canal {
    envio: Cifra,
    recibo: Cifra,
    hash: [u8; 32],
    remota: Option<[u8; CLAVE]>,
}

impl Canal {
    /// Cifra `carga` en `dst`. Devuelve cuantos bytes van al cable.
    pub fn sellar(&mut self, carga: &[u8], dst: &mut [u8]) -> Result<usize, Rechazo> {
        if carga.len() + ETIQUETA > MENSAJE_MAX {
            return Err(Rechazo::Largo);
        }
        dst.get_mut(..carga.len()).ok_or(Rechazo::SinSitio)?.copy_from_slice(carga);
        self.envio.sellar(&[], dst, carga.len())
    }

    /// Abre un mensaje del cable en `dst`. Si la etiqueta no cuadra, `dst`
    /// no tiene nada que leer y el contador no se mueve.
    pub fn abrir(&mut self, msg: &[u8], dst: &mut [u8]) -> Result<usize, Rechazo> {
        if msg.len() > MENSAJE_MAX {
            return Err(Rechazo::Largo);
        }
        if msg.len() < ETIQUETA {
            return Err(Rechazo::Corto);
        }
        let ventana = dst.get_mut(..msg.len()).ok_or(Rechazo::SinSitio)?;
        ventana.copy_from_slice(msg);
        self.recibo.abrir(&[], ventana)
    }

    /// Cuantos mensajes ha mandado esta punta. Es tambien el numero del
    /// SIGUIENTE, y por eso una REACCION puede nombrar un mensaje sin que el
    /// mensaje lleve un numero dentro.
    pub fn enviados(&self) -> u64 {
        self.envio.n
    }

    /// Cuantos mensajes ha abierto esta punta.
    pub fn recibidos(&self) -> u64 {
        self.recibo.n
    }

    /// El hash del saludo: el mismo en las dos puntas.
    pub fn hash(&self) -> [u8; 32] {
        self.hash
    }

    /// La clave fija de la otra punta.
    pub fn remota(&self) -> Option<[u8; CLAVE]> {
        self.remota
    }
}

#[cfg(test)]
#[path = "noise_pruebas.rs"]
mod pruebas;
