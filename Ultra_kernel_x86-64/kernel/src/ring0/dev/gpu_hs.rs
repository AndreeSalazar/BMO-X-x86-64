//! Lente externo HS: entradas, fases, fallos y registros accesibles en CABINA.
//! [carril] VERDE -- observador; no agrega ordenes de aparato ni permisos.
//! [consumo] NADA -- solo corre dentro de una operacion HS ya autorizada.
//! [estado]  RAM  ordinales por arranque; no sobreviven a reiniciar BMO.
use core::sync::atomic::{AtomicU64, Ordering};
use bmo_gpu_ga10x::{falcon as f, arranque::hs_observacion::{self as h, Motor}, Registros};
use crate::ring0::cabina;

static SIGUIENTE: AtomicU64 = AtomicU64::new(0);
static GSP: AtomicU64 = AtomicU64::new(0);
static SEC2: AtomicU64 = AtomicU64::new(0);
static PARADA_GSP: AtomicU64 = AtomicU64::new(0);
static PARADA_SEC2: AtomicU64 = AtomicU64::new(0);
pub struct Tramo { id: u64, motor: Motor }
impl Tramo {
    fn registrar(motor: Motor, funcion: &'static str) -> Self {
        let id = SIGUIENTE.fetch_add(1, Ordering::AcqRel) + 1;
        // Limite de volumen; nunca rechaza la operacion observada.
        let s = Self { id: if id <= 32 { id } else { 0 }, motor };
        s.actual().store(s.id, Ordering::Release);
        if id == 33 { cabina::warn("gpu", "HS lente: limite de 32 operaciones; posteriores sin detalle", id); }
        if s.id != 0 {
            s.paso(funcion);
            cabina::info("gpu", "HS entrada: motor base", motor.base() as u64);
        }
        s
    }
    pub fn nuevo(motor: Motor, funcion: &'static str, firma: u32, fusible: u32, crudo: u32) -> Self {
        let s = Self::registrar(motor, funcion);
        s.dato("HS entrada: firma elegida (indice, no autenticacion)", firma as u64);
        s.dato("HS entrada: registro de fusible | crudo << 32", fusible as u64 | (crudo as u64) << 32);
        s
    }
    /// Parametros ya copiados y validados por hs615::ordenes; no ejecuta nada.
    pub fn runtime(datos: &[u8]) -> Self {
        let s = Self::registrar(Motor::Gsp, "HS entrada: EXEC_HS_BINARY 615 validado; id");
        s.dato("HS 615 entrada: bytes de parametros", datos.len() as u64);
        for (i, w) in datos.chunks_exact(8).take(10).enumerate() {
            s.dato("HS 615 entrada: offset de palabra", i as u64 * 8);
            s.dato("HS 615 entrada: palabra cruda de parametros validados", u64::from_le_bytes(w.try_into().unwrap_or([0; 8])));
        }
        s
    }
    fn actual(&self) -> &'static AtomicU64 { match self.motor { Motor::Gsp => &GSP, Motor::Sec2 => &SEC2 } }
    pub fn paso(&self, texto: &'static str) {
        if self.id != 0 { cabina::info("gpu", texto, self.id); }
    }
    pub fn dato(&self, texto: &'static str, valor: u64) {
        if self.id != 0 { cabina::info("gpu", texto, valor); }
    }
    pub fn huella(&self, bytes: &[u8]) {
        if self.id == 0 { return; }
        self.dato("HS entrada: bytes de copia CPU identificada", bytes.len() as u64);
        let h = bmo_hash::hash(bytes);
        for (i, w) in h.chunks_exact(8).enumerate() {
            self.dato("HS entrada: BLAKE3 indice de palabra LE", i as u64);
            self.dato("HS entrada: BLAKE3 palabra LE (no prueba consumo DMA)", u64::from_le_bytes(w.try_into().unwrap_or([0; 8])));
        }
    }
    pub fn ejecutar<T, R: Registros>(&self, r: &mut R, fase: &'static str, hacer: impl FnOnce(&mut R) -> Result<T, f::NoFuego>) -> Result<T, f::NoFuego> {
        self.paso(fase);
        let inicio = crate::ring0::task::scheduler::rdtsc();
        let resultado = hacer(r);
        let fin = crate::ring0::task::scheduler::rdtsc();
        self.dato("HS fase: TSC antes de la operacion", inicio);
        self.dato("HS fase: TSC despues de la operacion", fin);
        if let Err(e) = resultado {
            let (texto, dato) = h::fallo(e);
            self.dato(texto, dato as u64);
            self.foto(r, "HS foto externa al fallar; id");
        }
        resultado
    }
    #[allow(clippy::too_many_arguments)]
    pub fn dma(&self, r: &mut impl Registros, t: &mut impl f::Reloj, iova: u64, origen: u32, destino: u32, bytes: u32, imem: bool, seguro: bool) -> Result<(), f::NoFuego> {
        self.paso(if imem { "HS fase: cargar IMEM; id" } else { "HS fase: cargar DMEM; id" });
        self.dato("HS DMA entrada: IOVA base", iova);
        self.dato("HS DMA entrada: origen | destino << 32", origen as u64 | (destino as u64) << 32);
        self.dato("HS DMA entrada: bytes | IMEM << 32 | SEC << 33", bytes as u64 | (imem as u64) << 32 | (seguro as u64) << 33);
        let x = self.ejecutar(r, "HS fase: copiar DMA; id", |r| f::copiar_etiquetado(r, t, self.motor.base(), iova, origen, destino, bytes, imem, seguro, 50_000));
        if x.is_ok() { self.paso("HS fase: DMA completo e IDLE observado; id"); }
        x
    }
    pub fn foto(&self, r: &mut impl Registros, fase: &'static str) {
        if self.id == 0 { return; }
        self.paso(fase);
        let desde = crate::ring0::task::scheduler::rdtsc();
        let foto = h::Foto::leer(r, self.motor);
        let hasta = crate::ring0::task::scheduler::rdtsc();
        self.dato("HS foto: TSC antes", desde);
        self.dato("HS foto: TSC despues", hasta);
        self.dato("HS foto: mascara PRI; orden hs_observacion::REGISTROS", foto.pri as u64);
        for (i, offset) in h::REGISTROS.into_iter().enumerate() {
            let texto = if foto.pri & (1 << i) != 0 { "HS foto PRI: id[31:24] dir[23:0] raw[63:32]" }
                else { "HS foto legible: id[31:24] dir[23:0] raw[63:32]" };
            self.dato(texto, (self.motor.base()+offset) as u64 | self.id << 24 | (foto.crudos[i] as u64) << 32);
        }
        self.paso("HS limite: foto externa; autenticacion interna y PC/TCM HS no observados; id");
    }
}
pub fn foto_actual(r: &mut impl Registros, motor: Motor, fase: &'static str) {
    let id = match motor { Motor::Gsp => &GSP, Motor::Sec2 => &SEC2 }.load(Ordering::Acquire);
    Tramo { id, motor }.foto(r, fase);
}
/// Se llama solo desde la observacion de parada que ya existe, una vez por id.
pub fn parado(r: &mut impl Registros, motor: Motor) {
    let (actual, ultima) = match motor { Motor::Gsp => (&GSP, &PARADA_GSP), Motor::Sec2 => (&SEC2, &PARADA_SEC2) };
    let id = actual.load(Ordering::Acquire);
    if id != 0 && ultima.swap(id, Ordering::AcqRel) != id {
        Tramo { id, motor }.foto(r, "HS foto externa de parada observada; id");
    }
}
