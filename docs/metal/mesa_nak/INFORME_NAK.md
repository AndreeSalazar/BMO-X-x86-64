<!--
  INFORME_NAK -- lo que NAK (el compilador de NVK, Mesa) dice de SHFL,
  FSWZADD, VOTE, SULD/SUST, ATOM y el TIC en Ampere (SM86).

  De donde sale (13-10): el propietario bajo `src/nouveau/` de Mesa
  (gitlab.freedesktop.org/mesa/mesa, 26.3.0-devel, commit 9d007d0187) y una
  IA en su PC lo leyo con el prompt de la sesion; aqui se COTEJO contra ese
  codigo: las derivadas (`from_nir.rs:2463-2500`), FSWZADD
  (`sm70_encode.rs:1132-1166`), el modo de LOD del TEX
  (`sm70_encode.rs:2763-2795`: Auto 0, Zero 1, Lod 3 -- los mismos de
  nuestras palabras de oro) y "NVIDIA can't do linear and mipmapping"
  (`nil/image.rs:236-237`, lo que ya habia decidido G5). Los fragmentos de
  codigo son de Mesa, licencia MIT (Copyright the Mesa authors). Es
  REFERENCIA: cada dato entra al emisor con su palabra de oro de `ptxas` y,
  lo que siga en duda (el signo de FSWZADD, arriba/abajo del cuadro), con lo
  que mida la lente.
-->

Mesa 26.3.0-devel (fichero VERSION) - commit 9d007d018715131a85e11bee9a91b19b744a4374 (Tue Oct 13 22:41:50 2026 +0000, "intel/dev: refactor update topology logic")

# INFORME NAK - SASS SM80/SM86 (Ampere, GA106 / RTX 3060)

Todas las rutas son relativas a `src/`. Las citas `fichero:linea` apuntan al
codigo leido en el commit de arriba. Cuando algo sale de fuera de
`src/nouveau/` (por ejemplo del compilador viejo de gallium) se marca como
**[FUERA DE src/nouveau]**.

---

## 0. Convenciones generales (validas para todos los apartados)

### 0.1 SM86 en NAK

- GA106 (chipset 0x172..0x17a) -> SM 86: `nouveau/winsys/nouveau_device.c:67-68`
  (`else if (chipset >= 0x172) return 86;`).
- NAK usa el MISMO codificador para todo Volta+ (`SM70Encoder`,
  `nouveau/compiler/nak/sm70_encode.rs`). Las ramas internas usan `e.sm >= 80`,
  `>= 90`, `>= 100`, `>= 120`. Para SM86 aplican siempre las ramas `sm >= 80`
  y `sm < 90`.
- Latencias/scoreboard: SM86 entra en `is_ampere()` = `sm >= 80 && sm < 89`
  (`nouveau/compiler/nak/ir.rs:9674-9676`), y se usa la tabla SM80
  (`nouveau/compiler/nak/sm70.rs:174-175`, `sm80_instr_latencies.rs`).
- La unica diferencia SM86 frente a SM80 que he visto en el codigo leido:
  `Op::F2FP(_) => self.sm() >= 86` (`nouveau/compiler/nak/sm70.rs:148`), que no
  afecta a nada de esta lista.

### 0.2 Formato de instruccion

Instruccion de 128 bits = `[u32; 4]` (`sm70_encode.rs:22-27`), 4 palabras por
instruccion (`sm70_encode.rs:4826`, `4842`). En el codigo los campos se
escriben con rangos Rust `a..b` = bits `a` a `b-1` (ancho `b-a`). En las
tablas uso esa misma notacion.

Campos comunes a TODAS las instrucciones:

| bits | nombre | valores | cita |
|---|---|---|---|
| 0..12 | opcode | 12 bits | `sm70_encode.rs:70-72` |
| 12..15 | predicado de guarda | P0..P6, 7 = PT | `sm70_encode.rs:279-290` |
| 15 | negar predicado de guarda | 0/1 | `sm70_encode.rs:289` |
| 16..24 | Rd (destino GPR) | 0..254, 255 = RZ | `sm70_encode.rs:292-298`, `57-64` |
| 16..22 | URd (destino UGPR, sm<100) | 0..63, 63 = URZ | `sm70_encode.rs:80-94`, `300-306` |
| 105..109 | delay (stall) | 0..15 | `sm70_encode.rs:325` |
| 109 | yield (sm<120) | 0/1 | `sm70_encode.rs:330` |
| 110..113 | scoreboard de escritura (wr_bar) | 0..5, 7 = ninguno | `sm70_encode.rs:326` |
| 113..116 | scoreboard de lectura (rd_bar) | 0..5, 7 = ninguno | `sm70_encode.rs:327` |
| 116..122 | mascara de espera (wt_bar_mask) | 6 bits, 1 por SB | `sm70_encode.rs:328` |
| 122..126 | reuse mask (sm<120) | 4 bits | `sm70_encode.rs:331` |

Predicados destino sin usar se escriben como 7 (PT) (`sm70_encode.rs:162-173`).
Registro cero: GPR 255, UGPR 63 en sm<100 (`sm70_encode.rs:49-64`).

```rust
// nouveau/compiler/nak/sm70_encode.rs:324-333
fn set_instr_deps(&mut self, deps: &InstrDeps) {
    self.set_field(105..109, deps.delay);
    self.set_field(110..113, deps.wr_bar().unwrap_or(7));
    self.set_field(113..116, deps.rd_bar().unwrap_or(7));
    self.set_field(116..122, deps.wt_bar_mask);
    if self.sm < 120 {
        self.set_bit(109, deps.yld);
        self.set_field(122..126, deps.reuse_mask);
    }
}
```

### 0.3 Regla general de scoreboard en SM80/SM86

- Hay 6 scoreboards (`nouveau/compiler/nak/calc_instr_deps.rs:262-270`).
- Una instruccion necesita scoreboard en Ampere si su categoria de latencia
  SM80 es `RedirectedFP64 | Clmad | IMMA_88 | MMA_* | DMMA | Cbu | Decoupled |
  DecoupledAgu` (`sm80_instr_latencies.rs:1457-1477`). Las uniformes solo si
  son `ToUr`.
- Si necesita scoreboard: se le asigna un SB de lectura (rd_bar) si alguien
  escribe despues sus fuentes, y uno de escritura (wr_bar) si alguien lee
  despues su destino; los consumidores ponen ese bit en `wt_bar_mask`
  (`calc_instr_deps.rs:663-742`).
- Si no lo necesita, la dependencia se cubre con el campo `delay`
  (`calc_instr_deps.rs:686-697`), con las tablas RAW/WAR/WAW de
  `sm80_instr_latencies.rs:257` y siguientes.
- `TEXDEPBAR` solo existe en Kepler: `assert!(sm.is_kepler())`
  (`calc_instr_deps.rs:600`). En Ampere NO hay barrera de textura especial,
  solo scoreboards.

---

## 1. SHFL

### a) Codificacion (`sm70_encode.rs:2558-2614`)

Cuatro opcodes segun que operandos son inmediatos:

| opcode | lane | c | cita |
|---|---|---|---|
| 0x389 | registro (32..40) | registro (64..72) | `sm70_encode.rs:2573-2577` |
| 0x589 | registro (32..40) | inmediato (40..53) | `sm70_encode.rs:2578-2582` |
| 0x989 | inmediato (53..58) | registro (64..72) | `sm70_encode.rs:2586-2590` |
| 0xf89 | inmediato (53..58) | inmediato (40..53) | `sm70_encode.rs:2591-2595` |

| bits | nombre | valores | cita |
|---|---|---|---|
| 0..12 | opcode | 0x389 / 0x589 / 0x989 / 0xf89 | arriba |
| 16..24 | Rd | GPR | `sm70_encode.rs:2601` |
| 24..32 | Ra (dato a barajar) | GPR (siempre registro) | `sm70_encode.rs:2603`, legalize `2561` |
| 32..40 | lane (registro) | GPR / RZ | `sm70_encode.rs:2575`, `2580` |
| 40..53 | c (inmediato, 13 bits) | se enmascara con `& 0x1f1f` | `sm70_encode.rs:2581`, `2593`; `ir.rs:5268-5270` |
| 53..58 | lane (inmediato, 5 bits) | se enmascara con `& 0x1f` | `sm70_encode.rs:2588`, `2594`; `ir.rs:5263-5265` |
| 58..60 | modo | IDX=0, UP=1, DOWN=2, BFLY=3 | `sm70_encode.rs:2604-2612` |
| 64..72 | c (registro) | GPR / RZ | `sm70_encode.rs:2576`, `2589` |
| 81..84 | predicado destino "in_bounds" | P0..P6, 7=PT (NAK siempre lo deja en PT) | `sm70_encode.rs:2602` |

```rust
// nouveau/compiler/nak/ir.rs:5261-5271
pub fn reduce_lane_c_imm(&mut self) {
    debug_assert!(self.lane.src_mod.is_none());
    if let SrcRef::Imm32(lane) = &mut self.lane.src_ref {
        *lane &= 0x1f;
    }
    debug_assert!(self.c.src_mod.is_none());
    if let SrcRef::Imm32(c) = &mut self.c.src_ref {
        *c &= 0x1f1f;
    }
}
```

### b) Modificadores

- `.IDX .UP .DOWN .BFLY` en bits 58..60 (arriba). Display `shfl.{idx,up,down,bfly}`
  (`ir.rs:4444-4460`).
- No hay tipo ni `.E`: siempre 32 bits (`from_nir.rs:3711-3718`). Los
  subgrupos de menos de 32 bits se ensanchan a 32 antes
  (`nouveau/compiler/nak_nir.c:347-360`).

### c) Cuando lo usa NAK (`from_nir.rs:3705-3763`, `2468-2475`, `2529-2536`)

| NIR | modo | lane | c |
|---|---|---|---|
| `shuffle`, `read_invocation` | IDX | indice | 0x1f |
| `shuffle_xor` | BFLY | mascara | 0x1f |
| `shuffle_down` | DOWN | delta | 0x1f |
| `shuffle_up` | UP | delta | 0 |
| `quad_broadcast` | IDX | indice | 0x1c03 |
| `quad_swap_horizontal` | BFLY | 1 | 0x1c03 |
| `quad_swap_vertical` | BFLY | 2 | 0x1c03 |
| `quad_swap_diagonal` | BFLY | 3 | 0x1c03 |
| `ddx*` (paso previo a FSWZADD) | BFLY | 1 | 0x3 \| (0x1c << 8) = 0x1c03 |
| `ddy*` (paso previo a FSWZADD) | BFLY | 2 | 0x1c03 |

Ademas `rotate` se baja a shuffle (`nak_nir.c:1661`, `.lower_rotate_to_shuffle = true`).

```rust
// nouveau/compiler/nak/from_nir.rs:3721-3737
b.push_op(OpShfl {
    dst: dst.into(),
    in_bounds: Dst::None,
    src: data,
    lane: idx,
    c: match intrin.intrinsic {
        nir_intrinsic_quad_broadcast => 0x1c_03.into(),
        nir_intrinsic_shuffle_up => 0.into(),
        _ => 0x1f.into(),
    },
    op: match intrin.intrinsic {
        nir_intrinsic_shuffle_down => ShflOp::Down,
        nir_intrinsic_shuffle_up => ShflOp::Up,
        nir_intrinsic_shuffle_xor => ShflOp::Bfly,
        _ => ShflOp::Idx,
    },
});
```

### d) Dependencias

- No es de latencia fija: `Op::Shfl(_) => false` (`ir.rs:8315`).
- Categoria SM80 GPR: `DecoupledAgu` (`sm80_instr_latencies.rs:248`) ->
  NECESITA scoreboard (`sm80_instr_latencies.rs:1464-1475`). Categoria del
  predicado: `Decoupled` (`sm80_instr_latencies.rs:888`). En la tabla de
  uniformes cuenta como `vdecoupled` (`sm80_instr_latencies.rs:1124`).
- El consumidor del resultado espera el wr_bar; quien sobrescriba Ra/lane/c
  despues espera el rd_bar (regla general 0.3).

---

## 2. Derivadas (ddx/ddy, fine/coarse, LOD implicito)

### a) Codificacion de FSWZADD (`sm70_encode.rs:1132-1166`)

| bits | nombre | valores | cita |
|---|---|---|---|
| 0..12 | opcode | 0x822 | `sm70_encode.rs:1141` |
| 16..24 | Rd | GPR | `sm70_encode.rs:1142` |
| 24..32 | Ra = src0 | GPR (obligatorio registro) | `sm70_encode.rs:1144`, legalize `1136` |
| 32..40 | subop: 4 ops de 2 bits | op[0] en bits 38..40, op[1] 36..38, op[2] 34..36, op[3] 32..34 | `sm70_encode.rs:1147-1160` |
| 64..72 | Rb = src1 | GPR (obligatorio registro) | `sm70_encode.rs:1145`, legalize `1137` |
| 77 | .NDV | 0 = Auto, 1 = NonDivergent | `sm70_encode.rs:1162`, `2800-2807` |
| 78..80 | redondeo | RN=0, RM=1, RP=2, RZ=3 | `sm70_encode.rs:1163`, `765-776` |
| 80 | .FTZ | 0/1 | `sm70_encode.rs:1164` |

Valores de cada op de 2 bits (en SM70+):

| op NAK | nombre display | codigo | cita |
|---|---|---|---|
| `Add` | add | 0 | `sm70_encode.rs:1151`, `ir.rs:2973` |
| `SubLeft` | sub | 1 | `sm70_encode.rs:1153`, `ir.rs:2975` |
| `SubRight` | subr | 2 | `sm70_encode.rs:1152`, `ir.rs:2974` |
| `MoveLeft` | mov2 | 3 | `sm70_encode.rs:1154`, `ir.rs:2976` |

```rust
// nouveau/compiler/nak/sm70_encode.rs:1147-1160
let mut subop = 0x0_u8;
for (i, swz_op) in self.ops.iter().enumerate() {
    let swz_op = match swz_op {
        FSwzAddOp::Add => 0,
        FSwzAddOp::SubRight => 2,
        FSwzAddOp::SubLeft => 1,
        FSwzAddOp::MoveLeft => 3,
    };
    subop |= swz_op << ((self.ops.len() - i - 1) * 2);
}
e.set_field(32..40, subop);
```

Nota: NAK usa los mismos codigos en SM50 (`SubLeft => 1u8, SubRight => 2u8`,
`nouveau/compiler/nak/sm50.rs:933-936`), mientras que el compilador viejo
cambia 1<->2 entre SM50 y GV100 (ver DUDAS 3).

### b) Secuencia que emite NAK para SM >= 50 (incluye SM86)

ddx (`from_nir.rs:2455-2516`):

```rust
// nouveau/compiler/nak/from_nir.rs:2468-2497
b.push_op(OpShfl {
    dst: scratch.into(),
    in_bounds: Dst::None,
    src: self.get_src(&srcs[0]),
    lane: 1_u32.into(),
    c: (0x3_u32 | (0x1c_u32 << 8)).into(),
    op: ShflOp::Bfly,
});
// Starting with Blackwell, the shader stage now affects
// fswzadd so we need to use fswzadd.ndv
let deriv_mode = if self.sm.sm() >= 100 {
    TexDerivMode::NonDivergent
} else {
    TexDerivMode::Auto
};
b.push_op(OpFSwzAdd {
    dst: dst.into(),
    srcs: [scratch.into(), self.get_src(&srcs[0])],
    ops: [
        FSwzAddOp::SubLeft,
        FSwzAddOp::SubRight,
        FSwzAddOp::SubLeft,
        FSwzAddOp::SubRight,
    ],
    ...
```

ddy (`from_nir.rs:2517-2577`): igual pero `lane: 2` y
`ops: [SubLeft, SubLeft, SubRight, SubRight]`.

Resumen en bits para SM86:

| derivada | SHFL | FSWZADD src0 | FSWZADD src1 | subop (bits 32..40) | .NDV (bit 77) |
|---|---|---|---|---|---|
| ddx / ddx_fine / ddx_coarse | BFLY lane=1 c=0x1c03 | resultado del SHFL (vecino) | valor propio | sub,subr,sub,subr = 0x66 | 0 |
| ddy / ddy_fine / ddy_coarse | BFLY lane=2 c=0x1c03 | resultado del SHFL (vecino) | valor propio | sub,sub,subr,subr = 0x5A | 0 |

(0x66 = 1<<6 | 2<<4 | 1<<2 | 2; 0x5A = 1<<6 | 1<<4 | 2<<2 | 2, calculado con la
formula de `sm70_encode.rs:1157`.)

Redondeo y FTZ salen de `self.float_ctl[F32]` (`from_nir.rs:2494-2495`).
Solo 32 bits: `assert!(intrin.def.bit_size() == 32)` (`from_nir.rs:2460`,
`2522`).

### Orden de los 4 hilos del cuadro 2x2

Lo que dice el codigo de src/nouveau:

- `quad_swap_horizontal` = BFLY 1, `quad_swap_vertical` = BFLY 2,
  `quad_swap_diagonal` = BFLY 3 (`from_nir.rs:3753-3758`). Es decir el bit 0
  del indice de hilo dentro del cuadro es la X y el bit 1 es la Y.
- Comentario del IR: `SwapHorizontal` = "swap [0, 1] and [2, 3]",
  `SwapVertical` = "swap [0, 2] and [1, 3]" (`ir.rs:3029-3032`).
- En compute con `DERIVATIVE_GROUP_QUADS`, NAK reconstruye el ID local asi:
  bit 0 del lane -> x bajo, bit 1 del lane -> y bajo
  (`nouveau/compiler/nak_nir.c:605-613`):

```c
/* nouveau/compiler/nak_nir.c:605-613 */
if (b->shader->info.derivative_group == DERIVATIVE_GROUP_QUADS) {
   nir_def *x_lo = nir_iand_imm(b, x, 0x1);
   nir_def *y_lo = nir_ushr_imm(b, nir_iand_imm(b, x, 0x2), 1);
   nir_def *x_hi = nir_ushr_imm(b, nir_iand_imm(b, x, ~0x3), 1);
   nir_def *y_hi = nir_ishl_imm(b, y, 1);
   x = nir_ior(b, x_lo, x_hi);
   y = nir_ior(b, y_lo, y_hi);
}
```

Por tanto, segun el codigo: hilo 0 = (x0,y0), hilo 1 = (x0+1,y0), hilo 2 =
(x0,y0+1), hilo 3 = (x0+1,y0+1).

Que y+1 sea "abajo" en pantalla en el sombreador de pixeles: NO ENCONTRADO en
src/nouveau. Solo lo dice el compilador viejo **[FUERA DE src/nouveau]**:
`gallium/drivers/nouveau/codegen/nv50_ir_lowering_gm107.cpp:39`
(`//             UL UR LL LR`), es decir hilo 0 = arriba-izquierda,
1 = arriba-derecha, 2 = abajo-izquierda, 3 = abajo-derecha.

### Signo de ddx y ddy

El codigo de NAK NO define la semantica aritmetica de `sub`/`subr`
(NO ENCONTRADO: no hay emulador ni comentario que diga cual menos cual).
Lo que si se puede afirmar del codigo:

- src0 = valor del vecino (SHFL BFLY), src1 = valor propio
  (`from_nir.rs:2487`, `2548`).
- ddx usa `sub` en los hilos 0 y 2 (columna izquierda) y `subr` en 1 y 3
  (columna derecha); ddy usa `sub` en 0 y 1 (fila de arriba) y `subr` en 2 y 3.
- Si `sub` = src0 - src1 y `subr` = src1 - src0 (interpretacion habitual,
  NO verificada en el codigo), entonces en los 4 hilos:
  ddx = v(x+1) - v(x) (derecha menos izquierda) y
  ddy = v(y+1) - v(y) (fila 2/3 menos fila 0/1). Con la interpretacion
  contraria saldria el signo opuesto, pero igual en los cuatro hilos.
- El compilador viejo usa exactamente la misma secuencia
  **[FUERA DE src/nouveau]**: `QUADOP(SUB, SUBR, SUB, SUBR)` con xid=1 para
  DFDX y `QUADOP(SUB, SUB, SUBR, SUBR)` con xid=2 para DFDY
  (`gallium/drivers/nouveau/codegen/nv50_ir_lowering_gm107.cpp:229-250`), y en
  su emisor GV100 intercambia 1<->2 (`nv50_ir_emit_gv100.cpp:571-577`,
  "NP/PN swapped vs SM60"), con lo que SUB acaba codificado como 1 y SUBR
  como 2, igual que NAK.

### Fine contra coarse

No hay diferencia: los tres intrinsics (`ddx`, `ddx_coarse`, `ddx_fine`) van
por el mismo camino y el codigo dice `// TODO: Real coarse derivatives`
(`from_nir.rs:2455-2458`, `2517-2520`). Coarse = fine.

### LOD implicito del muestreo

No usa SHFL/FSWZADD: el TEX lo calcula el hardware con lod_mode = Auto (ver
apartado 6). En SM86 `deriv_mode = Auto` (bit 77 = 0) (`from_nir.rs:2013-2018`).
Comentario sobre el modo Auto: "For partial (not full) quads, the derivative
will default to the value of DEFAULT_PARTIAL in SET_SHADER_CONTROL"
(`ir.rs:2061-2068`). Donde NVK programa DEFAULT_PARTIAL: NO ENCONTRADO (grep de
`DEFAULT_PARTIAL` / `SHADER_CONTROL` en `nouveau/vulkan/` sin resultados).

### Hilos ayudantes (helper invocations)

- `is_helper_invocation` / `load_helper_invocation` -> lectura del registro
  de sistema `NAK_SV_THREAD_KILL = 0x13` (S2R) (`nak_nir.c:663-666`,
  `nak_private.h:118`). S2R: opcode 0x919, indice SR en bits 72..80
  (`sm70_encode.rs:4409-4418`).
- `demote` / `demote_if` -> `KILL` (opcode 0x95b, pred fuente 87..90 = PT,
  negacion bit 90; el `demote_if` lo pone en el predicado de guarda)
  (`from_nir.rs:3040-3056`, `sm70_encode.rs:4365-4368`). Marca
  `uses_kill`, que activa KILLS_PIXELS en el SPH (`sph.rs:565`).
- `terminate` se convierte en demote en el sombreador de pixeles
  (`nak_nir.c:415-419`); `discard_is_demote: true` (`nak/api.rs:189`).
- En fragment se ejecuta `nir_opt_load_skip_helpers` (`nak_nir.c:1710-1715`),
  que marca cargas que los ayudantes pueden saltarse.
- Nada mas: NAK no inserta nada explicito para mantener vivos los ayudantes
  alrededor de SHFL/FSWZADD (NO ENCONTRADO).

### d) Dependencias

- FSWZADD: latencia fija (`ir.rs:8267`), categoria SM80 `CoupledFMA`
  (`sm80_instr_latencies.rs:124`) -> NO usa scoreboard, se cubre con delay.
- El SHFL previo SI usa scoreboard (apartado 1d): el FSWZADD espera el wr_bar
  del SHFL en `wt_bar_mask`.
- KILL: categoria `Decoupled` (`sm80_instr_latencies.rs:221`).

---

## 3. VOTE, MATCH, REDUX y Quad*

### a) Codificacion VOTE / VOTEU (`sm70_encode.rs:4465-4491`)

| bits | nombre | valores | cita |
|---|---|---|---|
| 0..12 | opcode | 0x806 VOTE, 0x886 VOTEU (si es uniforme) | `sm70_encode.rs:4471-4477` |
| 16..24 | Rd = ballot (GPR) | GPR, RZ si no se usa | `sm70_encode.rs:4476` |
| 16..22 | URd = ballot (VOTEU) | UGPR | `sm70_encode.rs:4473` |
| 72..74 | modo | ALL=0, ANY=1, EQ=2 | `sm70_encode.rs:4479-4486` |
| 81..84 | predicado destino (vote) | P0..P6, 7=PT | `sm70_encode.rs:4488` |
| 87..90 | predicado fuente | P0..P6, 7=PT | `sm70_encode.rs:4489` |
| 90 | negar predicado fuente | 0/1 (PF = PT negado) | `sm70_encode.rs:4489`, `183-201` |

### a) Codificacion MATCH (`sm70_encode.rs:4493-4518`)

| bits | nombre | valores | cita |
|---|---|---|---|
| 0..12 | opcode | 0x3a1 | `sm70_encode.rs:4499` |
| 16..24 | Rd = mascara | GPR | `sm70_encode.rs:4501` |
| 24..32 | Ra = valor | GPR (64 bits si .U64: par de registros) | `sm70_encode.rs:4502` |
| 73 | .U64 | 0 = 32 bits, 1 = 64 bits | `sm70_encode.rs:4503` |
| 79 | modo | 1 = ANY, 0 = ALL | `sm70_encode.rs:4505-4514` |
| 81..84 | predicado destino (solo ALL) | P0..P6, 7=PT | `sm70_encode.rs:4516`; ANY exige PT `4509` |

### a) Codificacion REDUX (`sm70_encode.rs:2705-2736`)

| bits | nombre | valores |
|---|---|---|
| 0..12 | opcode | 0x3c4 |
| 16..22 | URd (siempre UGPR) | UGPR |
| 24..32 | Ra | GPR |
| 73 | con signo (solo MIN/MAX) | 1 = I32, 0 = U32 |
| 78..81 | operacion | AND=0, OR=1, XOR=2, SUM=3, MIN=4, MAX=5 |

### c) Cuando lo usa NAK

| NIR | instruccion | cita |
|---|---|---|
| `ballot` (bool -> u32) | VOTE.ANY con Rd = ballot, vote = PT | `from_nir.rs:2578-2592` |
| `vote_all` / `vote_any` / `vote_ieq` sobre bool | VOTE.ALL / .ANY / .EQ con pred destino, Rd=RZ | `from_nir.rs:3988-4008` |
| `vote_ieq` sobre 32/64 bits | MATCH.ALL (.U64 si 64) con pred destino | `from_nir.rs:4009-4022` |
| `match_any_nv` | MATCH.ANY con mascara | `from_nir.rs:4025-4044` |
| `reduce` (iand/ior/ixor/iadd/imin/imax/umin/umax, 32 bits, cluster 32, control no divergente, sm>=80) | REDUX a UGPR | `from_nir.rs:3764-3788`, `nak_nir_lower_scan_reduce.c:226-236` |
| resto de `reduce`/`inclusive_scan`/`exclusive_scan` | se bajan a secuencias (pase `nak_nir_lower_scan_reduce`) | `nak_nir_lower_scan_reduce.c:237-250`, `nak_nir.c:1717` |
| `vote_feq` | se baja en NIR (`.lower_vote_feq = true`) | `nak_nir.c:1654` |
| `first_invocation`, `read_first_invocation`, `elect`, `inverse_ballot`, `quad_vote_*` | se bajan en NIR | `nak_nir.c:1656-1660` |
| `quad_broadcast` | SHFL.IDX c=0x1c03 | `from_nir.rs:3727` |
| `quad_swap_horizontal/vertical/diagonal` | SHFL.BFLY lane=1/2/3 c=0x1c03 | `from_nir.rs:3740-3763` |

Opciones de subgrupo: medida `NAK_SUBGROUP_SIZE`, ballot de 32 bits y 1
componente (`nak_nir.c:1649-1662`).

### d) Dependencias

- VOTE: latencia fija (`ir.rs:8389`), categoria `CoupledAlu`
  (`sm80_instr_latencies.rs:176`), predicado `Disp_Alu` (`:898`) -> sin
  scoreboard. VOTEU uniforme: categoria `VoteU` (`:1143`, `:1342-1346`),
  tampoco scoreboard (solo `ToUr` lo necesita, `:1458-1462`).
- MATCH: no fija (`ir.rs:8387`), categoria `Decoupled` (`:177`, pred `:899`)
  -> SI scoreboard.
- REDUX: no fija (`ir.rs:8321`); en la tabla uniforme `R2UR | Redux`
  (`sm80_instr_latencies.rs:1129-...`). Necesidad exacta de scoreboard:
  depende de que su categoria uniforme sea `ToUr` (ver DUDAS).

---

## 4. SULD y SUST (imagenes y buffers con tipo)

### a) Codificacion SULD (`sm70_encode.rs:3321-3357`)

| bits | nombre | valores | cita |
|---|---|---|---|
| 0..12 | opcode | 0x998 = SULD.P (con formato), 0x99a = SULD.B (binario) | `sm70_encode.rs:3334-3343` |
| 16..24 | Rd | GPR (1, 2 o 4 registros) | `sm70_encode.rs:3345` |
| 24..32 | Ra = coordenadas | GPR (vector) | `sm70_encode.rs:3346` |
| 61..64 | dimension | 1D=0, 1D_BUFFER=1, 1D_ARRAY=2, 2D=3, 2D_ARRAY=4, 3D=5 | `sm70_encode.rs:3353`, `2833-2846` |
| 64..72 | Rc = handle (descriptor) | GPR | `sm70_encode.rs:3347` |
| 72..76 | .P: mascara de canales | solo 0x1 (R), 0x3 (RG), 0xf (RGBA) | `sm70_encode.rs:3341`, `2856-2867` |
| 73..76 | .B: tipo de memoria | U8=0, S8=1, U16=2, S16=3, B32=4, B64=5, B128=6 | `sm70_encode.rs:3337`, `3298-3312` |
| 77..81 | orden de memoria (sm>=80) | CONSTANT=0x4, WEAK=0x0, STRONG.CTA=0x5, STRONG.GPU=0x7, STRONG.SYS=0xa | `sm70_encode.rs:3354`, `3270-3281` |
| 81..84 | predicado "fault" (residencia sparse) | P0..P6, 7=PT | `sm70_encode.rs:3348` |
| 84..87 | politica de expulsion de cache | EF=0, EN=1, EL=2, LU=3, EU=4, NA=5 | `sm70_encode.rs:3355`, `3284-3296` |

(Para sm < 80 el campo 77..81 se divide en scope 77..79 y orden 79..81,
`sm70_encode.rs:3246-3269`; no aplica a SM86.)

### a) Codificacion SUST (`sm70_encode.rs:3359-3389`)

| bits | nombre | valores |
|---|---|---|
| 0..12 | opcode | 0x99c = SUST.P, 0x99e = SUST.B |
| 24..32 | Ra = coordenadas | GPR |
| 32..40 | Rb = datos | GPR (vector) |
| 61..64 | dimension | igual que SULD |
| 64..72 | Rc = handle | GPR |
| 72..76 / 73..76 | mascara (.P) / tipo (.B) | igual que SULD |
| 77..81 | orden de memoria | igual que SULD |
| 84..87 | expulsion | igual que SULD |

No tiene Rd ni predicado fault.

### b) Modificadores

- `.P` (formateado, el formato lo da el descriptor) frente a `.B` (binario,
  tipo de memoria en la instruccion) (`ir.rs:5649-5661`).
- Dimension: `.1D .1D_BUFFER .1D_ARRAY .2D .2D_ARRAY .3D`.
- Orden: `.CONSTANT .WEAK .STRONG.{CTA,GPU,SYS}`.
- Cache: `.EF .EN .EL .LU .EU .NA`.

### c) Cuando lo usa NAK y como pasa el formato

| NIR | instruccion | detalles | cita |
|---|---|---|---|
| `bindless_image_load` | SULD.P | mascara = 1, 2 o 4 canales; orden CONSTANT si `ACCESS_CAN_REORDER`, si no STRONG.GPU | `from_nir.rs:2862-2894` |
| `bindless_image_sparse_load` | SULD.P + pred fault | el ultimo componente = `sel(fault, 0, 1)` | `from_nir.rs:2895-2935` |
| `bindless_image_store` | SUST.P | STRONG.GPU | `from_nir.rs:2971-2994` |

- SM70+ siempre usa la forma `.P` (formateada): el formato viene del
  DESCRIPTOR, la instruccion solo lleva cuantos canales (1/2/4). La forma
  `.B` y `get_image_mem_type` solo aparecen en el camino de Kepler
  (`from_nir.rs:2222-2233`, usada en `2827`).
- Expulsion: `EF` si `ACCESS_NON_TEMPORAL`, si no `EN` (`from_nir.rs:2180-2189`).
- Dimension desde NIR (`from_nir.rs:2191-2220`): CUBE -> 2D_ARRAY, BUF ->
  1D_BUFFER, 1D/2D con o sin array, 3D. Numero de coordenadas
  (`ir.rs:2177-2188`): 1D=1, 1D_BUFFER=1, 1D_ARRAY=2, 2D=2, 2D_ARRAY=3, 3D=3.
  La muestra MSAA esta comentada (`from_nir.rs:2241`, `// let sample = ...`).

```rust
// nouveau/compiler/nak/from_nir.rs:2235-2244
fn get_image_coord(
    &mut self,
    intrin: &nir_intrinsic_instr,
    dim: ImageDim,
) -> Src {
    let vec = self.get_ssa(intrin.get_src(1).as_def());
    // let sample = self.get_src(&srcs[2]);
    let comps = usize::from(dim.coord_comps());
    SSARef::try_from(&vec[0..comps]).unwrap().into()
}
```

### Como se calcula la direccion de la imagen

- En SM70+/Maxwell+ NAK NO calcula direcciones: pasa coordenadas + handle y
  el hardware usa el descriptor. El calculo de direcciones en el sombreador
  (`nak_nir_lower_image_addrs`, SULDGA/SUSTGA con `nil_su_info`) solo se usa en
  Kepler: `/* On Kepler, we have to lower images to addresses */ if
  (pdev->info.cls_eng3d < MAXWELL_A)` (`nouveau/vulkan/nvk_shader.c:433-435`).
- Descriptor de imagen de almacenamiento en NVK: el MISMO formato de cabecera
  de textura (TIC) que para muestreo, generado por `nil_image_view_descriptor`
  con `NIL_VIEW_ACCESS_STORAGE` (`nouveau/vulkan/nvk_image_view.c:195-228`):
  - Vistas CUBE / CUBE_ARRAY se convierten en 2D_ARRAY (`:198-201`).
  - Vistas 3D: solo 1 mip; si hay rebanadas parciales -> `3D_SLICED` (`:203-218`).
- En el shader el "handle" de imagen de almacenamiento es un indice de 20 bits
  en la tabla de descriptores: `struct nvk_storage_image_descriptor {
  unsigned image_index:20; unsigned _pad:12; }`
  (`nouveau/vulkan/nvk_descriptor_types.h:52-55`).
- Buffers con tipo (texel buffers): descriptor 1D_BUFFER
  (`nouveau/nil/descriptor.rs:776-806`): direccion de 48 bits en bits 32..79,
  `HEADER_VERSION = SELECT_ONE_D_BUFFER (0)`, ancho-1 partido en
  bits 128..143 (15..0) y 96..111 (31..16), `TEXTURE_TYPE = ONE_D_BUFFER (6)`
  (bits segun `nouveau/headers/nvidia/classes/clb097tex.h:960-974`).
- Detalle de la cabecera TIC (bloque lineal / pitch): ver apartado 7.

### d) Dependencias

- SULD/SUST: no fijas (`ir.rs:8332-8336`), categoria `Decoupled`
  (`sm80_instr_latencies.rs:241-242`; pred `:889-890`) -> SI scoreboard.
  SUST usa rd_bar (para no pisar coordenadas/datos antes de que se lean);
  SULD usa wr_bar para el resultado.

---

## 5. ATOM, ATOMG, RED, ATOMS y SUATOM

### a) Tabla de operaciones (`sm70_encode.rs:3702-3718`)

| op | codigo |
|---|---|
| ADD | 0 |
| MIN | 1 |
| MAX | 2 |
| INC | 3 |
| DEC | 4 |
| AND | 5 |
| OR | 6 |
| XOR | 7 |
| EXCH | 8 |
| CAS | opcode aparte |

### a) Tabla de tipos, sm < 90 (SM86) (`sm70_encode.rs:3760-3773`)

Campo 73..76:

| tipo | codigo |
|---|---|
| U32 | 0 |
| S32 | 1 |
| U64 | 2 |
| F32 | 3 |
| F16x2 | 4 |
| S64 | 5 |
| F64 | 6 |

(En sm>=90 cambia a 73..77 con otra tabla, `sm70_encode.rs:3734-3759`; no aplica.)

### a) Memoria global (`sm70_encode.rs:3794-3865`, rama `MemSpace::Global`)

En SM86 `has_ugpr = sm >= 73` es verdadero (`sm70_encode.rs:3795`).

**RED (sin destino)** - opcode 0x98e (`:3798-3816`):

| bits | nombre | valores |
|---|---|---|
| 0..12 | opcode | 0x98e |
| 24..32 | Ra = direccion | GPR |
| 32..40 | Rb = dato | GPR |
| 40..64 | desplazamiento | 24 bits (i32) |
| 64..70 | URa = direccion uniforme | UGPR (URZ si no hay) |
| 72 | URa es de 64 bits | 1 si par de UGPR (URZ cuenta como 64) |
| 77..81 | orden | ver apartado 4 |
| 84..87 | expulsion | ver apartado 4 |
| 87..90 | operacion | 3 bits (ADD..XOR; EXCH=8 no cabe) |
| 90 | Ra es de 64 bits (.E) | 1 si par de GPR |
| 91 | 1 (fijo cuando hay UGPR) | 1 |
| 73..76 | tipo | tabla de arriba |

**ATOMG (con destino)** - opcode 0x9a8 (`:3827-3860`):

| bits | nombre | valores |
|---|---|---|
| 0..12 | opcode | 0x9a8 (0x3a8 si no hay UGPR, sm<73) |
| 16..24 | Rd | GPR (1 o 2 registros) |
| 24..32 | Ra = direccion | GPR |
| 32..40 | Rb = dato | GPR |
| 40..64 | desplazamiento | 24 bits |
| 64..70 | URa | UGPR |
| 70 | Ra es de 64 bits (.E) | 0/1 |
| 72 | URa es de 64 bits | 0/1 |
| 73..76 | tipo | tabla |
| 77..81 | orden | |
| 81..84 | pred destino | 7 = PT |
| 84..87 | expulsion | |
| 87..91 | operacion | 4 bits (incluye EXCH=8) |
| 91 | hay UGPR | 1 |

**ATOMG.CAS** - opcode 0x3a9 (`:3817-3826`):

| bits | nombre |
|---|---|
| 16..24 | Rd |
| 24..32 | Ra = direccion |
| 72 | Ra es de 64 bits |
| 32..40 | Rb = valor de comparacion |
| 40..64 | desplazamiento |
| 64..72 | Rc = valor nuevo |
| 81..84 | pred = PT |
| 73..76 / 77..81 / 84..87 | tipo / orden / expulsion |

**ATOMS (memoria compartida)** (`:3867-3907`): opcode 0x98c (con UGPR, URa en
64..70 y bit 91 = 1) o 0x38d para CAS (Rb=cmp en 32..40, Rc=dato en 64..72);
Ra 24..32, desplazamiento 40..64, escala del desplazamiento en 78..80
(`OffsetStride::encode_sm75`, `sm70_encode.rs:461-477`), op 87..91, tipo
73..76; exige orden STRONG.CTA y expulsion EN (`:3903-3906`).

### a) SUATOM / SURED (`sm70_encode.rs:3391-3426`)

| bits | nombre | valores |
|---|---|---|
| 0..12 | opcode | 0x3a0 = SURED (sin destino, op en 87..90), 0x394 = SUATOM (op en 87..91), 0x396 = SUATOM.CAS |
| 16..24 | Rd | GPR |
| 24..32 | Ra = coordenadas | GPR |
| 32..40 | Rb = datos (en CAS: [cmp, dato] empaquetados, cmp primero) | GPR |
| 61..64 | dimension | igual que SULD |
| 64..72 | Rc = handle | GPR |
| 72 | .BA | siempre 0 |
| 73..76 | tipo | tabla de arriba (su=true) |
| 77..81 | orden | |
| 81..84 | pred fault | 7 = PT |
| 84..87 | expulsion | |
| 87..90 / 87..91 | operacion | |

### b) Modificadores

Operacion (tabla), tipo (`.U32 .S32 .U64 .F32 .F16x2 .S64 .F64`), orden
`.STRONG.GPU` etc., cache, `.E` (bit de 64 bits de la direccion: 90 en RED,
70 en ATOMG, 72 en CAS).

### c) Cuando lo usa NAK

Tipo segun la operacion NIR (`from_nir.rs:2134-2155`) y operacion
(`from_nir.rs:2157-2178`):

| NIR atomic_op | op | tipo |
|---|---|---|
| iadd | ADD | U32/U64 |
| imin / imax | MIN / MAX | S32/S64 |
| umin / umax | MIN / MAX | U32/U64 |
| iand / ior / ixor | AND / OR / XOR | U32/U64 |
| xchg | EXCH | U(bits x componentes) |
| fadd / fmin / fmax | ADD / MIN / MAX | F16x2 / F32 / F64 |
| cmpxchg | CAS | U32/U64 |

| NIR | instruccion | cita |
|---|---|---|
| `global_atomic_nv` cuyo resultado no se lee y la op es de reduccion | RED (0x98e) | `from_nir.rs:3057-3090` (`is_reduction` en `3068-3069`) |
| `global_atomic_nv` con resultado | ATOMG (0x9a8) | idem |
| `global_atomic_swap_nv` | ATOMG.CAS (0x3a9), cmp separado | `from_nir.rs:3091-3117` |
| `shared_atomic_nv` / `shared_atomic_swap_nv` | ATOMS / ATOMS.CAS | `from_nir.rs:3789-...` |
| `bindless_image_atomic` sin uso del resultado (op de reduccion, sm>=70) | SURED | `from_nir.rs:2678-2686` |
| `bindless_image_atomic` | SUATOM | `from_nir.rs:2643-2699` |
| `bindless_image_atomic_swap` | SUATOM.CAS (cmp+dato empaquetados) | `from_nir.rs:2656-2673` |

Todas las atomicas globales y de imagen se emiten con
`MemSpace::Global(MemAddrType::A64)` y `MemOrder::Strong(MemScope::GPU)`
(`from_nir.rs:3085-3086`, `3112-3113`, `2694`). Las globales siempre con
expulsion `Normal` (`from_nir.rs:3087`).

Atomicas compartidas que se bajan a bucle CAS en Maxwell+
(`nak_nir.c:1183-1195`, `1667-1670`): 64 bits aritmeticas, fadd de 32 bits y
f16vec2. Los f16vec4 se parten en 2 x f16vec2 (`nak_nir.c:927-1041`).

### Global contra memoria del sistema

NO ENCONTRADO: NAK no distingue memoria de video y memoria del sistema en la
instruccion. Siempre usa `MemSpace::Global(A64)` con scope `GPU`. El codigo de
orden `STRONG.SYS = 0xa` existe en el codificador (`sm70_encode.rs:3278`)
pero `from_nir.rs` no lo emite para atomicas; en barreras el scope maximo que
genera es GPU (`from_nir.rs:3680-3690`).

### d) Dependencias

- ATOM: no fija (`ir.rs:8345`), categoria `DecoupledAgu`
  (`sm80_instr_latencies.rs:237`, pred `:864`) -> SI scoreboard.
- SUATOM: `Decoupled` (`sm80_instr_latencies.rs:243`) -> SI scoreboard.
- RED sin destino solo necesita rd_bar si alguien sobrescribe sus fuentes.

---

## 6. TEX con LOD implicito

### a) Codificacion TEX en SM86 (`sm70_encode.rs:2887-2954`)

| bits | nombre | valores | cita |
|---|---|---|---|
| 0..12 | opcode | 0x361 (bindless) / 0xb60 (cbuf) | `sm70_encode.rs:2897-2909` |
| 16..24 | Rd (primer destino) | GPR | `:2913` |
| 24..32 | Ra (primer bloque de fuentes) | GPR | `:2921` |
| 32..40 | Rb (segundo bloque de fuentes) | GPR, RZ si no hay | `:2922` |
| 40..59 | (solo 0xb60) ref. cbuf: offset/4 en 40..54 (14 bits), indice en 54..59 (5 bits) | | `:2900`, `2739-2745` |
| 59 | .B (bindless) | 1 con 0x361 | `:2908` |
| 60 | .SCR (scalar) | 0/1 | `:2928` |
| 61..64 | dimension | 1D=0, 2D=1, 3D=2, CUBE=3, ARRAY_1D=4, ARRAY_2D=5, ARRAY_CUBE=7 | `:2929`, `2747-2761` |
| 64..72 | Rd2 (segundo destino) | GPR, 255 si no hay | `:2914-2918` |
| 72..76 | mascara de canales | 4 bits RGBA | `:2930`, `2848-2854` |
| 76 | .AOFFI | 0/1 | `:2942` |
| 77 | .NDV | 0 = Auto, 1 = NonDivergent | `:2943`, `2800-2807` |
| 78 | .DC (comparacion de profundidad) | 0/1 | `:2945` |
| 81..84 | pred fault (sparse) | 7=PT | `:2919` |
| 84..87 | expulsion | ver apartado 4 | `:2946` |
| **87..90** | **modo LOD** | **Auto=0**, LZ=1, LB=2, LL=3, LC=4, LB.LC=5 | `:2950`, `2763-2778` |
| 90 | .NODEP | 0/1 | `:2952` |

```rust
// nouveau/compiler/nak/sm70_encode.rs:2763-2778
fn set_tex_lod_mode(&mut self, range: Range<usize>, lod_mode: TexLodMode) {
    assert!(range.len() == 3);
    assert!(self.sm <= 100);
    self.set_field(
        range,
        match lod_mode {
            TexLodMode::Auto => 0_u8,
            TexLodMode::Zero => 1_u8,
            TexLodMode::Bias => 2_u8,
            TexLodMode::Lod => 3_u8,
            TexLodMode::Clamp => 4_u8,
            TexLodMode::BiasClamp => 5_u8,
        },
    );
}
```

### b) Modificadores

Display: Auto = "" (sin sufijo), `.lz`, `.lb`, `.ll`, `.lc`, `.lb.lc`
(`ir.rs:2040-2051`). "LOD implicito" = Auto (0), y tambien Bias (2), Clamp (4)
y BiasClamp (5) se consideran no explicitos (`ir.rs:2028-2038`,
`is_explicit_lod`).

### c) Cuando lo usa NAK (`nouveau/compiler/nak_nir_lower_tex.c:261-293`)

```c
/* nouveau/compiler/nak_nir_lower_tex.c:261-278 */
enum nak_nir_lod_mode lod_mode = NAK_NIR_LOD_MODE_AUTO;
if (tex->op == nir_texop_txf_ms) {
   /* Multisampled textures do not have miplevels */
   lod_mode = NAK_NIR_LOD_MODE_ZERO;
   lod = NULL; /* We don't need this */
} else if (lod != NULL) {
   nir_scalar lod_s = { .def = lod, .comp = 0 };
   if (nir_scalar_is_const(lod_s) &&
       nir_scalar_as_uint(lod_s) == 0) {
      lod_mode = NAK_NIR_LOD_MODE_ZERO;
      lod = NULL; /* We don't need this */
   } else {
      lod_mode = NAK_NIR_LOD_MODE_LOD;
   }
} else if (bias != NULL) {
   lod_mode = NAK_NIR_LOD_MODE_BIAS;
   lod = bias;
}
```

- `tex` sin lod ni bias -> Auto (0). Con `min_lod`: Auto -> Clamp (4),
  Bias -> BiasClamp (5); `min_lod` se pasa como `f2u32(max(min_lod*256, 16))`
  (`nak_nir_lower_tex.c:280-293`).
- Paso a IR: `from_nir.rs:1999-2007`.
- En etapas sin derivadas implicitas, NIR convierte el LOD implicito en LOD
  cero antes de NAK: `.lower_invalid_implicit_lod = true`
  (`nak_nir.c:389`), que llama a `lower_zero_lod` si
  `!nir_shader_supports_implicit_lod` (`compiler/nir/nir_lower_tex.c:1885-1891`
  **[FUERA DE src/nouveau, en src/compiler]**). Es decir: TEX con Auto solo
  llega en fragment (y compute con derivadas, si NIR lo permite).
- `deriv_mode` en SM86 = Auto -> bit 77 = 0 (`from_nir.rs:2009-2018`).
- `txd` (gradientes explicitos) va a TXD, `lod` (consulta) a TMML
  (`from_nir.rs:2035-2060`).

### d) Dependencias

- TEX: no fija (`ir.rs:8324-8329`), categoria `Decoupled`
  (`sm80_instr_latencies.rs:224`, pred `:891`) -> SI scoreboard.
- Sin TEXDEPBAR en Ampere (`calc_instr_deps.rs:600`).

---

## 7. Texturas BC1..BC7 en el descriptor (TIC), pitch y mips

### 7.1 Formatos que pone NIL (`nouveau/nil/nil_formats.csv:241-263`)

Columnas: nombre pipe | componentes TIC | tipo por canal | swizzle.
Valores de `COMPONENTS` (bits 0..7 = `MW(6:0)`) de
`nouveau/headers/nvidia/classes/clb097tex.h:294-301`.

| Vulkan (BCn) | PIPE_FORMAT | COMPONENTS | valor | tipos R,G,B,A | swizzle X,Y,Z,W | sRGB |
|---|---|---|---|---|---|---|
| BC1 RGB | DXT1_RGB | DXT1 | 0x24 | UNORM x4 | R,G,B,1 | no |
| BC1 RGB sRGB | DXT1_SRGB | DXT1 | 0x24 | UNORM x4 | R,G,B,1 | si |
| BC1 RGBA | DXT1_RGBA | DXT1 | 0x24 | UNORM x4 | R,G,B,A | no |
| BC1 RGBA sRGB | DXT1_SRGBA | DXT1 | 0x24 | UNORM x4 | R,G,B,A | si |
| BC2 | DXT3_RGBA / DXT3_SRGBA | DXT23 | 0x25 | UNORM x4 | R,G,B,A | no / si |
| BC3 | DXT5_RGBA / DXT5_SRGBA | DXT45 | 0x26 | UNORM x4 | R,G,B,A | no / si |
| BC4 UNORM | RGTC1_UNORM | DXN1 | 0x27 | UNORM x4 | R,0,0,1 | no |
| BC4 SNORM | RGTC1_SNORM | DXN1 | 0x27 | SNORM x4 | R,0,0,1 | no |
| BC5 UNORM | RGTC2_UNORM | DXN2 | 0x28 | UNORM x4 | R,G,0,1 | no |
| BC5 SNORM | RGTC2_SNORM | DXN2 | 0x28 | SNORM x4 | R,G,0,1 | no |
| BC6H SFLOAT | BPTC_RGB_FLOAT | BC6H_SF16 | 0x10 | FLOAT x4 | R,G,B,1 | no |
| BC6H UFLOAT | BPTC_RGB_UFLOAT | BC6H_UF16 | 0x11 | FLOAT x4 | R,G,B,1 | no |
| BC7 | BPTC_RGBA_UNORM / BPTC_SRGBA | BC7U | 0x17 | UNORM x4 | R,G,B,A | no / si |

(Tambien LATC1/LATC2 con DXN1/DXN2, `nil_formats.csv:255-258`.)

- Codigos de tipo (`clb097tex.h:344-375`): SNORM=1, UNORM=2, SINT=3, UINT=4,
  FLOAT=7. Campos R 7..10, G 10..13, B 13..16, A 16..19.
- Codigos de swizzle (`clb097tex.h:376-383`): ZERO=0, R=2, G=3, B=4, A=5,
  ONE_INT=6, ONE_FLOAT=7. Campos X 19..22, Y 22..25, Z 25..28, W 28..31.
  El "1" es ONE_FLOAT si el canal de referencia es S/N/F
  (`nouveau/nil/nil_format_table_gen.py:194-210`).
- El swizzle final de la vista se compone con el del formato
  (`nouveau/nil/descriptor.rs:73-93`, `122-145`).
- sRGB: NO va en COMPONENTS; se pone el bit `S_R_G_B_CONVERSION` (bit 150)
  con `view.format.is_srgb()` (`descriptor.rs:522`,
  `clb097tex.h:470`).
- Todos solo tienen capacidad `T` (textura), no almacenamiento ni render.

### 7.2 Cabecera TIC Maxwell-B..Ampere (`nvb097_fill_image_view_desc`, `descriptor.rs:400-559`)

Se usa para Maxwell A .. Ada (`descriptor.rs:925-932`; Hopper+ usa otra).
Bits del descriptor de 256 bits (`clb097tex.h:266-544`):

| bits | campo | valor que pone NIL |
|---|---|---|
| 0..7 | COMPONENTS | tabla 7.1 |
| 7..19 | tipos R,G,B,A | tabla 7.1 |
| 19..31 | X,Y,Z,W SOURCE | swizzle |
| 41..64 | ADDRESS_BITS31TO9 (bloque lineal) | direccion >> 9 (alineada a 512) |
| 64..80 | ADDRESS_BITS47TO32 | |
| 85..88 | HEADER_VERSION | BLOCKLINEAR=3, PITCH=2, ONE_D_BUFFER=0 |
| 96..99 | GOBS_PER_BLOCK_WIDTH | ONE_GOB |
| 99..102 | GOBS_PER_BLOCK_HEIGHT | `tiling.y_log2` |
| 102..105 | GOBS_PER_BLOCK_DEPTH | `tiling.z_log2` |
| 106..109 | TILE_WIDTH_IN_GOBS | `tiling.x_log2` |
| 112 / 113 / 114 | LOD_ANISO_QUALITY2 / ANISO_QUALITY / ISO_QUALITY | 1 / HIGH / HIGH |
| 124..128 | MAX_MIP_LEVEL | `num_levels - 1` (0 si vista simple de 1 mip y 1 capa) (`descriptor.rs:220-248`) |
| 128..144 | WIDTH_MINUS_ONE | |
| 150 | S_R_G_B_CONVERSION | sRGB |
| 151..155 | TEXTURE_TYPE | 1D=0, 2D=1, 3D=2, CUBE=3, 1D_ARRAY=4, 2D_ARRAY=5, 1D_BUFFER=6, 2D_NO_MIPMAP=7, CUBE_ARRAY=8 |
| 160..176 + bit 146 | HEIGHT_MINUS_ONE (+bit16 en Pascal+) | (`clc097tex.h:235`) |
| 176..190 + bit 145 | DEPTH_MINUS_ONE (+bit14 en Pascal+) | (`clc097tex.h:234`) |
| 191 | NORMALIZED_COORDS | 1 |
| 224..228 | RES_VIEW_MIN_MIP_LEVEL | `view.base_level` |
| 228..232 | RES_VIEW_MAX_MIP_LEVEL | `base_level + num_levels - 1` |
| 232..236 | MULTI_SAMPLE_COUNT | segun muestras |
| 236..248 | MIN_LOD_CLAMP | punto fijo |

Variante PITCH (lineal) (`descriptor.rs:457-488`, bits `clb097tex.h:1150-1162`):

| bits | campo | valor |
|---|---|---|
| 37..64 | PITCH_ADDRESS_BITS31TO5 | direccion >> 5 (alineada a 32) |
| 64..80 | ADDRESS_BITS47TO32 | |
| 85..88 | HEADER_VERSION | SELECT_PITCH = 2 |
| 96..112 | PITCH_BITS20TO5 | `row_stride_B >> 5` (multiplo de 32, < 2^21) |
| 151..155 | TEXTURE_TYPE | TWO_D_NO_MIPMAP = 7 |

```rust
// nouveau/nil/descriptor.rs:457-488 (recortado)
set_enum!(th, clb097, TEXHEAD_BL_HEADER_VERSION, SELECT_PITCH);
...
let pitch = image.levels[0].row_stride_B;
let pitch = BitView::new(&pitch);
assert!(pitch.get_bit_range_u64(0..5) == 0);
assert!(pitch.get_bit_range_u64(21..32) == 0);
th.set_field(
    clb097::TEXHEAD_PITCH_PITCH_BITS20TO5,
    pitch.get_bit_range_u64(5..21),
);
assert!(
    view.view_type == ViewType::_2D
        || view.view_type == ViewType::_2DArray
);
assert!(image.sample_layout == SampleLayout::_1x1);
assert!(view.num_levels == 1);
set_enum!(th, clb097, TEXHEAD_PITCH_TEXTURE_TYPE, TWO_D_NO_MIPMAP);
```

### 7.3 Paso de fila y medida de cada mip

Conversion pixeles -> bloques -> bytes:

- Tamano de elemento = `block.bits / 8` del formato (BC1/BC4 = 8 bytes,
  resto BC = 16 bytes segun util_format) (`nouveau/nil/format.rs:42-46`).
- Dimensiones de bloque = `block.width/height/depth` (4x4x1 en BC)
  (`format.rs:48-51`).
- px -> elementos con redondeo hacia arriba (`div_ceil`)
  (`nouveau/nil/extent.rs:114-120`, `85-93`); elementos -> bytes multiplica
  solo el ancho por el medida de elemento (`extent.rs:176-181`).
- Mip: `max(1, x >> level)` en pixeles, por dimension, ANTES de pasar a
  bloques (`nouveau/nil/lib.rs:36-40`, `extent.rs:340-349`,
  `image.rs:491-507`).

Imagen con tiling (bloque lineal), `new_tiled` (`nouveau/nil/image.rs:383-430`):

```rust
// nouveau/nil/image.rs:384-414
for level in 0..info.levels {
    let mut lvl_ext_B = image.level_extent_B(level);
    lvl_ext_B.array_len = 1;
    let lvl_tiling = tiling.clamp(lvl_ext_B);
    if tiling != lvl_tiling {
        image.mip_tail_first_lod =
            std::cmp::min(image.mip_tail_first_lod, level);
    }
    // Align the size to tiles
    let lvl_tiling_ext_B = lvl_tiling.extent_B();
    lvl_ext_B = lvl_ext_B.align(&lvl_tiling_ext_B);
    ...
    image.levels[level as usize] = ImageLevel {
        offset_B: layer_size_B,
        tiling: lvl_tiling,
        row_stride_B: lvl_ext_B.width,
    };
    layer_size_B += lvl_ext_B.size_B();
}
```

- GOB en Turing/Ampere (color): 64 bytes x 8 filas x 1
  (`nouveau/nil/tiling.rs:138-143`, `157-168`). Bloque = GOB << (x_log2,
  y_log2, z_log2) (`tiling.rs:289-298`).
- Eleccion de y_log2 para 2D segun alto en pixeles: >=86 -> 4, >=43 -> 3,
  >=22 -> 2, >=11 -> 1, si no 0; 3D usa z_log2 segun profundidad
  (`tiling.rs:402-422`); luego `clamp` al medida real (`tiling.rs:253-277`),
  que tambien se aplica por mip.
- Paso de fila de cada mip = ancho en bytes alineado al ancho del bloque
  (64 << x_log2); medida del mip = ancho_alineado x alto_alineado x
  prof_alineada; offset del mip = suma de los anteriores.
- Paso entre capas (`array_stride_B`) = suma de mips alineada al medida del
  bloque del mip 0 (`image.rs:422-430`). Alineacion minima 4096
  (`image.rs:454-455`).
- Direccion en el TIC de bloque lineal: base + `base_array_layer *
  array_stride_B`; no hay campo de capa base (`descriptor.rs:417-433`).

Imagen lineal (pitch), `new_linear` (`image.rs:228-293`):

```rust
// nouveau/nil/image.rs:242-274 (recortado)
let mut align_B = if info.explicit_row_stride_B > 0 {
    ...
    32
} else {
    // If we get to pick the alignment, require 128B so that we can
    // render to the image without workarounds.
    128
};
...
let row_stride_B = if info.explicit_row_stride_B > 0 {
    debug_assert!(info.explicit_row_stride_B % align_B == 0);
    info.explicit_row_stride_B
} else {
    extent_B.width.next_multiple_of(align_B)
};
...
let size_B = u64::from(row_stride_B) * u64::from(extent_B.height);
```

### 7.4 Texturas lineales con mas de una mip

NO. Tres sitios lo impiden:

- NIL: `// NVIDIA can't do linear and mipmapping` `assert!(info.levels == 1);`
  (`nouveau/nil/image.rs:236-237`); tambien solo 2D, sin arrays ni MSAA
  (`image.rs:232-239`).
- NIL descriptor pitch: `assert!(view.num_levels == 1)` y tipo
  `TWO_D_NO_MIPMAP` (`descriptor.rs:486-487`).
- NVK: `if (... tiling == VK_IMAGE_TILING_LINEAR) maxMipLevels = 1;`
  y `maxArraySize = 1` (`nouveau/vulkan/nvk_image.c:504-510`).

---

## DUDAS

1. **Signo exacto de FSWZADD `sub`/`subr`.** NAK solo da nombres y codigos
   (SubLeft="sub"=1, SubRight="subr"=2). No hay en src/nouveau ningun texto ni
   emulador que diga si `sub` es src0-src1 o src1-src0. Ademas, el nombre
   interno (`SubLeft`/`SubRight`) no coincide con el texto mostrado
   (`ir.rs:2974-2975`: `SubRight => "subr"`, `SubLeft => "sub"`). El resultado
   es coherente en los 4 hilos con cualquiera de las dos lecturas, pero el
   signo global (derecha-izquierda o izquierda-derecha) hay que comprobarlo en
   hardware.
2. **Orden "arriba/abajo" del cuadro en el sombreador de pixeles.** NAK solo
   fija hilo = x_bit0 | y_bit1 (en compute y en los quad_swap). Que y+1 sea
   abajo en pantalla solo lo dice el comentario del compilador viejo de gallium
   (`UL UR LL LR`).
3. **Codigos FSWZADD en SM50 frente a SM70.** NAK codifica SubLeft=1 y
   SubRight=2 tanto en SM50 (`sm50.rs:933-936`) como en SM70
   (`sm70_encode.rs:1152-1153`). El compilador viejo **[FUERA DE src/nouveau]**
   define `QOP_SUBR 1`, `QOP_SUB 2` para GM107
   (`nv50_ir_lowering_gm107.cpp:34-37`) y en GV100 intercambia 1<->2
   ("NP/PN swapped vs SM60", `nv50_ir_emit_gv100.cpp:571-577`). En SM70+ los
   dos compiladores coinciden (sub=1, subr=2), que es lo que importa para
   SM86; en SM50 no coinciden, asi que uno de los dos nombra al reves.
4. **Constante `c` de SHFL.** El codigo usa 0x1f (subgrupo entero), 0 (UP)
   y 0x1c03 (cuadro) y enmascara con 0x1f1f, pero no documenta que bits son
   "clamp" y cuales "mascara de segmento". Interpretacion probable (no
   escrita en el codigo): bits 0..4 = clamp, bits 8..12 = mascara de segmento.
5. **Predicado "in_bounds" de SHFL (81..84).** NAK siempre lo deja en PT;
   no hay uso que confirme su significado.
6. **Bit 91 en RED/ATOMG.** Se pone a 1 cuando hay UGPR
   (`sm70_encode.rs:3812`, `3859`) sin comentario sobre que significa.
7. **Bit de 64 bits de la direccion distinto por forma:** 90 en RED, 70 en
   ATOMG (sm<100), 72 en CAS y en la forma sin UGPR. Esta asi en el codigo,
   sin explicacion.
8. **Float min/max atomicos en SM86.** El codificador acepta F32 + MIN/MAX
   con la tabla de sm<90, pero no he comprobado si NVK expone esas
   operaciones en SM86 ni si el hardware las soporta (no visto en lo leido).
9. **REDUX y scoreboard.** REDUX no es de latencia fija y escribe un UGPR;
   que necesite scoreboard depende de que su categoria uniforme SM80 sea
   `ToUr` (`sm80_instr_latencies.rs:1129` y `1457-1462`). No he leido la rama
   completa de `URegLatencySM80::op_category` para `Redux`.
10. **DEFAULT_PARTIAL de SET_SHADER_CONTROL** (que pasa con las derivadas en
    cuadros parciales con modo Auto): NVK no lo programa en ningun `.c` de
    `nouveau/vulkan/` (grep sin resultados); el valor por defecto no aparece
    en el codigo leido.
11. **Imagenes MSAA de almacenamiento en SM70+:** `get_image_coord` ignora el
    indice de muestra (comentado). No he localizado donde se convierte la
    muestra en coordenadas para SULD/SUST en Ampere.
12. **Tamano de bloque de BC.** NIL lo saca de `util_format_description`
    (fuera de src/nouveau); los valores 8/16 bytes y 4x4 son los de
    util_format, no constantes escritas en NIL.
