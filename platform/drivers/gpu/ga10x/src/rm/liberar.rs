//! **E7b: LIBERAR EL CANAL DE GR MUERTO** (13-10) -- el `FREE` del RM, SOLO
//! para los tres objetos del canal de GR0, para pedirlo otra vez sin
//! reiniciar la maquina.
//!
//! capa: puro -- arma y juzga bytes; no toca un registro (L8)
//!
//! [eje]     CORRECCION -- `rpc_free_v03_00` de la 570.144: un
//!           `NVOS00_PARAMETERS_v03_00` (hRoot, hObjectParent, hObjectOld,
//!           status), funcion 10 (`NV_VGPU_MSG_FUNCTION_FREE`). El padre a 0,
//!           como `r535_gsp_rpc_rm_free` de nouveau.
//!
//! # Por que existe
//!
//! Un canal que tomo un Xid el GSP-RM lo deja MUERTO, y hasta hoy BMO-X solo
//! sabia reiniciar: el propietario dependia del boton. Los drivers de verdad
//! hacen lo mismo que se hace aqui -- liberan el canal y sus objetos y lo
//! piden de nuevo -- y el resto de la tarjeta (el cliente, el dispositivo, el
//! espacio de direcciones, el canal de copia) sigue vivo.
//!
//! # Lo que el contrato deja liberar, y en que orden
//!
//! ```text
//!    1  AMPERE_B          el objeto 3D del canal
//!    2  AMPERE_COMPUTE_B  el de computo
//!    3  el canal de GR0   y con el, su grupo
//! ```
//!
//! Nada mas: ni el cliente, ni el dispositivo, ni el espacio, ni el canal de
//! copia. Liberar uno de esos dejaria a BMO-X sin tarjeta hasta reiniciar,
//! que es lo que esto existe para evitar.

use crate::canal::GR;
use crate::objeto::CLIENTE;

/// `NV_VGPU_MSG_FUNCTION_FREE`.
pub const FREE: u32 = 10;
/// Lo que miden los datos: `NVOS00_PARAMETERS_v03_00`.
pub const DATOS: usize = 16;

/// **Lo unico que se puede liberar.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Liberable {
    TresDe,
    Computo,
    CanalGr,
}

impl Liberable {
    /// En el orden en que se liberan: los hijos antes que el canal.
    pub const TODOS: [Liberable; 3] = [Liberable::TresDe, Liberable::Computo, Liberable::CanalGr];

    /// Su asa.
    pub const fn asa(self) -> u32 {
        match self {
            Liberable::TresDe => crate::gr::forma_tresde().2,
            Liberable::Computo => crate::computo::forma().2,
            Liberable::CanalGr => GR.asa,
        }
    }

    /// El de la posicion `k` de [`Liberable::TODOS`].
    pub const fn de(k: u64) -> Option<Liberable> {
        match k {
            0 => Some(Liberable::TresDe),
            1 => Some(Liberable::Computo),
            2 => Some(Liberable::CanalGr),
            _ => None,
        }
    }
}

fn poner(d: &mut [u8], o: usize, v: u32) {
    d[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

fn u32_de(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// **Armar el `FREE` de `que`** en `hueco`, con el numero `numero`.
pub fn armar(hueco: &mut [u8], numero: u32, que: Liberable) -> Option<usize> {
    crate::orden::componer(hueco, numero, FREE, DATOS, |d| {
        poner(d, 0, CLIENTE);
        poner(d, 4, 0);
        poner(d, 8, que.asa());
        poner(d, 12, 0);
    })
}

/// **Los datos de un `FREE` pueden salir?** Solo nuestro cliente, el padre
/// a 0, el estado a 0, y una de las tres asas de [`Liberable`].
pub fn permitida(d: &[u8]) -> bool {
    d.len() == DATOS
        && u32_de(d, 0) == CLIENTE
        && u32_de(d, 4) == 0
        && u32_de(d, 12) == 0
        && Liberable::TODOS.iter().any(|l| l.asa() == u32_de(d, 8))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::rpc::{Mensaje, Suma, CABECERA};

    #[test]
    fn los_tres_y_nada_mas() {
        let asas: [u32; 3] = core::array::from_fn(|k| Liberable::TODOS[k].asa());
        assert_eq!(asas, [0xC797_0002, 0xC7C0_0002, 0xF1F0_0002], "AMPERE_B, AMPERE_COMPUTE_B y el canal de GR0");
        for l in Liberable::TODOS {
            let mut d = [0u8; DATOS];
            poner(&mut d, 0, CLIENTE);
            poner(&mut d, 8, l.asa());
            assert!(permitida(&d));
            let mut padre = d;
            poner(&mut padre, 4, 1);
            assert!(!permitida(&padre), "con padre, no");
            let mut otro = d;
            poner(&mut otro, 0, CLIENTE + 1);
            assert!(!permitida(&otro), "de otro cliente, no");
        }
        let mut copia = [0u8; DATOS];
        poner(&mut copia, 0, CLIENTE);
        poner(&mut copia, 8, crate::canal::COPIA.asa);
        assert!(!permitida(&copia), "el canal de copia NO se libera");
        let mut cliente = [0u8; DATOS];
        poner(&mut cliente, 0, CLIENTE);
        poner(&mut cliente, 8, CLIENTE);
        assert!(!permitida(&cliente), "el cliente NO se libera");
        assert!(!permitida(&[0u8; DATOS + 4]), "otra medida, no");
    }

    #[test]
    fn el_mensaje_armado_pasa_el_contrato_y_suma_cero() {
        let mut h = [0u8; 4096];
        let n = armar(&mut h, 77, Liberable::CanalGr).unwrap();
        assert_eq!(n, CABECERA + DATOS);
        let m = Mensaje::de(h[..CABECERA].try_into().unwrap());
        assert!(m.bien_formado());
        assert_eq!((m.funcion, m.numero, m.datos()), (FREE, 77, DATOS));
        let mut s = Suma::default();
        s.mas(&h[..n]);
        assert_eq!(s.valor(), 0);
        assert_eq!(crate::contrato::permitido(&h[..n]), Ok(FREE));
        assert_eq!(Liberable::de(2), Some(Liberable::CanalGr));
        assert_eq!(Liberable::de(3), None);
    }
}
