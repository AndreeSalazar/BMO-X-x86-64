//! **BMO HERMES** -- el protocolo HERMES/1: como hablan dos BMO-X, sin
//! servidor de nadie.
//!
//! generacion: hijo -- recibe bytes, claves y la hora; no sabe de sockets, ni de discos, ni de quien lo usa. Debajo solo tiene a `bmo-cripto`
//! capa: puro -- ni un `unsafe`, ni un aparato: lo usan la PUERTA HERMES y el banco
//!
//! [carril]  AMARILLO  lee lo que manda otra maquina
//! [cuesta]  DATO      un mensaje mal leido muestra, guarda o deja pasar lo que no es
//! [riesgo]  AJENO     cada byte lo escribe la otra punta
//!
//! # H2 de `docs/plan/PLAN_HERMES.md` (2026-10-03)
//!
//! El propietario: *"vamos a aplicar [...] pero primero la base"*. La base de
//! HERMES es esto, y se prueba entera en el anfitrion antes de tocar el metal:
//!
//! ```text
//!    noise    el SALUDO: Noise XX la primera vez, IK con un amigo, sobre
//!             X25519, AES-256-GCM y SHA-256 de `bmo-cripto`. Contra los
//!             vectores publicos de Noise, byte a byte
//!    trama    lo que viaja DENTRO: TEXTO, ZUMBIDO, GUINO, OFERTA, SI, NO,
//!             TROZO, PIDE y REACCION, en lista blanca
//!    marco    como se cortan los mensajes en un flujo de TCP
//!    huella   lo que se lee en voz alta para hacerse amigos
//!    grifo    un ZUMBIDO cada 10 segundos por amigo, como mucho
//! ```
//!
//! # Y lo que NO es
//!
//! **No es la ANTENA.** La ANTENA (`bmo-antena`, `ANTENA/1`) es como BMO-X le
//! pide la web a un movil. HERMES es BMO-X con BMO-X. No comparten ni una
//! linea, y este crate no la nombra (el propietario, 02-10: *"no lo
//! mezcles"*).
//!
//! **No abre sockets ni lee discos.** Lo usa la PUERTA HERMES (H4), que es la
//! unica jaula con autoridad RED, y la app de F3 no lo enlaza para hablar con
//! el cable: le llegan los mensajes ya abiertos por una cola.
//!
//! **No firma.** La identidad es una clave X25519; ver la cabecera de
//! [`noise`].

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

/// **El saludo y la conversacion cifrada.**
pub mod noise;
/// **Los mensajes de HERMES/1**, en lista blanca.
pub mod trama;
/// **El marco**: la medida delante de cada mensaje, en el flujo de TCP.
pub mod marco;
/// **La huella**: una clave publica en 16 grupos que se leen en voz alta.
pub mod huella;
/// **El grifo del ZUMBIDO.**
pub mod grifo;

#[cfg(test)]
mod vectores;

/// **Por que no.** Uno por motivo: "se descarto" no es un diagnostico (L6i).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rechazo {
    // -- el saludo y el cifrado --
    /// Le tocaba a la otra punta.
    FueraDeTurno,
    /// El saludo ya habia terminado.
    SaludoTerminado,
    /// Se pidio la conversacion antes de acabar el saludo.
    SaludoSinTerminar,
    /// Un error anterior mato el saludo; se tira y se empieza otro.
    Roto,
    /// Un IK que inicia sin la clave de su amigo.
    FaltaRemota,
    /// Una clave remota donde el patron no la lleva.
    SobraRemota,
    /// Un intercambio dio ceros: la otra clave era de orden chico.
    ClaveDebil,
    /// La etiqueta de AES-GCM no cuadra: tocado, repetido o de otro.
    Etiqueta,
    /// El contador llego al final: hay que saludar otra vez.
    Agotado,
    // -- las medidas --
    /// Falta un trozo: el mensaje se acaba antes de tiempo.
    Corto,
    /// Mas largo de lo que cabe en un mensaje de Noise.
    Largo,
    /// El buffer de quien llama no tiene sitio.
    SinSitio,
    /// Un mensaje o un marco vacio.
    Vacio,
    // -- lo que viaja dentro --
    /// Un verbo que no esta en la lista.
    Verbo,
    /// El cuerpo no mide lo que su verbo dice.
    Medida,
    /// Bytes que no son UTF-8.
    Utf8,
    /// Un caracter de control en un TEXTO.
    Control,
    /// Un guino fuera del catalogo.
    Guino,
    /// Una OFERTA de cero bytes o de mas de lo que cabe.
    Bytes,
    /// Un nombre con lo que no puede llevar.
    Nombre,
    /// Un PIDE de algo que no existe.
    Que,
    /// Una bandera que no es 0 ni 1.
    Bandera,
    /// Una REACCION con texto en vez de un emoji.
    NoEsEmoji,
}
