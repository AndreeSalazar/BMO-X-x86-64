//! **La matriz del EMISOR**: cada `.spv` de una carpeta que el juez acepta, por
//! el oraculo Y emitido a x86-64 en el emulador, con los mismos datos (buffers
//! a cero, un grupo de trabajo). Tienen que acabar igual, bit a bit.
//!
//! Herramienta del ANFITRION (usa `std`); la biblioteca sigue `no_std`. La
//! carpeta de siempre es la del banco de Naga, FUERA del repo:
//!
//!     cargo run -q -p bmo-spirv-x86-64 --example census_emit -- <carpeta de .spv>

use std::collections::BTreeMap;
use std::fs;

use bmo_lower::emu::{run, Machine};
use bmo_spirv_front::table::op;
use bmo_spirv_front::{read, validate, workspace_words, Buffer, Interpreter, Module, Reason};
use bmo_spirv_x86_64::{emit, tables_words, trap_reason, IDS_WORDS};

const PALABRAS: usize = 1 << 12;
const FUEL: u64 = 200_000;

fn buffers_de(m: &Module) -> Vec<(u32, u32)> {
    let deco = |id: u32, d: u32| {
        m.instructions().find(|i| i.opcode == op::OpDecorate && i.op(1) == id && i.op(2) == d).map(|i| i.op(3)).unwrap_or(0)
    };
    let mut v: Vec<(u32, u32)> = m
        .instructions()
        .take_while(|i| i.opcode != op::OpFunction)
        .filter(|i| i.opcode == op::OpVariable && (i.op(3) == 2 || i.op(3) == 12))
        .map(|i| (deco(i.op(2), 34), deco(i.op(2), 33)))
        .collect();
    v.dedup();
    v
}

type Salida = Result<(), Reason>;

fn oraculo(m: &Module, pares: &[(u32, u32)], datos: &mut [Vec<u32>]) -> Salida {
    let mut ws = vec![0u32; workspace_words(m)];
    let mut it = Interpreter::new(m, &mut ws).map_err(|e| e.reason)?;
    let mut bufs: Vec<Buffer> =
        pares.iter().zip(datos.iter_mut()).map(|(&(set, binding), d)| Buffer { set, binding, data: d }).collect();
    it.dispatch([1, 1, 1], &mut bufs, FUEL).map(|_| ()).map_err(|t| t.reason)
}

fn emulado(m: &Module, pares: &[(u32, u32)], datos: &mut [Vec<u32>]) -> Result<Salida, Reason> {
    let mut tablas = vec![0u32; tables_words(m)];
    let mut code = vec![0u8; 1 << 22];
    let p = emit(m, &mut tablas, &mut code).map_err(|e| e.reason)?;
    code.truncate(p.code_len);
    let mut mq = Machine::new(code);
    let marco = mq.load_data(&vec![0u8; p.frame_words * 4 + 4]);
    let mut dirs = Vec::new();
    let mut tabla = Vec::new();
    for &par in &p.buffers[..p.n_buffers] {
        let k = pares.iter().position(|&x| x == par).unwrap();
        let bytes: Vec<u8> = datos[k].iter().flat_map(|x| x.to_le_bytes()).collect();
        let addr = mq.load_data(&bytes);
        dirs.push((k, addr));
        tabla.extend_from_slice(&addr.to_le_bytes());
        tabla.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    }
    tabla.extend_from_slice(&[0u8; 16]);
    let t = mq.load_data(&tabla);
    let ids = mq.load_data(&[0u8; IDS_WORDS * 4]);
    mq.regs[7] = marco;
    mq.regs[6] = t;
    mq.llamar(p.init);
    mq = run(mq, 100_000_000);
    let ls = p.local_size;
    let mut salida = Ok(());
    'todo: for lz in 0..ls[2] {
        for ly in 0..ls[1] {
            for lx in 0..ls[0] {
                let index = lz * ls[0] * ls[1] + ly * ls[0] + lx;
                let bloque: Vec<u8> = [[lx, ly, lz], [lx, ly, lz], [0, 0, 0], [1, 1, 1]]
                    .concat()
                    .into_iter()
                    .chain([index])
                    .flat_map(|x| x.to_le_bytes())
                    .collect();
                mq.escribir(ids, &bloque);
                mq.regs[7] = marco;
                mq.regs[6] = t;
                mq.regs[2] = ids;
                mq.regs[1] = FUEL;
                mq.llamar(p.main);
                mq = run(mq, 100_000_000);
                let c = mq.regs[0] as u32;
                if c != 0 {
                    salida = Err(trap_reason(c).unwrap_or(Reason::NotYet { what: "codigo de trampa desconocido" }));
                    break 'todo;
                }
            }
        }
    }
    for (k, addr) in dirs {
        for (i, x) in datos[k].iter_mut().enumerate() {
            *x = (mq.read_u64(addr + 4 * i as u64) & 0xFFFF_FFFF) as u32;
        }
    }
    Ok(salida)
}

fn main() {
    let carpeta = std::env::args().nth(1).expect("uso: census_emit <carpeta con .spv>");
    let mut nombres: Vec<_> = fs::read_dir(&carpeta)
        .expect("no se puede leer la carpeta")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "spv").unwrap_or(false))
        .collect();
    nombres.sort();
    let mut cuenta: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for p in &nombres {
        let name = p.file_stem().unwrap().to_string_lossy().to_string();
        let bytes = fs::read(p).unwrap();
        let mut ids = vec![0u32; 1 << 20];
        let Ok(m) = read(&bytes, &mut ids) else { continue };
        if validate(&m).is_err() {
            continue;
        }
        let pares = buffers_de(&m);
        let mut a: Vec<Vec<u32>> = pares.iter().map(|_| vec![0u32; PALABRAS]).collect();
        let mut b = a.clone();
        let ra = oraculo(&m, &pares, &mut a);
        let veredicto = match emulado(&m, &pares, &mut b) {
            Err(r) => format!("NO EMITE: {}", r.name()),
            Ok(rb) if rb != ra => format!("DISTINTO: oraculo {:?}, emitido {:?}", ra, rb),
            Ok(_) if a != b => "DISTINTO: los buffers no coinciden".to_string(),
            Ok(Ok(())) => "IGUAL".to_string(),
            // Con los buffers a cero el collatz no termina: IGUAL en el motivo.
            Ok(Err(r)) if r == Reason::OutOfFuel || matches!(ra, Err(Reason::OutOfFuel)) => "IGUAL (los dos paran por combustible)".to_string(),
            Ok(Err(r)) => format!("IGUAL (los dos paran: {})", r.name()),
        };
        println!("  {:<44} {}", name, veredicto);
        cuenta.entry(veredicto).or_default().push(name);
    }
    println!();
    for (v, quienes) in &cuenta {
        println!("  {:>4}  {}", quienes.len(), v);
    }
}
