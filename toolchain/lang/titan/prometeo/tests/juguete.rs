//! ** UNA TARJETA DE JUGUETE ENTRA POR EL CONTRATO DE PROMETEO SIN TOCAR
//! TITAN++ (LB3 de `docs/plan/PLAN_LAS_LIBRERIAS.md`, 08-10). Esta prueba
//! vive FUERA del crate y solo ve lo publico: si una tarjeta nueva tuviera que
//! cambiar el emisor de GPU de TITAN++, esto no compilaria.
//!
//! La tarjeta: su "codigo maquina" es un numero de cajon (guarda cada Programa
//! que se le da), su simulador es el interprete de la casa, su juez un techo
//! de operaciones, y no sabe dividir -- un LIMITE, dicho en sus palabras. Y al
//! lado de la 3060 de verdad: dos tarjetas, AISLADAS, los mismos bits.
//!
//! [!] Lo que esta prueba NO dice: que el contrato baste para una tarjeta de
//! verdad distinta de la 3060. Eso lo dira LB4 (la CPU, con codigo real) y,
//! el dia que llegue, LB10.

use bmo_prometeo::programa::Op;
use bmo_prometeo::{Codigo, Ficha, NoEmite, Para, Programa, Tarjeta};
use bmo_proton_x_sm86::tarjeta::SM86;
use bmo_titan_front::calc::DeviceNo;
use bmo_titan_front::Code;
use bmo_titan_prometeo::{kernels, Oracle};
use std::sync::Mutex;

struct Juguete {
    cajones: Mutex<Vec<Programa>>,
    techo: usize,
}

impl Juguete {
    fn nueva(techo: usize) -> Self {
        Juguete { cajones: Mutex::new(Vec::new()), techo }
    }

    fn guardados(&self) -> usize {
        self.cajones.lock().unwrap().len()
    }
}

impl Tarjeta for Juguete {
    fn ficha(&self) -> Ficha {
        Ficha { nombre: "la tarjeta de juguete", lengua: "JUGUETE", registros: 1 << 16 }
    }

    fn emitir(&self, p: &Programa, _para: Para) -> Result<Codigo, NoEmite> {
        if let Some(op) = p.ops.iter().position(|o| matches!(o, Op::Div { .. })) {
            return Err(NoEmite::Limite {
                op,
                que: "una division que la tarjeta de juguete no sabe".into(),
                por_que: "la tarjeta de juguete no tiene division".into(),
            });
        }
        let mut cajones = self.cajones.lock().unwrap();
        cajones.push(p.clone());
        Ok(Codigo { bytes: ((cajones.len() - 1) as u32).to_le_bytes().to_vec(), instrucciones: p.ops.len(), registros: p.iniciales.len() as u32 })
    }

    fn juzgar(&self, c: &Codigo, _para: Para) -> Result<(), String> {
        if c.instrucciones > self.techo {
            return Err(format!("{} operaciones, y su techo es {}", c.instrucciones, self.techo));
        }
        Ok(())
    }

    fn simular(&self, c: &Codigo, entradas: &[[u32; 4]], salidas: usize) -> Result<Vec<[u32; 4]>, String> {
        let cajon = c.bytes.get(..4).and_then(|b| b.try_into().ok()).map(u32::from_le_bytes).ok_or("no es un codigo de la tarjeta de juguete")? as usize;
        let p = self.cajones.lock().unwrap().get(cajon).cloned().ok_or("un cajon que no existe")?;
        let e: Vec<[f32; 4]> = entradas.iter().map(|e| e.map(f32::from_bits)).collect();
        let mut s = vec![[0.0f32; 4]; salidas];
        let mut regs = Vec::new();
        p.correr(&e, &[], &mut s, &mut regs);
        Ok(s.iter().map(|o| o.map(f32::to_bits)).collect())
    }
}

const TOML: &str = "[package]\nname = \"x\"\n[permissions]\ngpu = \"compute\"\n";

fn lee(p: &str) -> Option<String> {
    (p == "Titan.toml").then(|| TOML.to_string())
}

const MEZCLA: &str = "mod main \"x\"\ngpu fn mezcla(a: f32, b: f32) -> f32\n    return (a + b) * 0.25 - a\nfn main()\n    let xs: [f32; 3] = [1.0, 0.1, -7.5]\n    let ys: [f32; 3] = [2.0, 0.2, 1.25]\n    let r = mezcla(xs, ys)\n    for x in r\n        print(round(x, 9))\n";

/// ** ENTRA SIN TOCAR TITAN++: el calculo corre la gpu fn por la tarjeta de
/// juguete (su emisor, su juez, su simulador, la bateria entera) y el
/// programa dice lo mismo que el calculo solo.
#[test]
fn a_toy_card_enters_through_the_contract_without_touching_titan() {
    let juguete = Juguete::nueva(1000);
    let plain = bmo_titan_front::lower_package("src/main.titan", MEZCLA, &mut lee).unwrap();
    let mut oracle = Oracle::new(&[&juguete]);
    let ran = bmo_titan_front::lower_package_with("src/main.titan", MEZCLA, &mut lee, Some(&mut oracle)).unwrap();
    assert_eq!(oracle.written(), 1, "the oracle wrote, judged and ran the gpu fn on the toy card");
    assert_eq!(plain.flat, ran.flat, "the toy card gives the cells of the calculation");
    let ks = kernels(&plain, &[&juguete]).unwrap();
    assert_eq!(ks.len(), 1);
    assert_eq!(ks[0].tarjeta.ficha().lengua, "JUGUETE");
    assert!(juguete.guardados() >= 2, "its emitter wrote the oracle's code and the travelling one");
}

/// ** DOS TARJETAS, AISLADAS, LOS MISMOS BITS: la 3060 de verdad y la de
/// juguete, una al lado de la otra. Cada una con su codigo, su juez y su
/// simulador; ninguna sabe de la otra, y las celdas son las mismas.
#[test]
fn two_cards_side_by_side_give_the_same_bits() {
    let juguete = Juguete::nueva(1000);
    let plain = bmo_titan_front::lower_package("src/main.titan", MEZCLA, &mut lee).unwrap();
    let mut oracle = Oracle::new(&[&SM86, &juguete]);
    let ran = bmo_titan_front::lower_package_with("src/main.titan", MEZCLA, &mut lee, Some(&mut oracle)).unwrap();
    assert_eq!(plain.flat, ran.flat);
    let ks = kernels(&plain, &[&SM86, &juguete]).unwrap();
    let lenguas: Vec<&str> = ks.iter().map(|k| k.tarjeta.ficha().lengua).collect();
    assert_eq!(lenguas, ["SM86", "JUGUETE"]);
    assert_ne!(ks[0].viaje, ks[1].viaje, "each card, its own code");
}

/// ** EL LIMITE ES DE LA TARJETA, EN SUS PALABRAS: la division general, con
/// la de juguete sola, es el NO del programa en la linea y la columna de la
/// division -- y lo que dice es lo suyo, no lo de la 3060.
#[test]
fn a_limit_of_a_toy_card_is_the_program_s_no_in_its_words() {
    let src = "mod main \"x\"\ngpu fn tercio(a: f32) -> f32\n    return a / 3.0\nfn main()\n    let xs: [f32; 1] = [1.0]\n    let r = tercio(xs)\n    print(round(r[0], 2))\n";
    let juguete = Juguete::nueva(1000);
    let mut oracle = Oracle::new(&[&juguete]);
    let no = bmo_titan_front::lower_package_with("src/main.titan", src, &mut lee, Some(&mut oracle)).unwrap_err();
    assert_eq!((no.code, no.line, no.col), (Code::GpuBody, 3, 14), "{:?}", no);
    assert!(no.what.contains("tarjeta de juguete") && no.why.starts_with("la tarjeta de juguete no tiene division"), "{:?}", no);
    assert!(no.why.contains("LI2g") && no.how.contains("potencia de dos"), "{:?}", no);
    assert!(!no.what.contains("3060") && !no.why.contains("3060"), "{:?}", no);
}

/// ** SU JUEZ DICE QUE NO, y es un FALLO, no el NO del programa: el juez de
/// la de juguete tiene un techo de 2 operaciones y la gpu fn tiene mas. No hay
/// kernel, y el fallo dice quien lo dijo.
#[test]
fn the_toy_judge_says_no_and_it_is_a_failure_not_the_program_s() {
    let juguete = Juguete::nueva(2);
    let m = bmo_titan_front::lower_package("src/main.titan", MEZCLA, &mut lee).unwrap();
    match kernels(&m, &[&juguete]) {
        Err(DeviceNo::Failure(why)) => assert!(why.contains("el juez de la tarjeta de juguete dijo que no") && why.contains("su techo es 2"), "{}", why),
        other => panic!("a failure of its judge, not {:?}", other.map(|k| k.len())),
    }
}

/// ** SIN TARJETAS NO HAY gpu fn: quien arma la herramienta no dio ninguna, y
/// se dice -- una gpu fn no se escribe a ciegas.
#[test]
fn without_cards_a_gpu_fn_is_not_written_blind() {
    let m = bmo_titan_front::lower_package("src/main.titan", MEZCLA, &mut lee).unwrap();
    match kernels(&m, &[]) {
        Err(DeviceNo::Failure(why)) => assert!(why.contains("no hay ninguna tarjeta"), "{}", why),
        other => panic!("{:?}", other.map(|k| k.len())),
    }
    let mut oracle = Oracle::new(&[]);
    let no = bmo_titan_front::lower_package_with("src/main.titan", MEZCLA, &mut lee, Some(&mut oracle)).unwrap_err();
    assert!(no.why.contains("no hay ninguna tarjeta") && !no.what.contains("3060"), "{:?}", no);
}
