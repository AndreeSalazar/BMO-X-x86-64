//! **LOS SUB-DIRECTORES: un trozo de Ring 3 en cada obrero** (H4.3 de
//! `PLAN_LOS_DOCE_DIRECTORES`, 07-10; con el permiso de CPU del propietario:
//! *"te doy permiso con CPU pero que sea brutal para que exprima"*).
//!
//! [carril]  ROJO      decide con que pila, que bloque y que funcion entra
//!                     un nucleo en el espacio de un proceso
//! [cuesta]  DATO -- un pedido mal juzgado no falla aqui: el obrero entraria
//!           con una pila que no es suya y lo de otra parte se pisaria.
//!
//! Hasta hoy un obrero solo corria faenas DEL KERNEL (`atril`): numeros de
//! catalogo, nunca codigo de Ring 3. Esto deja que una app reparta SU codigo:
//!
//! ```text
//!    la app       llena n bloques (cada uno: lo que la parte quiera, y su
//!                 pila al final) y dice "repartir(funcion, arg, partes)"
//!    el kernel    juzga el pedido (esto), y cada obrero k = 1..n-1 entra en
//!                 Ring 3, en el CR3 de la app, en `funcion(k, n, arg)` con
//!                 la pila del bloque k; la parte 0 la corre la app
//!    la vuelta    `funcion` vuelve a [`FIN`] (una direccion que nunca esta
//!                 mapeada): el #PF de ahi es "termine"; cualquier otro
//!                 fallo, "esta parte fallo", y el obrero sigue vivo
//! ```
//!
//! **Aislado:** el obrero corre con IF=0 y SIN syscalls (EFER.SCE apagado
//! en los APs: un `syscall` es un #UD, o sea "esta parte fallo"); no toca el
//! planificador, ni drivers, ni CABINA: solo su ficha y su resultado. Lo que
//! la app no pudo hacer en un obrero, lo rehace ella en el nucleo 0.
//!
//! **Y no contradice a `atril`.** `atril` prohibe LLAMAR desde Ring 0 a un
//! puntero de Ring 3 (seria correr codigo de la app con el privilegio del
//! kernel). Aqui nadie lo llama: el obrero BAJA a Ring 3 (CPL 3, `iretq`) y
//! el codigo corre con el mismo privilegio que en el nucleo 0 de su app.

/// Donde vuelve una parte al acabar: el #PF de buscar instrucciones AQUI es
/// "termine". Canonica, de Ring 3, y por debajo de la ultima pagina: nunca
/// la mapea nadie (la ventana de reserva acaba mucho antes).
pub const FIN: u64 = 0x0000_7FFF_FFFF_E000;

/// El techo de lo que es Ring 3 (lo canonico de abajo).
pub const TECHO_RING3: u64 = 0x0000_8000_0000_0000;

/// Lo minimo que mide un bloque: su pila tiene que caber.
pub const BLOQUE_MINIMO: u64 = 64 << 10;

/// Las partes que caben (los 12 hilos del Ryzen de hoy, y holgura para una
/// CPU con mas: el perfil de la CPU dice cuantos hay de verdad).
pub const PARTES_MAXIMAS: u32 = 64;

/// **Un pedido**: la funcion, su argumento, cuantas partes, y los bloques
/// (uno por parte, seguidos, `bloque_bytes` cada uno).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pedido {
    pub funcion: u64,
    pub arg: u64,
    pub partes: u32,
    pub bloques: u64,
    pub bloque_bytes: u64,
}

/// Por que no.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoRing3 {
    /// Menos de 2 partes (una sola, la app la corre sola), o mas de las
    /// que hay obreros + la de la app.
    Partes = 1,
    /// La funcion o los bloques no son de Ring 3 (o la funcion es 0).
    NoEsRing3 = 2,
    /// Un bloque que no mide al menos [`BLOQUE_MINIMO`] o no va a 16.
    Bloque = 3,
    /// Los bloques se pasan del techo de Ring 3 (o la cuenta desborda).
    Desborda = 4,
    /// Ya hay una faena en marcha (del kernel o de Ring 3).
    Ocupado = 5,
    /// No hay obreros en pie (`smp all`).
    SinObreros = 6,
}

impl Pedido {
    /// **Juzgar** el pedido con los obreros que hay en pie.
    pub fn juzgar(self, obreros: u32) -> Result<Pedido, NoRing3> {
        if obreros == 0 {
            return Err(NoRing3::SinObreros);
        }
        if self.partes < 2 || self.partes > obreros + 1 || self.partes > PARTES_MAXIMAS {
            return Err(NoRing3::Partes);
        }
        if self.funcion == 0 || self.funcion >= TECHO_RING3 || self.bloques == 0 || self.bloques >= TECHO_RING3 {
            return Err(NoRing3::NoEsRing3);
        }
        if self.bloque_bytes < BLOQUE_MINIMO || self.bloque_bytes % 16 != 0 || self.bloques % 16 != 0 {
            return Err(NoRing3::Bloque);
        }
        let fin = (self.partes as u64).checked_mul(self.bloque_bytes).and_then(|t| t.checked_add(self.bloques)).ok_or(NoRing3::Desborda)?;
        if fin > TECHO_RING3 || fin > FIN {
            return Err(NoRing3::Desborda);
        }
        Ok(self)
    }

    /// El bloque de la parte `k`.
    pub fn bloque(&self, k: u32) -> u64 {
        self.bloques + k as u64 * self.bloque_bytes
    }

    /// **La pila con la que entra la parte `k`**: al final de su bloque,
    /// con la vuelta ([`FIN`]) ya empujada -- como si la hubieran llamado:
    /// `rsp + 8` va a 16, lo que pide el ABI de System V al entrar. La app
    /// escribe FIN en `[pila(k)]` (los 8 ultimos bytes del bloque).
    ///
    /// *** Era `- 16`, y con eso `rsp` iba a 16 al entrar: un `movaps` del
    /// codigo de la parte a la pila habria sido #GP -- una parte "fallida"
    /// que no tenia nada mal. El banco ahora pide `rsp % 16 == 8`.
    pub fn pila(&self, k: u32) -> u64 {
        self.bloque(k) + self.bloque_bytes - 8
    }
}

/// Lo mas que mide un bloque por la puerta: los bytes van en 48 bits.
pub const BYTES_PUERTA: u64 = (1 << 48) - 1;

/// **Bytes y partes en un numero** (la puerta del kernel lleva dos numeros y
/// `preparar` necesita tres: bloques, bytes de cada uno y partes).
pub fn empaquetar(bloque_bytes: u64, partes: u32) -> Option<u64> {
    if bloque_bytes > BYTES_PUERTA || partes > PARTES_MAXIMAS {
        return None;
    }
    Some(bloque_bytes | (partes as u64) << 48)
}

/// Lo de [`empaquetar`], al reves: `(bloque_bytes, partes)`.
pub fn desempaquetar(v: u64) -> (u64, u32) {
    (v & BYTES_PUERTA, (v >> 48) as u32)
}

/// El bit de "ya acabo" en lo que contesta `esperar` (los de abajo: una
/// mascara de las partes que NO salieron bien y la app rehace).
pub const ACABADA: u64 = 1 << 63;

#[cfg(test)]
mod pruebas {
    use super::*;

    fn bueno() -> Pedido {
        Pedido { funcion: 0x40_1000, arg: 7, partes: 12, bloques: 0x30_0000_0000, bloque_bytes: 1 << 20 }
    }

    /// Doce partes (once obreros y la app), cada una su bloque, sin
    /// solaparse; y la pila de cada una dentro del SUYO, alineada como pide
    /// el ABI al entrar en una funcion.
    #[test]
    fn un_pedido_bueno_da_a_cada_parte_su_bloque_y_su_pila() {
        let p = bueno().juzgar(11).unwrap();
        for k in 0..12 {
            let (b, s) = (p.bloque(k), p.pila(k));
            assert!(s > b && s < b + p.bloque_bytes, "la pila de {k} en su bloque");
            assert_eq!((s + 8) % 16, 0, "rsp + 8 va a 16: la vuelta ya esta empujada");
            assert!(s + 8 <= b + p.bloque_bytes, "FIN cabe en el bloque");
            if k > 0 {
                assert_eq!(b, p.bloque(k - 1) + p.bloque_bytes, "seguidos, sin solaparse");
            }
        }
    }

    /// Lo que dice NO, cada uno con su motivo.
    #[test]
    fn lo_que_no_se_reparte_dice_por_que() {
        assert_eq!(bueno().juzgar(0), Err(NoRing3::SinObreros));
        assert_eq!(bueno().juzgar(10), Err(NoRing3::Partes), "12 partes y 10 obreros + la app: 11");
        assert_eq!(Pedido { partes: 1, ..bueno() }.juzgar(11), Err(NoRing3::Partes));
        assert_eq!(Pedido { funcion: 0, ..bueno() }.juzgar(11), Err(NoRing3::NoEsRing3));
        assert_eq!(Pedido { funcion: 0xFFFF_8000_0000_0000, ..bueno() }.juzgar(11), Err(NoRing3::NoEsRing3), "una del kernel");
        assert_eq!(Pedido { bloque_bytes: 4096, ..bueno() }.juzgar(11), Err(NoRing3::Bloque));
        assert_eq!(Pedido { bloques: 0x30_0000_0008, ..bueno() }.juzgar(11), Err(NoRing3::Bloque));
        assert_eq!(Pedido { bloques: 0x7FFF_FFF0_0000, ..bueno() }.juzgar(11), Err(NoRing3::Desborda), "pisaria FIN");
        assert_eq!(Pedido { bloque_bytes: u64::MAX - 15, ..bueno() }.juzgar(11), Err(NoRing3::Desborda));
    }

    /// Tres numeros por una puerta de dos: ida y vuelta sin perder nada, y
    /// lo que no cabe dice NO.
    #[test]
    fn bytes_y_partes_caben_en_un_numero() {
        let v = empaquetar(1 << 20, 12).unwrap();
        assert_eq!(desempaquetar(v), (1 << 20, 12));
        assert_eq!(empaquetar(BYTES_PUERTA + 1, 2), None);
        assert_eq!(empaquetar(1 << 20, PARTES_MAXIMAS + 1), None);
        assert_eq!(desempaquetar(empaquetar(BYTES_PUERTA, PARTES_MAXIMAS).unwrap()), (BYTES_PUERTA, PARTES_MAXIMAS));
    }

    /// FIN es de Ring 3, canonica, y no cae en la ventana de reserva de
    /// PROTON-X (0x20_0000_0000, 384 GiB) ni en la de imagenes.
    #[test]
    fn la_vuelta_no_la_mapea_nadie() {
        assert!(FIN < TECHO_RING3 && FIN % 4096 == 0);
        assert!(FIN > 0x20_0000_0000 + (384u64 << 30));
    }
}
