//! Lector anfitrion del MISMO parser puro usado en BMO-X.
//! Uso: cargo run -p bmo-gpu-ga10x --example libos -- firmware.bin [gsplog.bin] [task.elf]
//! task.elf es el ELF de logging de la misma build; nunca se sustituye por otro.
use bmo_gpu_ga10x::{elf::Fuente, inspeccion::libos::{Lector, resumen, diccionario::Elf}};
use std::{fs::File, io::{Read, Seek, SeekFrom}, error::Error};
struct Archivo(File);
impl Fuente for Archivo {
    fn leer(&mut self, desde:u64, dst:&mut[u8])->bool {
        self.0.seek(SeekFrom::Start(desde)).is_ok() && self.0.read_exact(dst).is_ok()
    }
}
fn abrir(p:&str)->Result<Elf<Archivo>,Box<dyn Error>> {
    let f=File::open(p)?; let n=f.metadata()?.len();
    Elf::nuevo(Archivo(f),n).map_err(|e|format!("ELF: {e:?}").into())
}
fn main()->Result<(),Box<dyn Error>> {
    let args:Vec<_>=std::env::args().collect();
    let fw=args.get(1).ok_or("falta firmware.bin [gsplog.bin] [task.elf]")?;
    let mut e=abrir(fw)?; let mut logs=0;
    for i in 0..e.secciones() {
        let s=e.seccion(i).map_err(|e|format!("seccion {i}: {e:?}"))?;
        let nombre=String::from_utf8_lossy(&s.nombre[..s.n]);
        println!("{nombre:26} VA={:016x} offset={:08x} bytes={}",s.va,s.offset,s.bytes);
        if nombre.starts_with(".fwlogging") { logs+=1; }
    }
    println!("secciones .fwlogging*: {logs}; cero = diccionario ausente, no logs vacios");
    let Some(ruta)=args.get(2) else {return Ok(())};
    // Contrato BMO-X: tres anillos de 64 KiB. Un limite ANTES de reservar.
    let f=File::open(ruta)?;
    if f.metadata()?.len()!=3*65536 {return Err("gsplog.bin debe medir exactamente 196608 bytes".into());}
    let mut b=vec![0;3*65536]; f.take(b.len() as u64).read_exact(&mut b)?;
    for (nombre,copia) in ["LOGINIT","LOGINTR","LOGRM"].into_iter().zip(b.chunks_exact(65536)) {
        println!("{nombre}: {:?} (palabras; instantanea sin garantia de coherencia)",resumen(copia));
        let mut l=Lector::nuevo(copia).map_err(|e|format!("{e:?}"))?;
        println!("  ultimo (timestamp, metadata VA): {:x?}",l.cabecera());
        let Some(dict)=args.get(3) else {continue};
        let mut d=abrir(dict)?;
        for _ in 0..1024 {
            match l.siguiente(&mut d) {
                Ok(Some(r))=> {
                    println!("  t={} meta={:016x} args={:x?}",r.timestamp,r.metadata_va,&r.argumentos[..r.cuantos as usize]);
                    let m=d.metadata(r.metadata_va).map_err(|e|format!("{e:?}"))?;
                    let mut formato=[0;512];
                    match d.texto(m.formato,&mut formato) {
                        Ok(n)=>println!("    formato literal: {}",String::from_utf8_lossy(&formato[..n]).escape_default()),
                        Err(e)=>println!("    formato sin resolver: {e:?}"),
                    }
                    if m.remoto!=0 { println!("    tarea remota: VA={:016x}; argumentos sin resolver con otra tarea",m.remoto); }
                }
                Ok(None)=>break,
                Err(e)=>{println!("  detenido sin adivinar: {e:?}");break;}
            }
        }
    }
    Ok(())
}
