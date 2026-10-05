//! **`ID3D12Device::CheckFeatureSupport`: lo que la tarjeta dice que sabe**
//! (02-10).
//!
//! Cyberpunk pregunta, nada mas crear el dispositivo, D3D12_FEATURE_FEATURE_LEVELS
//! (2) y D3D12_FEATURE_D3D12_OPTIONS (0). La casa solo contestaba la 12 (la
//! firma de raiz) y a lo demas E_INVALIDARG: el juego no monto lo grafico,
//! siguio, y cayo en el fin de fotograma con cero colas
//! (`Cyberpunk2077.exe+0x24d8b3`).
//!
//! Se dice lo de la tarjeta del propietario, una RTX 3060 (Ampere, nivel
//! 12_2), como con los procesadores se dice su Ryzen. Lo que la casa haga
//! luego con cada cosa es otra pared, y esa SI se ve: un hueco que falta
//! sale con su nombre (`com::falta`), no en silencio.
//!
//! Como Windows: la medida tiene que ser EXACTAMENTE la de la estructura; si
//! no, E_INVALIDARG. Lo que no se contesta, se dice (aviso) y E_INVALIDARG.

use crate::aviso;
use crate::com::{E_INVALIDARG, S_OK};

const OPTIONS: u32 = 0;
const ARCHITECTURE: u32 = 1;
const FEATURE_LEVELS: u32 = 2;
const FORMAT_SUPPORT: u32 = 3;
const MULTISAMPLE_QUALITY_LEVELS: u32 = 4;
const FORMAT_INFO: u32 = 5;
const GPU_VIRTUAL_ADDRESS_SUPPORT: u32 = 6;
const SHADER_MODEL: u32 = 7;
const OPTIONS1: u32 = 8;
const ROOT_SIGNATURE: u32 = 12;
const ARCHITECTURE1: u32 = 16;
const OPTIONS2: u32 = 18;
const OPTIONS3: u32 = 21;
const OPTIONS4: u32 = 23;
const OPTIONS5: u32 = 27;
const OPTIONS6: u32 = 30;
const OPTIONS7: u32 = 32;
// E2.3b (05-10): nBodyGravity pregunta OPTIONS12 con ThrowIfFailed.
const OPTIONS8: u32 = 36;
const OPTIONS9: u32 = 37;
const OPTIONS10: u32 = 39;
const OPTIONS11: u32 = 40;
const OPTIONS12: u32 = 41;

/// OPTIONS2 a OPTIONS7 (02-10, lo siguiente que pregunta Cyberpunk): su
/// numero y sus campos de 4 bytes, en orden.
///
/// Lo de la 3060 menos lo que la casa NO hace y un motor usaria si se dice:
/// trazado de rayos (OPTIONS5), sombreado de tasa variable (OPTIONS6),
/// sombreadores de malla y "sampler feedback" (OPTIONS7) van a 0, "no hay",
/// como con el MSAA. Un juego que los tiene por opcionales sigue sin ellos;
/// decir que si seria una pared por cada uno.
///
/// ** Tanda 48 (la leccion de vkd3d-proton: "anunciar solo lo que existe"):
/// pasan a 0 los recursos reservados (tiles), la prueba de limites de
/// profundidad, las posiciones de muestra, el view instancing, las
/// baricentricas, compartir recursos y los 16 bits nativos: la casa no los
/// hace, y un motor que lee un "si" los usa y cae. Sigue el "si" de
/// WriteBufferImmediate y de los sellos de tiempo en la cola de copia, que
/// la casa ya hace (`d3d12_lista2`, `d3d12_resto`).
///
/// ** E2.3b (05-10): de OPTIONS8 a OPTIONS12, todo NO. Las barreras nuevas
/// (`EnhancedBarriersSupported`, OPTIONS12) las pregunta nBodyGravity de
/// Microsoft con ThrowIfFailed: sin contestar, la muestra no arranca; con
/// un NO, va por ResourceBarrier, que la casa ya sabe. Los atomicos de 64
/// bits, el VRS y los de malla, lo mismo que en OPTIONS5 a OPTIONS7.
const OPCIONES_MAS: [(u32, &[u32]); 11] = [
    // DepthBoundsTest NO, ProgrammableSamplePositions NO.
    (OPTIONS2, &[0, 0]),
    // CopyQueueTimestamps, CastingFullyTypedFormat, WriteBufferImmediate en
    // DIRECT|BUNDLE|COMPUTE|COPY, ViewInstancing NO, Barycentrics NO.
    (OPTIONS3, &[1, 1, 0xF, 0, 0]),
    // MSAA64KBAlignedTexture, SharedResourceCompatibility 0, Native16Bit NO.
    (OPTIONS4, &[1, 0, 0]),
    // SRVOnlyTiledResourceTier3 NO, RenderPasses tier 0, Raytracing NO.
    (OPTIONS5, &[0, 0, 0]),
    // AdditionalShadingRates, PerPrimitive..., VRS tier NO, tesela 0,
    // BackgroundProcessing.
    (OPTIONS6, &[0, 0, 0, 0, 0]),
    // MeshShader NO, SamplerFeedback NO.
    (OPTIONS7, &[0, 0]),
    // UnalignedBlockTextures NO.
    (OPTIONS8, &[0]),
    // Las de malla, los atomicos de 64 bits y las derivadas en malla NO,
    // WaveMMA NO.
    (OPTIONS9, &[0, 0, 0, 0, 0, 0]),
    // VRS con suma NO, VRS por primitiva de malla NO.
    (OPTIONS10, &[0, 0]),
    // Atomicos de 64 bits en el monton de descriptores NO.
    (OPTIONS11, &[0]),
    // Las estadisticas de malla: sin sombreadores de malla, UNKNOWN (-1).
    // EnhancedBarriers NO, RelaxedFormatCasting NO.
    (OPTIONS12, &[0xFFFF_FFFF, 0, 0]),
];

/// D3D_FEATURE_LEVEL_12_2: lo mas alto que dice la tarjeta.
const NIVEL_MAXIMO: u32 = 0xc200;
/// D3D_SHADER_MODEL_6_6.
const MODELO_MAXIMO: u32 = 0x66;
/// D3D_ROOT_SIGNATURE_VERSION_1_0: lo que lee la firma de raiz de la casa.
const FIRMA_1_0: u32 = 1;
/// D3D12_FEATURE_DATA_D3D12_OPTIONS, en orden (15 campos de 4 bytes):
/// doble precision, LogicOp, MinPrecision, TiledResources, ResourceBinding
/// 3, PSSpecifiedStencilRef, TypedUAVLoadAdditionalFormats, ROVs,
/// ConservativeRasterization, 40 bits de VA por recurso, StandardSwizzle64KB,
/// CrossNodeSharing 0, CrossAdapterRowMajorTexture, VPAndRTArrayIndex sin
/// GS, ResourceHeap 2. Los de una RTX 3060 MENOS lo que la casa no hace
/// (tanda 48): dobles, LogicOp, 16 bits, tiles, ROVs, rasterizacion
/// conservadora y texturas entre adaptadores, a 0.
const OPCIONES: [u32; 15] = [0, 0, 0, 0, 3, 0, 1, 0, 0, 40, 0, 0, 0, 1, 2];

/// Los DXGI_FORMAT de profundidad: D32_FLOAT_S8X24_UINT, D32_FLOAT,
/// D24_UNORM_S8_UINT, D16_UNORM.
const PROFUNDIDAD: [u32; 4] = [20, 40, 45, 55];

/// `CheckFeatureSupport(this, que, datos, medida)`.
pub(crate) extern "win64" fn check_feature_support(_this: u64, que: u32, datos: *mut u8, medida: u32) -> i32 {
    let m = medida as usize;
    let cabe = |n: usize| !datos.is_null() && m == n;
    // SAFETY (todas las ramas): `datos` es la estructura del `.exe`, de
    // `medida` bytes, comprobada con `cabe` antes de tocarla.
    unsafe {
        let u = |k: usize| (datos.add(4 * k) as *const u32).read_unaligned();
        let pon = |k: usize, v: u32| (datos.add(4 * k) as *mut u32).write_unaligned(v);
        if let Some((_, campos)) = OPCIONES_MAS.iter().find(|(n, _)| *n == que) {
            if !cabe(4 * campos.len()) {
                return E_INVALIDARG;
            }
            for (k, &v) in campos.iter().enumerate() {
                pon(k, v);
            }
            return S_OK;
        }
        match que {
            OPTIONS if cabe(60) => {
                for (k, &v) in OPCIONES.iter().enumerate() {
                    pon(k, v);
                }
            }
            // NodeIndex (entra), TileBasedRenderer, UMA, CacheCoherentUMA:
            // una tarjeta aparte con su memoria. El 1 agrega IsolatedMMU.
            ARCHITECTURE if cabe(16) => arquitectura(datos, 16),
            ARCHITECTURE1 if cabe(20) => arquitectura(datos, 20),
            // NumFeatureLevels (u32), pFeatureLevelsRequested (+8),
            // MaxSupportedFeatureLevel (+16): el mas alto de la lista que la
            // tarjeta da. Ninguno: DXGI_ERROR_UNSUPPORTED, como Windows.
            FEATURE_LEVELS if cabe(24) => {
                let n = u(0) as usize;
                let lista = (datos.add(8) as *const *const u32).read_unaligned();
                if n == 0 || lista.is_null() {
                    return E_INVALIDARG;
                }
                let mejor = (0..n).map(|k| lista.add(k).read_unaligned()).filter(|&l| l <= NIVEL_MAXIMO).max();
                match mejor {
                    Some(l) => pon(4, l),
                    None => return 0x887A_0004_u32 as i32,
                }
            }
            // Format (entra), Support1, Support2. Uno de profundidad: textura
            // 2D y cubo, mip, profundidad. Los demas, lo de una textura
            // comun: bufer y vertices, 1D/2D/3D/cubo, carga y muestreo, mip,
            // destino de dibujo, mezcla, pantalla, gather y UAV tipada; y en
            // el 2, la UAV tipada se lee y se escribe.
            FORMAT_SUPPORT if cabe(12) => {
                let f = u(0);
                if f == 0 {
                    return E_INVALIDARG;
                }
                if PROFUNDIDAD.contains(&f) {
                    pon(1, 0x20 | 0x80 | 0x1000 | 0x1_0000);
                    pon(2, 0);
                } else {
                    pon(1, 0x1 | 0x2 | 0x10 | 0x20 | 0x40 | 0x80 | 0x100 | 0x200 | 0x1000 | 0x4000 | 0x8000 | 0x8_0000 | 0x10_0000 | 0x80_0000 | 0x200_0000);
                    pon(2, 0x40 | 0x80);
                }
            }
            // Format, SampleCount, Flags (entran), NumQualityLevels: una
            // muestra si; varias, no (la casa no tiene MSAA).
            MULTISAMPLE_QUALITY_LEVELS if cabe(16) => pon(3, u32::from(u(1) == 1)),
            // Format (entra), PlaneCount (u8): dos para profundidad con
            // plantilla y NV12, uno lo demas.
            FORMAT_INFO if cabe(8) => {
                let f = u(0);
                if f == 0 {
                    return E_INVALIDARG;
                }
                *datos.add(4) = if matches!(f, 19 | 20 | 21 | 22 | 44 | 45 | 46 | 47 | 103) { 2 } else { 1 };
            }
            GPU_VIRTUAL_ADDRESS_SUPPORT if cabe(8) => {
                pon(0, 40);
                pon(1, 40);
            }
            // HighestShaderModel: entra el que se quiere y sale el que hay
            // (como mucho, ese).
            SHADER_MODEL if cabe(4) => pon(0, u(0).min(MODELO_MAXIMO)),
            // WaveOps, WaveLaneCountMin/Max (32: un warp), TotalLaneCount,
            // ExpandedComputeResourceStates, Int64ShaderOps.
            OPTIONS1 if cabe(24) => {
                for (k, v) in [1, 32, 32, 28 * 128, 1, 1].into_iter().enumerate() {
                    pon(k, v);
                }
            }
            // HighestVersion: entra la que se quiere; la casa lee 1.0 (y
            // d3dx12 baja la 1.1 a 1.0).
            ROOT_SIGNATURE if cabe(4) => pon(0, FIRMA_1_0),
            // Una que la casa sabe, con la medida mal: el error es del `.exe`
            // (Windows dice lo mismo, y no hay nada que avisar).
            OPTIONS | ARCHITECTURE | FEATURE_LEVELS | FORMAT_SUPPORT | MULTISAMPLE_QUALITY_LEVELS | FORMAT_INFO | GPU_VIRTUAL_ADDRESS_SUPPORT
            | SHADER_MODEL | OPTIONS1 | ROOT_SIGNATURE | ARCHITECTURE1 => return E_INVALIDARG,
            _ => {
                aviso(&alloc::format!("ID3D12Device::CheckFeatureSupport({que}, {medida} B): todavia no se contesta"));
                return E_INVALIDARG;
            }
        }
    }
    S_OK
}

/// D3D12_FEATURE_DATA_ARCHITECTURE(1): el nodo entra; una tarjeta discreta.
///
/// # Safety
/// `datos` tiene `m` (16 o 20) bytes del `.exe`.
unsafe fn arquitectura(datos: *mut u8, m: usize) {
    for k in 1..4 {
        (datos.add(4 * k) as *mut u32).write_unaligned(0);
    }
    if m == 20 {
        (datos.add(16) as *mut u32).write_unaligned(1);
    }
}
