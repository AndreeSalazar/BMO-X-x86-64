//! INSPECCION NVIDIA -- que significan las copias preparadas, sin enviarlas.
//! capa: puro -- bytes y palabras dados por el llamador; sin aparatos
//! [carril] VERDE
//! [consumo] NADA -- solamente consultas explicitas
//!
//! Por que existe separado: un lector no debe adquirir la autoridad del
//! motor que ejecuta. El formato FIFO y sus metodos son NVIDIA; no entran
//! en VERRANO ni en la lente comun. Otra GPU trae otro interprete.
//! Contrato, fuentes y limites: `../../INSPECCION_NVIDIA.md`.

pub mod empuje;
pub mod metodos;
/// LOGINIT/LOGINTR/LOGRM: evidencia copiada y diccionario ELF, sin aparatos.
pub mod libos;

/// Referencia reproducible: el mismo constructor de VERRANO V0, un
/// triangulo, sin escalera. Direcciones PLANEADAS, no leidas del aparato.
pub fn verrano_template() -> crate::cubo::Ordenes {
    let v = crate::cubo::Ventana { x0: 0, y0: 0, va: crate::pantalla::VA,
        fila: 1280*4, rgb: false, flotante: crate::destino::Flotante::No,
        ancho: 1280, alto: 720 };
    crate::tuberia::ordenes_con(&v, 1, true)
}

/// Explica metodos solo bajo el contexto BMO-X/AMPERE_B del ejemplo.
pub fn explain(out: &mut impl core::fmt::Write, c: empuje::Command, m: metodos::Meaning) -> core::fmt::Result {
    writeln!(out, "+{:04x} sub={} metodo={:04x} dato={:08x} [{}] {}", c.word*4, c.subchannel, c.method, c.value, m.track.label(), m.name)?;
    writeln!(out, "  motivo: {}", m.why)
}

#[cfg(test)]
mod pruebas;
