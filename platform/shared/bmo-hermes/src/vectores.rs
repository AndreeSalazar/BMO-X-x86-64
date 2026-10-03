//! **LOS VECTORES DE NOISE** -- la referencia de fuera, no la nuestra.
//!
//! Copiados de `cacophony.txt` (el banco de vectores de Noise que publican
//! cacophony y snow, dos implementaciones independientes; los dos ficheros son
//! identicos byte a byte, comprobado el 2026-10-03). Solo los dos patrones que
//! usa HERMES/1, con la suite que usa HERMES/1: 25519, AES-GCM y SHA-256.
//!
//! Cada vector trae las claves PRIVADAS de las dos puntas, el prologo, seis
//! mensajes (los primeros son el saludo; el resto, transporte alternando de
//! punta) y el `handshake_hash` final. Si un solo byte de lo nuestro difiere
//! del suyo, no estamos hablando Noise: estamos hablando algo parecido.

pub struct Vector {
    pub nombre: &'static str,
    pub prologo: &'static str,
    pub ini_estatica: &'static str,
    pub ini_efimera: &'static str,
    /// Solo IK: la publica del que responde, que el que inicia ya conoce.
    pub ini_remota: Option<&'static str>,
    pub res_estatica: &'static str,
    pub res_efimera: &'static str,
    pub hash_saludo: &'static str,
    /// `(carga, cifrado)` en orden de envio.
    pub mensajes: &'static [(&'static str, &'static str)],
}

pub const XX: Vector = Vector {
    nombre: "Noise_XX_25519_AESGCM_SHA256",
    prologo: "4a6f686e2047616c74",
    ini_estatica: "e61ef9919cde45dd5f82166404bd08e38bceb5dfdfded0a34c8df7ed542214d1",
    ini_efimera: "893e28b9dc6ca8d611ab664754b8ceb7bac5117349a4439a6b0569da977c464a",
    ini_remota: None,
    res_estatica: "4a3acbfdb163dec651dfa3194dece676d437029c62a408b4c5ea9114246e4893",
    res_efimera: "bbdb4cdbd309f1a1f2e1456967fe288cadd6f712d65dc7b7793d5e63da6b375b",
    hash_saludo: "1b7aefb1125762aa21a252890d00af54519638b76437444538f9a52f21e2e0dc",
    mensajes: &[
        ("4c756477696720766f6e204d69736573",
         "ca35def5ae56cec33dc2036731ab14896bc4c75dbb07a61f879f8e3afa4c79444c756477696720766f6e204d69736573"),
        ("4d757272617920526f746862617264",
         "95ebc60d2b1fa672c1f46a8aa265ef51bfe38e7ccb39ec5be34069f144808843757117acceb05bd7a45733bc22015c97a9d0cbaf41b80446d5988ff5127235d76b79eade70f473d6a4ef521fdcbeda5340d01e028ba793fc059f2724a83af05f12dda0448a7621a926b379a92477fd"),
        ("462e20412e20486179656b",
         "c90f1cf77eba4e50edb038991565e36c9758943a989229b6051244dc4fbecb6946744b401af2ee1a5881b65fbb87fd07cb6a328ececc9ce6ce84c399dc332d4fd521fa4bb7f467ce909395"),
        ("4361726c204d656e676572",
         "bc3fa77f6aca3e8466d7dc6bea10013e88a6a29add5132b461806c"),
        ("4a65616e2d426170746973746520536179",
         "250b01074cdfe0df2ecf8ccbf1737b15a2ddb5b52fd9a396604e9c793cee3b3bb9"),
        ("457567656e2042f6686d20766f6e2042617765726b",
         "449d4d433b3cdc3d02bf6fc881774b9df54366ebcffb9689bb13f14709822cd7ef42bcdb4d"),
    ],
};

pub const IK: Vector = Vector {
    nombre: "Noise_IK_25519_AESGCM_SHA256",
    prologo: "4a6f686e2047616c74",
    ini_estatica: "e61ef9919cde45dd5f82166404bd08e38bceb5dfdfded0a34c8df7ed542214d1",
    ini_efimera: "893e28b9dc6ca8d611ab664754b8ceb7bac5117349a4439a6b0569da977c464a",
    ini_remota: Some("31e0303fd6418d2f8c0e78b91f22e8caed0fbe48656dcf4767e4834f701b8f62"),
    res_estatica: "4a3acbfdb163dec651dfa3194dece676d437029c62a408b4c5ea9114246e4893",
    res_efimera: "bbdb4cdbd309f1a1f2e1456967fe288cadd6f712d65dc7b7793d5e63da6b375b",
    hash_saludo: "669c8640d9e42a3cda2f232f78597ceefb01daa6e3df81181ccce6fc6b5026bf",
    mensajes: &[
        ("4c756477696720766f6e204d69736573",
         "ca35def5ae56cec33dc2036731ab14896bc4c75dbb07a61f879f8e3afa4c79444e417bc55c7a8166c993356c1be41ef67818a292426f301556c7f26b21d25ddb097153891a9a956cff47b83e63ad8d701c1342c209cff1ca5ecd43402762ac249e3bd3a4c0a145fe07cb5dae28ea13a3"),
        ("4d757272617920526f746862617264",
         "95ebc60d2b1fa672c1f46a8aa265ef51bfe38e7ccb39ec5be34069f144808843af2ccf9972e22afc67aeafcd25162f7f98c363b7762e3e4cb7d272e39f27a5"),
        ("462e20412e20486179656b",
         "66acfc92e3197de166809e6d4d5d003dcc819a84bc3522ca53c9d9"),
        ("4361726c204d656e676572",
         "71f89aa6533a6de70b0826864dd75f60806ee40170c16290189eb3"),
        ("4a65616e2d426170746973746520536179",
         "4795a3423550c8bf00386bd496a3e2c76c10669d2a75ab8f79b5094c5412a25705"),
        ("457567656e2042f6686d20766f6e2042617765726b",
         "aa0bb39097555c918e40be82abc2b909eb79d9eb87adb07e268fc37323a6cf904fd01fb391"),
    ],
};
