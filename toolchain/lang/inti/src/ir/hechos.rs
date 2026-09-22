//! **Los HECHOS de una funcion** -- lo que el emisor necesita saber de sus
//! locales para repartirles sitio, calculado AQUI y sin nombrar una maquina.
//!
//! `PLAN_EL_TROQUEL.md` 12.1: *"el frontend MAESTRO dice HECHOS sobre el
//! arbol; el REGISTRO dice NOMBRES sobre la maquina; y lo que cruza es un
//! hecho hacia abajo, nunca un nombre hacia arriba."* Esto es I1: los tres
//! hechos que el troquel de BMO C ya usa (`decidir/registros.rs`), dichos
//! sobre la IR de INTI:
//!
//! ```text
//!    tomadas   a que locales se les toma la DIRECCION. Una local marcada
//!              tiene que vivir en memoria: alguien va a leerla por ahi
//!    peso      cuantas veces se usa cada local, y un uso dentro de un bucle
//!              vale mas -- es el orden en el que merecen un sitio rapido
//!    pisa      si el cuerpo LLAMA a alguien. El que llama no puede fiarse de
//!              lo que una llamada puede tocar
//! ```
//!
//! ** Ni un numero de registro, ni un ancho de palabra: `tests/agnostico.rs`
//! vigila este fichero como los demas. Y se calcula sobre la IR y no sobre el
//! arbol porque la IR ya tiene las locales numeradas y los saltos explicitos:
//! un bucle es un salto hacia atras, y eso se ve sin saber que es un `mientras`.
//!
//! [!] El `match` de abajo NO tiene comodin, a proposito y por el mismo motivo
//! que `tramos_de_vida` en el emisor: una instruccion nueva en la IR no compila
//! hasta que alguien diga aqui si toca locales.

use super::forma::{FuncionIr, Instr, Local, Valor};

/// Cuanto vale un uso por cada nivel de bucle que lo envuelve. Un uso a
/// profundidad 2 cuenta `1 + 2 * POR_BUCLE`.
///
/// BMO C cuenta el cuerpo de un bucle DOBLE; aqui se cuenta por nivel porque
/// la IR los tiene contados. El numero de verdad lo pondra el metro de INTI.
pub const POR_BUCLE: u32 = 3;

/// Los hechos de una funcion. Ver la cabecera.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hechos {
    /// Locales a las que se les toma la direccion, sin repetir y en orden.
    pub tomadas: Vec<Local>,
    /// El peso de cada local, por indice (`peso.len() == locales`).
    pub peso: Vec<u32>,
    /// El cuerpo llama a alguien.
    pub pisa: bool,
}

impl Hechos {
    /// Una local que puede vivir fuera del marco: se usa y nadie la marca.
    pub fn candidata(&self, l: Local) -> bool {
        !self.tomadas.contains(&l) && self.peso.get(l.0 as usize).copied().unwrap_or(0) > 0
    }
}

impl FuncionIr {
    /// Los hechos de esta funcion, calculados sobre sus instrucciones.
    pub fn hechos(&self) -> Hechos {
        let n = self.locales as usize;
        let profundidad = profundidad_por_instruccion(&self.instrucciones);
        let mut h = Hechos {
            tomadas: Vec::new(),
            peso: vec![0; n],
            pisa: false,
        };
        let usa = |v: &Valor, i: usize, peso: &mut Vec<u32>| {
            if let Valor::Local(l) = v {
                if let Some(p) = peso.get_mut(l.0 as usize) {
                    *p += 1 + POR_BUCLE * profundidad[i];
                }
            }
        };
        for (i, instr) in self.instrucciones.iter().enumerate() {
            match instr {
                Instr::Mueve { origen, .. } => usa(origen, i, &mut h.peso),
                Instr::Binaria { izquierda, derecha, .. } => {
                    usa(izquierda, i, &mut h.peso);
                    usa(derecha, i, &mut h.peso);
                }
                Instr::Unaria { valor, .. } => usa(valor, i, &mut h.peso),
                Instr::Convierte { valor, .. } => usa(valor, i, &mut h.peso),
                Instr::Comprueba { sobre, contra, .. } => {
                    usa(sobre, i, &mut h.peso);
                    if let Some(c) = contra {
                        usa(c, i, &mut h.peso);
                    }
                }
                Instr::Direccion { .. } | Instr::MontonDeLaTarea { .. } | Instr::Etiqueta(_) | Instr::Salta(_) => {}
                Instr::DireccionDeLocal { local, .. } => {
                    if !h.tomadas.contains(local) {
                        h.tomadas.push(*local);
                    }
                }
                Instr::Lee { direccion, .. } => usa(direccion, i, &mut h.peso),
                Instr::Escribe { direccion, valor, .. } => {
                    usa(direccion, i, &mut h.peso);
                    usa(valor, i, &mut h.peso);
                }
                // Escribir una local tambien es usarla: cuesta lo mismo que
                // leerla, y un contador de bucle se escribe en cada vuelta.
                Instr::Guarda { destino, valor } => {
                    usa(valor, i, &mut h.peso);
                    if let Some(p) = h.peso.get_mut(destino.0 as usize) {
                        *p += 1 + POR_BUCLE * profundidad[i];
                    }
                }
                Instr::Devuelve(v) => {
                    if let Some(v) = v {
                        usa(v, i, &mut h.peso);
                    }
                }
                Instr::SaltaSi { cond, .. } => usa(cond, i, &mut h.peso),
                Instr::Llama { que, argumentos, .. } => {
                    h.pisa = true;
                    usa(que, i, &mut h.peso);
                    for a in argumentos {
                        usa(a, i, &mut h.peso);
                    }
                }
                Instr::Metal { argumentos, .. } => {
                    for a in argumentos {
                        usa(a, i, &mut h.peso);
                    }
                }
            }
        }
        h
    }
}

/// **Cuantos bucles envuelven cada instruccion.**
///
/// Un bucle es un salto HACIA ATRAS: una etiqueta que ya paso y un `Salta` o
/// `SaltaSi` que vuelve a ella. Todo lo que hay entre la etiqueta y el salto
/// esta dentro. Dos bucles anidados son dos saltos atras que se contienen, y
/// las instrucciones de dentro del interior suman los dos.
fn profundidad_por_instruccion(instrucciones: &[Instr]) -> Vec<u32> {
    let mut donde: Vec<(u32, usize)> = Vec::new();
    for (i, instr) in instrucciones.iter().enumerate() {
        if let Instr::Etiqueta(e) = instr {
            donde.push((e.0, i));
        }
    }
    let sitio = |e: u32| donde.iter().find(|(x, _)| *x == e).map(|(_, i)| *i);
    let mut prof = vec![0u32; instrucciones.len()];
    for (i, instr) in instrucciones.iter().enumerate() {
        let destinos: Vec<u32> = match instr {
            Instr::Salta(e) => vec![e.0],
            Instr::SaltaSi { cierto, falso, .. } => vec![cierto.0, falso.0],
            _ => Vec::new(),
        };
        for d in destinos {
            if let Some(ini) = sitio(d) {
                if ini <= i {
                    for p in &mut prof[ini..=i] {
                        *p += 1;
                    }
                }
            }
        }
    }
    prof
}

#[cfg(test)]
mod pruebas {
    use super::super::forma::*;
    use super::*;

    fn f(locales: u32, instrucciones: Vec<Instr>) -> FuncionIr {
        FuncionIr {
            nombre: "f".into(),
            parametros: 0,
            locales,
            medidas_locales: Vec::new(),
            temporales: 4,
            instrucciones,
            sin_ancho: 0,
        }
    }

    #[test]
    fn un_uso_pesa_uno_y_escribir_tambien() {
        let h = f(
            2,
            vec![
                Instr::Mueve { destino: Temporal(0), origen: Valor::Local(Local(0)) },
                Instr::Guarda { destino: Local(1), valor: Valor::Temporal(Temporal(0)) },
                Instr::Devuelve(Some(Valor::Local(Local(1)))),
            ],
        )
        .hechos();
        assert_eq!(h.peso, vec![1, 2]);
        assert!(h.tomadas.is_empty());
        assert!(!h.pisa);
        assert!(h.candidata(Local(0)) && h.candidata(Local(1)));
    }

    #[test]
    fn un_uso_dentro_de_un_bucle_pesa_mas() {
        // L0: t0 = l0 ; salta_si t0 -> L0 / L1 ; L1: devuelve l1
        let h = f(
            2,
            vec![
                Instr::Etiqueta(Etiqueta(0)),
                Instr::Mueve { destino: Temporal(0), origen: Valor::Local(Local(0)) },
                Instr::SaltaSi { cond: Valor::Temporal(Temporal(0)), cierto: Etiqueta(0), falso: Etiqueta(1) },
                Instr::Etiqueta(Etiqueta(1)),
                Instr::Devuelve(Some(Valor::Local(Local(1)))),
            ],
        )
        .hechos();
        assert_eq!(h.peso[0], 1 + POR_BUCLE, "dentro del bucle");
        assert_eq!(h.peso[1], 1, "fuera del bucle");
    }

    #[test]
    fn dos_bucles_anidados_suman() {
        let h = f(
            1,
            vec![
                Instr::Etiqueta(Etiqueta(0)),
                Instr::Etiqueta(Etiqueta(1)),
                Instr::Mueve { destino: Temporal(0), origen: Valor::Local(Local(0)) },
                Instr::Salta(Etiqueta(1)),
                Instr::Salta(Etiqueta(0)),
            ],
        )
        .hechos();
        assert_eq!(h.peso[0], 1 + 2 * POR_BUCLE);
    }

    #[test]
    fn un_salto_hacia_delante_no_es_un_bucle() {
        let h = f(
            1,
            vec![
                Instr::Salta(Etiqueta(0)),
                Instr::Mueve { destino: Temporal(0), origen: Valor::Local(Local(0)) },
                Instr::Etiqueta(Etiqueta(0)),
            ],
        )
        .hechos();
        assert_eq!(h.peso[0], 1);
    }

    #[test]
    fn tomar_la_direccion_deja_a_la_local_fuera() {
        let h = f(
            2,
            vec![
                Instr::DireccionDeLocal { destino: Temporal(0), local: Local(0) },
                Instr::DireccionDeLocal { destino: Temporal(1), local: Local(0) },
                Instr::Mueve { destino: Temporal(2), origen: Valor::Local(Local(1)) },
            ],
        )
        .hechos();
        assert_eq!(h.tomadas, vec![Local(0)], "una vez, aunque se tome dos");
        assert!(!h.candidata(Local(0)));
        assert!(h.candidata(Local(1)));
    }

    #[test]
    fn una_llamada_pisa() {
        let h = f(
            1,
            vec![Instr::Llama {
                destino: None,
                que: Valor::Nombre("g".into()),
                argumentos: vec![Valor::Local(Local(0))],
            }],
        )
        .hechos();
        assert!(h.pisa);
        assert_eq!(h.peso[0], 1);
    }

    #[test]
    fn una_local_que_nadie_usa_no_es_candidata() {
        let h = f(1, vec![Instr::Devuelve(None)]).hechos();
        assert_eq!(h.peso, vec![0]);
        assert!(!h.candidata(Local(0)));
    }
}
