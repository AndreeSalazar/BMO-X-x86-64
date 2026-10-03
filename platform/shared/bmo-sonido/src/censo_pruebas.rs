//! Las pruebas del censo: una cabecera de verdad por formato, los nombres que
//! mienten, y basura que no lo tumba.

use super::*;

extern crate std;
use std::vec;
use std::vec::Vec;

/// Una cabecera de WAV con `fmt ` de 16 bytes y un `data` que dice medir un
/// minuto: lo normal cuando solo se leen los primeros 64 bytes.
fn wav(codec: u16) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36u32 + 11_520_000).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&codec.to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes());
    v.extend_from_slice(&48_000u32.to_le_bytes());
    v.extend_from_slice(&192_000u32.to_le_bytes());
    v.extend_from_slice(&4u16.to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&11_520_000u32.to_le_bytes());
    v.extend_from_slice(&[0u8; 20]);
    v
}

fn ogg(codec: &[u8]) -> Vec<u8> {
    let mut v = b"OggS".to_vec();
    v.resize(28, 0);
    v.extend_from_slice(codec);
    v.resize(64, 0);
    v
}

#[test]
fn cada_formato_por_dentro() {
    let casos: &[(Vec<u8>, Tipo)] = &[
        (wav(1), Tipo::Wav),
        (b"ID3\x04\x00\x00\x00\x00\x00\x00".to_vec(), Tipo::Mp3),
        (b"fLaC\x00\x00\x00\x22".to_vec(), Tipo::Flac),
        (ogg(b"\x01vorbis"), Tipo::Vorbis),
        (ogg(b"OpusHead"), Tipo::Opus),
        (ogg(b"\x80theora"), Tipo::Ogg),
        (vec![0xFF, 0xF1, 0x50, 0x80], Tipo::Aac),
        (b"\x00\x00\x00\x20ftypM4A \x00\x00\x00\x00".to_vec(), Tipo::M4a),
        (b"FORM\x00\x00\x10\x00AIFFCOMM".to_vec(), Tipo::Aiff),
        (b"MThd\x00\x00\x00\x06".to_vec(), Tipo::Midi),
        (vec![0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6], Tipo::Wma),
        (b"\x7FELF\x02\x01".to_vec(), Tipo::Nada),
        (b"BEX\x00".to_vec(), Tipo::Nada),
    ];
    for (b, t) in casos {
        assert_eq!(tipo(b).0, *t, "{:?}", t);
    }
    // El MPEG por sincronismo: MP3, pero con la duda dicha.
    assert_eq!(tipo(&[0xFF, 0xFB, 0x90, 0x64]), (Tipo::Mp3, true));
    assert_eq!(tipo(&[0xFF, 0xF1]), (Tipo::Aac, false), "capa 00 es ADTS, no MP3");
}

#[test]
fn lo_oficial_y_lo_que_no() {
    assert_eq!(ficha(&wav(1), "cancion.wav").veredicto, Veredicto::Suena, "PCM: suena");
    assert_eq!(ficha(&wav(0xFFFE), "cancion.wav").veredicto, Veredicto::Suena, "extensible tambien");
    assert!(matches!(ficha(&wav(3), "x.wav").veredicto, Veredicto::Oficial(_)), "coma flotante");
    assert!(matches!(ficha(&wav(0x55), "x.wav").veredicto, Veredicto::Oficial(m) if m.contains("MP3")));
    assert!(matches!(ficha(&wav(0x161), "x.wav").veredicto, Veredicto::NoOficial(_)), "un codec raro dentro");
    assert!(matches!(ficha(b"ID3\x04\x00", "a.mp3").veredicto, Veredicto::Oficial(m) if m.contains("M2")));
    assert!(matches!(ficha(b"MThd", "a.mid").veredicto, Veredicto::NoOficial(m) if m.contains("sintetizador")));
    assert!(matches!(ficha(&[0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11], "a.wma").veredicto, Veredicto::NoOficial(_)));
    assert_eq!(ficha(b"hola", "notas.txt").veredicto, Veredicto::NoEsSonido);
    assert_eq!(Veredicto::Suena.palabra(), "SUENA");
}

#[test]
fn el_nombre_que_miente() {
    let f = ficha(&wav(1), "cancion.mp3");
    assert!(f.nombre_miente, "un WAV que se llama .mp3");
    assert!(!ficha(&wav(1), "CANCION.WAV").nombre_miente, "las mayusculas no mienten");
    assert!(!ficha(&wav(1), "sin_extension").nombre_miente, "sin extension no hay nada que contradecir");
    assert!(ficha(b"\x7FELF", "musica.mp3").nombre_miente, "un ejecutable que dice ser un mp3");
    assert!(!ficha(b"\x7FELF", "programa").nombre_miente);
    assert!(!ficha(&ogg(b"OpusHead"), "voz.ogg").nombre_miente, "Opus en .ogg vale");
    assert!(!ficha(b"\x00\x00\x00\x20ftypM4A ", "a.m4a").nombre_miente);
}

#[test]
fn la_cuenta() {
    let mut c = Cuenta::default();
    for (b, n) in [(wav(1), "a.wav"), (wav(1), "b.mp3"), (b"ID3\x04".to_vec(), "c.mp3"), (b"MThd".to_vec(), "d.mid"), (b"texto".to_vec(), "e.txt"), (b"texto".to_vec(), "f.flac")] {
        c.meter(&ficha(&b, n));
    }
    assert_eq!((c.ficheros, c.suenan, c.oficiales, c.no_oficiales, c.mienten), (6, 2, 1, 1, 2));
    assert_eq!(c.sonidos(), 4);
    assert_eq!(c.por_tipo[Tipo::Wav as usize], 2);
}

/// Bytes hechos a mala idea: con cabeceras buenas cortadas, largos de trozo
/// que dan la vuelta, y nombres raros. Nada de esto puede tumbar el censo:
/// lo va a leer de un disco entero, y un disco tiene de todo.
#[test]
fn la_basura_no_lo_tumba() {
    let buenas: Vec<Vec<u8>> = std::vec![wav(1), wav(0x55), ogg(b"OpusHead"), b"ID3\x04\x00\x00".to_vec(), b"FORM\x00\x00\x00\x00AIFF".to_vec()];
    let refs: Vec<&[u8]> = buenas.iter().map(|v| v.as_slice()).collect();
    bmo_hostile::attack("censo::ficha", 0xC0DE_0003, 30_000, &refs, CABECERA, |b| {
        let f = ficha(b, "x.wav");
        let _ = (f.veredicto.palabra(), f.veredicto.motivo(), f.tipo.nombre());
        let _ = ficha(b, "");
        let _ = ficha(b, ".");
        let _ = ficha(b, "a.\u{00F1}\u{00F1}");
        let _ = ficha(b, "muy.largaextension");
    });
}
