//! 17 y 20 de la pila A (07-10): los operandos que no son lo que se
//! esperaba (hijo de `programa`, aparte por la regla de las 1.000 lineas).

use super::*;
use alloc::vec;

fn compilador(valores: Vec<Valor>) -> Compilador {
    Compilador { valores, iniciales: Vec::new(), ops: Vec::new(), entradas: 0, salidas: 0, lee: 0, filas_cb: 0, recursos: Vec::new(), ranuras: Ranuras::default(), bloques: Default::default(), literales: Vec::new(), compartida: 0 }
}

/// 17 de la pila A (07-10): un operando float que llega como `undef`
/// es 0.0, y uno apuntado como entero son sus bits (el IR tiene tipos);
/// lo que no es un numero lo DICE con su nombre (20). La prueba que
/// dice NO: un handle no se usa como float.
#[test]
fn un_float_undef_es_cero_y_lo_que_no_es_numero_se_nombra() {
    let mut c = compilador(vec![Valor::Indefinido, Valor::Bits(7), Valor::Entero(3), Valor::Textura(0), Valor::Nada, Valor::Cuatro(9)]);
    let r = c.float(0).unwrap();
    assert_eq!(c.iniciales[r as usize].to_bits(), 0, "undef: 0.0");
    assert_eq!(c.float(1).ok(), Some(7), "los bits de un float apuntado como entero");
    let r = c.float(2).unwrap();
    assert_eq!(c.iniciales[r as usize].to_bits(), 3);
    assert_eq!(c.float(3), Err(NoPrograma::Forma("un operando que es el handle de un recurso")));
    assert_eq!(c.float(4), Err(NoPrograma::Forma("un operando float que es una constante que la casa no lee (half, double, o un cast constante)")));
    assert_eq!(super::super::estructura::bits(&mut c, 5), Err(NoPrograma::Forma("un operando que es un ResRet o un CBufRet entero (sin extractvalue)")));
    assert_eq!(super::super::estructura::bits(&mut c, 9), Err(NoPrograma::Forma("un operando que todavia no existe (una referencia hacia delante fuera de un phi)")));
}
