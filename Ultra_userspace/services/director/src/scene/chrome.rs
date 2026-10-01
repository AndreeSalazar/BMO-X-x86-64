//! **El MARCO**: lo que toda ventana tiene y ninguna deberia escribir dos veces.
//!
//! [consumo] NADA      no corre en reposo por su cuenta: pinta cuando el
//!                     compositor se lo pide, y el compositor solo pinta si
//!                     algo cambio (L6h)
//!
//! === Por que existe ===
//!
//! Habia tres ventanas --Ejecutar, Datos y el log del kernel-- y **tres copias
//! de lo mismo**: cada una con su geometria, su pintado de borde y su barra de
//! titulo. En cuanto entro el arrastre, pasaron a ser tres copias de un
//! algoritmo, que es la forma en que dos ventanas del mismo sistema acaban
//! comportandose distinto sin que nadie lo decida.
//!
//! Agregar minimizar, maximizar y cerrar a cada una habria sido escribir tres
//! veces la misma maquina de estados. Aqui se escribe una.
//!
//! * **El criterio no es la estetica: es que la CUARTA ventana salga gratis.**
//!
//! === Lo que el marco SI sabe, y lo que no ===
//!
//! Sabe de rectangulos, de agarres y de botones. **No sabe que hay dentro** --
//! ni una linea de este modulo menciona ESTRATOS, el log ni la caja de lanzar.
//! Es la misma ley que separa `bmo-lower` de los frontends: se comparten
//! contratos, nunca cerebros.

use bmo_userland as bmo;

use super::*;

/// Los tres botones de la esquina, en el orden de todos los escritorios.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Button {
    Minimize,
    Maximize,
    Close,
}

/// Hacia donde va un gesto de teclado.
///
/// Es un rumbo y no un `(dx, dy)` porque los cuatro gestos NO son simetricos:
/// `Up` maximiza y `Down` restaura, mientras que los lados encajan una
/// mitad. Con un par de numeros habria que reconstruir cual era cual a base de
/// comparaciones, que es como se acaba maximizando al pulsar izquierda.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Heading {
    Left,
    Right,
    Up,
    Down,
}

/// Cuanto se mueve una ventana por pulsacion.
///
/// Veinticuatro pixeles: se ve que se movio sin tener que mirar dos veces, y
/// cruzar una pantalla de 1920 cuesta unas ochenta pulsaciones -- que suena a
/// mucho hasta que se recuerda que para eso estan `Shift` y las mitades.
pub(crate) const KEY_STEP: u32 = 24;

/// Lado de la zona sensible de cada boton. Veinticuatro pixeles se aciertan sin
/// mirar; doce obligan a apuntar, y apuntar para CERRAR una ventana es como se
/// cierra la que no era.
pub(crate) const BTN_SIDE: u32 = 24;
/// Lo que se puede agarrar en la esquina para estirar.
const GRIP_CORNER: u32 = 16;

/// El rojo de cerrar cuando el puntero esta encima. Es el unico sitio del
/// sistema donde el rojo no significa "algo va mal" sino "esto destruye", y por
/// eso solo aparece al senalarlo: un aspa roja permanente es una alarma de
/// fondo.
const CLOSE_HOVER: u32 = 0x00C4_2B1F;
/// El realce de los otros dos: un nivel mas claro, sin color propio.
const BTN_HOVER: u32 = 0x003A_3358;

/// Geometria y estado de una ventana. **Lo unico que hay que llevar.**
pub(crate) struct Chrome {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// Por debajo de esto no se puede encoger. Una ventana que se puede dejar
    /// inservible con el raton es una trampa, no una libertad.
    min_w: u32,
    min_h: u32,
    /// Donde se agarro DENTRO de la ventana, si se esta arrastrando.
    ///
    /// Se guarda el agarre y no la posicion del puntero: si no, la ventana pega
    /// un salto al empezar y se coloca con su esquina bajo el raton en vez de
    /// quedarse donde la cogiste.
    drag: Option<(u32, u32)>,
    resizing: bool,
    /// La geometria de antes de maximizar, para poder volver.
    saved: Option<(u32, u32, u32, u32)>,
    /// == ** PANTALLA COMPLETA NO ES MAXIMIZAR (2026-09-11) ===========
    ///
    /// Y por eso es OTRO estado y no un `bool` mas en `saved`. Maximizar
    /// deja la barra del sistema a la vista **a proposito** --lo dice
    /// `toggle_maximized`: una ventana que tapa la barra esconde las fichas
    /// de las demas--. Pantalla completa hace justo lo contrario: se la
    /// come, quita el marco entero y se queda con el panel. Es la
    /// configuracion de *pantalla completa* de un juego, y se pone y se
    /// quita con Alt+Enter sin relanzar nada.
    ///
    /// Guarda la geometria de antes, igual que `saved`, y pueden coexistir:
    /// de pantalla completa se vuelve a lo que hubiera, maximizado incluido.
    fs: Option<(u32, u32, u32, u32)>,
    /// Abierta pero escondida. **No es lo mismo que cerrada**: una minimizada
    /// conserva su sitio, su medida y lo que estuviera mirando.
    pub(crate) minimized: bool,
    /// Que boton tiene el puntero encima, para realzarlo. Se lleva como estado
    /// porque el realce solo se repinta cuando CAMBIA -- repintarlo cada
    /// fotograma son 1.700 pixeles de memoria de video sin cache para dejarlo
    /// igual.
    pub(crate) hover: Option<Button>,
    /// ** Se puede CERRAR esta ventana?
    ///
    /// Casi siempre si, y por eso es `true` en los dos constructores. La
    /// excepcion es la TERMINAL: es el unico sitio del escritorio donde se
    /// escriben ordenes, y al shell de Ring 0 **no se vuelve** una vez el
    /// compositor arranca. Una aspa que deja la maquina sin linea de ordenes no
    /// es una libertad, es el mismo error que `fb::rescue` ya se niega a
    /// cometer: *no se echa al que sostiene la casa*.
    ///
    /// Minimizar SI puede, que es lo que de verdad se quiere cuando estorba --
    /// y de ahi vuelve con Alt+Tab o con su atajo.
    closable: bool,
    /// ** TIENE EL FOCO (2026-09-22, HUD 2). El marco no sabe QUE ventana es,
    /// y no lo necesita: sabe si las teclas van a ella, y entonces su borde es
    /// del color de acento (el `col.active_border` de Hyprland). Lo pone UN
    /// sitio -- `desktop::foco::seguir` -- cuando el foco cambia.
    pub(crate) foco: bool,
    /// ** SIN MARCO (01-10): ni barra de titulo ni titulo -- un borde de un
    /// pixel que se enciende, y los tres botones que ASOMAN arriba a la
    /// derecha cuando el puntero se acerca. Lo pidio el propietario para la
    /// LUDOTECA: *"no necesitan bordes... solo minimizar, maximizar y cerrar"*.
    pub(crate) sin_marco: bool,
    /// Cuanto asoman los botones, de 0 (escondidos) a [`ASOMA`] (fuera).
    pub(crate) asoma: u8,
}

/// Los pasos de asomar: 8 fotogramas de 33 ms, un cuarto de segundo.
pub(crate) const ASOMA: u8 = 8;
/// Sin marco: la franja de arriba que sirve de asa (siempre).
const ASA: u32 = 8;
/// Sin marco: lo cerca de arriba que tiene que estar el puntero para que
/// asomen los botones.
pub(crate) const ZONA_ASOMA: u32 = 44;
/// La pastilla de los botones: su alto y lo que se separa del borde.
const PASTILLA_H: u32 = 28;
const PASTILLA_AIRE: u32 = 8;
/// La segunda luz del borde vivo, la que corre al reves: el magenta del glitch.
const MAGENTA: u32 = 0x00FF_2E88;

/// ** LOS HUECOS (HUD 2): lo que queda libre entre dos ventanas encajadas, y
/// entre una ventana y la barra o el borde de la pantalla. Los `gaps` de
/// Hyprland: una ventana pegada a otra se lee como UNA; con aire se leen dos.
pub(crate) const HUECO: u32 = 8;

/// **El sitio donde caben las ventanas**: la pantalla menos la barra y menos
/// los huecos. Encajar, maximizar y el mosaico miden AQUI y en ningun otro
/// sitio: el dia que la barra lateral ocupe su columna, cambia esta cuenta y
/// nada mas.
pub(crate) fn area_util(p: &bmo::Pantalla) -> (u32, u32, u32, u32) {
    // Sin barra de arriba desde el HUD 5: la pantalla entera menos el panel de
    // la izquierda y los huecos.
    let izq = super::lateral::margen();
    (
        izq + HUECO,
        HUECO,
        p.ancho.saturating_sub(izq + 2 * HUECO),
        p.alto.saturating_sub(2 * HUECO),
    )
}

/// **Lo mas a la izquierda que puede estar una ventana**: la columna de la
/// barra lateral es RESERVADA (HUD 3). Los topes del arrastre, de las flechas y
/// de `fit` la leen aqui, para que ninguna mano deje una ventana encima.
fn tope_izq() -> u32 {
    super::lateral::margen()
}

impl Chrome {
    /// Una ventana nueva, **en fracciones de la pantalla**.
    ///
    /// === Por que no en pixeles ===
    ///
    /// Porque `640 x 330` es un medida correcto en la pantalla del que lo
    /// escribio y ninguna otra. En una 4K es un sello de correos; en una
    /// 1024x768 no cabe. Un medida en tantos por ciento se adapta solo, y los
    /// minimos --que si son absolutos, porque el texto mide lo que mide--
    /// impiden que en una pantalla chica quede ilegible.
    pub(crate) fn new(
        p: &bmo::Pantalla,
        pct_w: u32,
        pct_h: u32,
        min_w: u32,
        min_h: u32,
    ) -> Self {
        let width = (p.ancho * pct_w / 100)
            .max(min_w)
            .min(p.ancho.saturating_sub(16 + tope_izq()));
        let height = (p.alto * pct_h / 100)
            .max(min_h)
            .min(p.alto.saturating_sub(16));
        Self {
            // Centrada en el hueco que queda A LA DERECHA del panel, no en la
            // pantalla: centrarla en la pantalla la deja corrida hacia el panel,
            // y con el al lado parece descolocada. (Era el mismo cuidado con la
            // barra de arriba, en vertical, cuando la habia.)
            x: tope_izq() + p.ancho.saturating_sub(tope_izq() + width) / 2,
            y: p.alto.saturating_sub(height) / 2,
            width,
            height,
            min_w,
            min_h,
            drag: None,
            resizing: false,
            saved: None,
            fs: None,
            minimized: false,
            hover: None,
            closable: true,
            foco: false,
            sin_marco: false,
            asoma: 0,
        }
    }

    /// Una ventana **del medida de su contenido**, mas el cromo.
    ///
    /// === Por que esta si va en pixeles, cuando `new` no ===
    ///
    /// Porque aqui el medida no lo elige el escritorio: **lo eligio la app**. Una
    /// superficie de 640x400 mide eso, y darle un 40 % de la pantalla la
    /// estiraria o la dejaria con un borde muerto alrededor. El argumento contra
    /// los pixeles --que `640x330` solo es correcto en la pantalla del que lo
    /// escribio-- vale para una ventana del sistema y no para una que envuelve
    /// una imagen de medida conocida.
    ///
    /// Se recorta contra el panel: una app puede pedir una superficie mas grande
    /// que la pantalla, y una ventana que no cabe no se puede ni agarrar.
    pub(crate) fn for_content(p: &bmo::Pantalla, width: u32, height: u32) -> Self {
        let width = (width + 2).min(p.ancho.saturating_sub(16 + tope_izq())).max(3 * BTN_SIDE + 16);
        let height = (height + TITLE_H + 1).min(p.alto.saturating_sub(16));
        Self {
            x: tope_izq() + p.ancho.saturating_sub(tope_izq() + width) / 2,
            y: p.alto.saturating_sub(height) / 2,
            width,
            height,
            // El minimo es el cromo: por debajo de eso no quedan ni los botones,
            // y una ventana sin boton de cerrar es una ventana que no se cierra.
            min_w: 3 * BTN_SIDE + 16,
            min_h: TITLE_H + 8,
            drag: None,
            resizing: false,
            saved: None,
            fs: None,
            minimized: false,
            hover: None,
            closable: true,
            foco: false,
            sin_marco: false,
            asoma: 0,
        }
    }

    /// **Una ventana sin marco** del medida de su contenido: un pixel de borde
    /// alrededor y nada mas. Ver [`Chrome::sin_marco`].
    pub(crate) fn for_content_sin_marco(p: &bmo::Pantalla, width: u32, height: u32) -> Self {
        let mut c = Self::for_content(p, width, height);
        let height = (height + 2).min(p.alto.saturating_sub(16));
        c.y = p.alto.saturating_sub(height) / 2;
        c.height = height;
        c.min_h = PASTILLA_H + 2 * PASTILLA_AIRE;
        c.sin_marco = true;
        c
    }

    /// **Lo que ocupa la barra de arriba**: `TITLE_H`, o un pixel sin marco.
    /// Es lo que separa el borde de arriba del contenido de una app.
    pub(crate) fn titulo(&self) -> u32 {
        if self.sin_marco { 1 } else { TITLE_H }
    }

    /// La pastilla de los botones de una ventana sin marco, ENTERA:
    /// `(x, y, ancho, alto)`.
    fn pastilla(&self) -> (u32, u32, u32, u32) {
        let w = 3 * BTN_SIDE + 12;
        (self.x + self.width.saturating_sub(w + PASTILLA_AIRE), self.y + PASTILLA_AIRE, w, PASTILLA_H)
    }

    /// Esta ventana no lleva aspa. Ver [`Chrome::closable`].
    pub(crate) fn sin_cerrar(mut self) -> Self {
        self.closable = false;
        self
    }

    pub(crate) fn contains(&self, px: u32, py: u32) -> bool {
        !self.minimized
            && px >= self.x
            && px < self.x + self.width
            && py >= self.y
            && py < self.y + self.height
    }

    pub(crate) fn is_maximized(&self) -> bool {
        self.saved.is_some()
    }

    /// **Esta a pantalla completa?** Sin marco, sin barra, y el panel entero.
    pub(crate) fn is_fullscreen(&self) -> bool {
        self.fs.is_some()
    }

    /// **Pantalla completa, o volver.** Devuelve la geometria VIEJA, para que
    /// quien llama sepa que trozo hay que repintar.
    ///
    /// ** Se come la barra, y eso aqui SI es correcto: una app a pantalla
    /// completa es la que el propietario esta mirando entera, y la salida no es una
    /// ficha de la barra sino la misma tecla con la que entro. Lo que no se
    /// puede es dejar al propietario sin salida, y Alt+Enter es simetrico.
    pub(crate) fn toggle_fullscreen(&mut self, p: &bmo::Pantalla) -> (u32, u32, u32, u32) {
        let old = (self.x, self.y, self.width, self.height);
        match self.fs.take() {
            Some((x, y, a, l)) => {
                self.x = x;
                self.y = y;
                self.width = a;
                self.height = l;
            }
            None => {
                self.fs = Some(old);
                self.x = 0;
                self.y = 0;
                self.width = p.ancho;
                self.height = p.alto;
            }
        }
        old
    }

    // -- Los botones -----------------------------------------------------

    /// La `x` donde empieza el boton `i` contando desde la derecha.
    fn boton_x(&self, i: u32) -> u32 {
        if self.sin_marco {
            return self.pastilla().0 + 6 + i * BTN_SIDE;
        }
        self.x + self.width - (3 - i) * BTN_SIDE - 6
    }

    /// Que boton hay bajo el puntero, si hay alguno.
    pub(crate) fn button_at(&self, px: u32, py: u32) -> Option<Button> {
        if self.minimized || self.is_fullscreen() {
            return None;
        }
        // Sin marco: solo cuando ya asomaron, y dentro de su pastilla. Un boton
        // escondido que se pulsa es un aspa invisible que funciona.
        if self.sin_marco {
            let (_, by, _, bh) = self.pastilla();
            if self.asoma < ASOMA / 2 || py < by || py >= by + bh {
                return None;
            }
        } else if py < self.y + 2 || py >= self.y + TITLE_H {
            return None;
        }
        for (i, b) in [Button::Minimize, Button::Maximize, Button::Close].into_iter().enumerate() {
            // Un boton que no se pinta tampoco se pulsa. Las dos mitades tienen
            // que mirar la misma bandera o quedaria un aspa invisible que
            // funciona -- que es peor que un aspa visible.
            if b == Button::Close && !self.closable {
                continue;
            }
            let bx = self.boton_x(i as u32);
            if px >= bx && px < bx + BTN_SIDE {
                return Some(b);
            }
        }
        None
    }

    /// Cae en la barra de titulo, o sea en el asa de arrastrar?
    ///
    /// **Los botones NO cuentan como asa**: si contaran, cada clic en cerrar
    /// empezaria ademas un arrastre, y soltar en otro sitio moveria la ventana
    /// justo cuando querias cerrarla.
    pub(crate) fn on_the_grip(&self, px: u32, py: u32) -> bool {
        // A pantalla completa no hay barra de titulo que agarrar.
        if self.is_fullscreen() {
            return false;
        }
        if self.sin_marco {
            // La franja fina de arriba siempre; con los botones fuera, tambien
            // la raya de agarrar del centro.
            let centro = self.x + self.width / 2;
            let raya = self.asoma > 0 && py < self.y + 2 * ASA + 4 && px + 48 > centro && px < centro + 48;
            return self.contains(px, py) && (py < self.y + ASA || raya) && self.button_at(px, py).is_none();
        }
        self.contains(px, py) && py < self.y + TITLE_H && self.button_at(px, py).is_none()
    }

    /// Cae en la esquina de estirar, la de abajo a la derecha?
    pub(crate) fn on_the_corner(&self, px: u32, py: u32) -> bool {
        // Ni esquina que estirar: el medida lo pone el panel.
        if self.is_fullscreen() {
            return false;
        }
        !self.minimized
            && !self.is_maximized()
            && px + GRIP_CORNER >= self.x + self.width
            && px < self.x + self.width
            && py + GRIP_CORNER >= self.y + self.height
            && py < self.y + self.height
    }

    // -- Mover y estirar -------------------------------------------------

    /// Empieza a arrastrar o a estirar. `true` si agarro algo.
    ///
    /// La esquina se mira ANTES que el asa: si se solaparan --una ventana en su
    /// medida minimo--, gana estirar, porque es la zona mas chica y la que no
    /// se puede acertar de otra forma.
    pub(crate) fn grab(&mut self, px: u32, py: u32) -> bool {
        if self.on_the_corner(px, py) {
            self.resizing = true;
            return true;
        }
        if !self.on_the_grip(px, py) || self.is_maximized() {
            return false;
        }
        self.drag = Some((px - self.x, py - self.y));
        true
    }

    pub(crate) fn release(&mut self) {
        self.drag = None;
        self.resizing = false;
    }

    pub(crate) fn grabbed(&self) -> bool {
        self.drag.is_some() || self.resizing
    }

    /// Lleva o estira la ventana hasta el puntero. `true` si cambio algo.
    ///
    /// Las dos cosas van juntas porque **quien llama no tiene por que saber
    /// cual de las dos esta pasando**: agarro, mueve el raton, y el marco sabe
    /// lo que habia agarrado.
    pub(crate) fn follow_pointer(&mut self, p: &bmo::Pantalla, px: u32, py: u32) -> bool {
        if let Some((ax, ay)) = self.drag {
            let nx = px
                .saturating_sub(ax)
                .min(p.ancho.saturating_sub(self.width))
                .max(tope_izq());
            // Sin barra de arriba, el techo es el borde: el asa nunca sale de
            // la pantalla, y eso es lo que la deja volver a coger.
            let ny = py
                .saturating_sub(ay)
                .min(p.alto.saturating_sub(self.height));
            if nx == self.x && ny == self.y {
                return false;
            }
            self.x = nx;
            self.y = ny;
            return true;
        }
        if self.resizing {
            let na = (px.saturating_sub(self.x) + 1)
                .max(self.min_w)
                .min(p.ancho.saturating_sub(self.x));
            let nl = (py.saturating_sub(self.y) + 1)
                .max(self.min_h)
                .min(p.alto.saturating_sub(self.y));
            if na == self.width && nl == self.height {
                return false;
            }
            self.width = na;
            self.height = nl;
            return true;
        }
        false
    }

    // -- Mover y encajar SIN raton ---------------------------------------
    //
    // ** El raton no es la unica mano.** Todo lo de arriba --agarrar, seguir al
    // puntero, la esquina-- exige un puntero, y hay dos momentos en los que no
    // lo hay: cuando la ventana se ha quedado con el asa fuera de la pantalla y
    // cuando el propietario esta escribiendo y no quiere soltar el teclado.
    //
    // Va en el marco y no en el compositor por la misma ley que el resto del
    // modulo: esto son rectangulos y topes. Que tecla lo dispara es politica, y
    // la politica vive arriba.

    /// Mueve la ventana un paso en un rumbo. `true` si de verdad se movio.
    ///
    /// Los topes son los MISMOS que los del arrastre --nunca por encima de la
    /// barra, nunca fuera del panel--, y eso no es economia de codigo sino la
    /// unica forma de que las dos manos dejen la ventana en sitios alcanzables
    /// por la otra. Una ventana que el teclado puede meter donde el raton no
    /// llega es una ventana perdida.
    ///
    /// Una MAXIMIZADA no se mueve, y sale gratis: ocupa el panel entero, asi que
    /// los topes la dejan donde estaba y esto devuelve `false` solo.
    pub(crate) fn push(&mut self, p: &bmo::Pantalla, heading: Heading) -> bool {
        if self.minimized {
            return false;
        }
        let (nx, ny) = match heading {
            Heading::Left => (self.x.saturating_sub(KEY_STEP).max(tope_izq()), self.y),
            Heading::Right => (
                (self.x + KEY_STEP).min(p.ancho.saturating_sub(self.width)),
                self.y,
            ),
            Heading::Up => (
                self.x,
                self.y.saturating_sub(KEY_STEP),
            ),
            Heading::Down => (
                self.x,
                (self.y + KEY_STEP).min(p.alto.saturating_sub(self.height)),
            ),
        };
        if nx == self.x && ny == self.y {
            return false;
        }
        self.x = nx;
        self.y = ny;
        true
    }

    /// Encaja la ventana contra un borde: media pantalla a un lado, el panel
    /// entero arriba, y abajo **deshace** el maximizado.
    ///
    /// === Por que `Down` no minimiza ===
    ///
    /// Porque seria una trampa sin salida. Hoy la barra del sistema solo tiene
    /// ficha para Ejecutar y para Datos --`chip_at(.., 2)`--, asi que una
    /// CABINA minimizada no tiene por donde volver: seguiria abierta, sin
    /// pintarse, y sin ningun control que la traiga. Un atajo que puede dejar
    /// una ventana inalcanzable no se da hasta que exista el camino de vuelta.
    ///
    /// Encajar a un lado **deja de ser estar maximizada**: se olvida la
    /// geometria guardada, porque el sitio al que hay que poder volver es este y
    /// no el de hace tres gestos.
    pub(crate) fn snap(&mut self, p: &bmo::Pantalla, heading: Heading) -> bool {
        if self.minimized {
            return false;
        }
        // ** Con HUECOS (HUD 2): las dos mitades se reparten el area util
        // dejando uno entre ellas, y el maximizado no toca ni la barra ni el
        // borde de la pantalla.
        let (ax, ay, aw, ah) = area_util(p);
        let avg = (aw.saturating_sub(HUECO) / 2).max(self.min_w).min(aw);
        let dest = match heading {
            Heading::Left => (ax, ay, avg, ah),
            Heading::Right => (ax + aw - avg, ay, avg, ah),
            Heading::Up => (ax, ay, aw, ah),
            // `take` aunque no se vaya a usar: si no estaba maximizada no hay
            // nada que quitar, y si lo estaba deja de estarlo aqui mismo.
            Heading::Down => match self.saved.take() {
                Some(g) => g,
                None => return false,
            },
        };
        let old = (self.x, self.y, self.width, self.height);
        if dest == old {
            return false;
        }
        match heading {
            // Maximizar por teclado guarda el sitio igual que el boton: son el
            // mismo gesto por dos caminos, y tienen que deshacerse igual.
            Heading::Up => self.saved = Some(old),
            Heading::Left | Heading::Right => self.saved = None,
            Heading::Down => {}
        }
        let (nx, ny, na, nl) = dest;
        self.x = nx;
        self.y = ny;
        self.width = na;
        self.height = nl;
        true
    }

    /// Maximizar, o volver al medida de antes. Devuelve la geometria VIEJA para
    /// que quien llama sepa que trozo de pantalla tiene que repintar.
    ///
    /// Maximizada NO es "pantalla completa": deja la barra del sistema a la
    /// vista. Una ventana que tapa la barra esconde las fichas de las demas, y
    /// entonces no hay forma de volver a ellas sin adivinar un atajo.
    pub(crate) fn toggle_maximized(&mut self, p: &bmo::Pantalla) -> (u32, u32, u32, u32) {
        let old = (self.x, self.y, self.width, self.height);
        match self.saved.take() {
            Some((x, y, a, l)) => {
                self.x = x;
                self.y = y;
                self.width = a;
                self.height = l;
            }
            None => {
                self.saved = Some(old);
                let (ax, ay, aw, ah) = area_util(p);
                self.x = ax;
                self.y = ay;
                self.width = aw;
                self.height = ah;
            }
        }
        old
    }

    /// **Ponerla en un sitio EXACTO** (el mosaico, HUD 4). Por debajo de su
    /// minimo no se encoge: una ventana inservible es peor que una que asoma.
    /// Deja de estar maximizada y suelta cualquier agarre a medias.
    pub(crate) fn colocar(&mut self, x: u32, y: u32, w: u32, h: u32) {
        self.x = x;
        self.y = y;
        self.width = w.max(self.min_w);
        self.height = h.max(self.min_h);
        self.saved = None;
        self.drag = None;
        self.resizing = false;
    }

    /// Recoloca la ventana si se ha quedado fuera del panel.
    ///
    /// Hace falta al restaurar una minimizada y al cambiar de resolucion: una
    /// geometria guardada puede haber dejado de ser valida mientras no se veia.
    pub(crate) fn fit(&mut self, p: &bmo::Pantalla) {
        let libre = p.ancho.saturating_sub(tope_izq());
        self.width = self.width.min(libre).max(self.min_w.min(libre));
        self.height = self
            .height
            .min(p.alto)
            .max(self.min_h.min(p.alto));
        self.x = self.x.min(p.ancho.saturating_sub(self.width)).max(tope_izq());
        self.y = self.y.min(p.alto.saturating_sub(self.height));
    }

    // -- Pintar ----------------------------------------------------------

    /// El cromo entero: sombra, borde, cuerpo, barra de titulo y los tres
    /// botones. Deja el interior listo para que el contenido se pinte encima.
    ///
    /// Los colores los trae quien llama porque **cada ventana tiene el suyo** y
    /// eso es informacion, no decoracion: el verde dice ESTRATOS y el azul dice
    /// el kernel antes de que nadie lea un titulo.
    pub(crate) fn paint_chrome(
        &self,
        p: &bmo::Pantalla,
        edge: u32,
        cuerpo: u32,
        title_bg: u32,
        acento: u32,
    ) {
        // ** A PANTALLA COMPLETA NO HAY CROMO: ni sombra, ni borde, ni barra
        // de titulo, ni botones. Es la definicion del estado, no un ahorro.
        if self.is_fullscreen() {
            return;
        }
        shadow(p, self.x, self.y, self.width, self.height);
        if self.sin_marco {
            // Sin marco: el borde fino y nada mas. Su luz, la que corre, la
            // pinta `paint_vivo` en cada fotograma que anima.
            self.paint_borde(p, if self.foco { mezcla_c(super::acento(), 0, 120) } else { mezcla_c(edge, 0, 60) });
            return;
        }
        // ** EL BORDE DE FOCO: el de la ventana a la que van las teclas es del
        // acento; los demas, del color de su ventana (que dice CUAL es).
        let edge = if self.foco { super::acento() } else { edge };
        // ** LA VENTANA, REDONDA DE VERDAD (25-09): radio 10, las cuatro
        // esquinas suavizadas contra el escritorio, el borde de 1 px CONSTANTE
        // y la barra de titulo con la curva de DENTRO. Antes: la curva corrida
        // una fila, el borde que engordaba en las esquinas y la barra que se
        // comia un pixel del borde. Ver `borde`.
        let alto = p.alto;
        super::borde::ventana(
            p,
            (self.x, self.y, self.width, self.height),
            edge,
            title_bg,
            TITLE_H - 1,
            cuerpo,
            &|px, py| super::background_at(px, py, alto),
        );
        self.paint_hacker(p, edge, title_bg, acento);

        self.paint_buttons(p, title_bg);
        self.paint_corner_grip(p, edge);
    }

    /// ** EL ESTILO HACKER (2026-09-25, pedido: *"todo hyprland estilo
    /// cyberpunk pero modo estilo hacker futurista"*). Sobre el marco de
    /// siempre, sin mover ni un pixel de su geometria:
    ///
    /// ```text
    ///    scanlines     una fila de cada dos, un pelo mas oscura, en la barra
    ///    la linea      bajo la barra, del acento a nada, con un "cursor" claro
    ///    esquinas HUD  el borde, mas grueso y mas claro junto a las curvas
    ///    segmentos     tres barritas inclinadas (dos llenas) antes de los botones
    /// ```
    ///
    /// Todo DENTRO del rectangulo de la ventana: lo de fuera (la sombra) ya
    /// lo repinta el escritorio al moverla, y un adorno fuera dejaria rastro.
    fn paint_hacker(&self, p: &bmo::Pantalla, edge: u32, title_bg: u32, acento: u32) {
        use super::globo::mezcla;
        let (x, y, w) = (self.x, self.y, self.width);
        // Scanlines: solo donde la barra ya es recta (bajo la curva).
        let oscura = mezcla(title_bg, 0, 46);
        let mut fila = super::borde::R_VENTANA + 2;
        while fila < TITLE_H - 1 {
            p.rect(x + 1, y + fila, w - 2, 1, oscura);
            fila += 2;
        }
        // La linea de acento, en degradado hacia el fondo de la barra, y su
        // cursor: un tramo claro de 24 px despues de la curva.
        let largo = w - 2;
        let tramos = 24u32;
        for t in 0..tramos {
            let (a, b) = (largo * t / tramos, largo * (t + 1) / tramos);
            if b > a {
                p.rect(x + 1 + a, y + TITLE_H - 1, b - a, 1, mezcla(acento, title_bg, t * 220 / tramos));
            }
        }
        let r = super::borde::R_VENTANA;
        p.rect(x + r + 4, y + TITLE_H - 2, 24.min(largo), 2, mezcla(acento, 0x00FF_FFFF, 110));
        // Esquinas HUD: 14 px de borde mas claro, justo donde acaba cada
        // curva y SOBRE la raya del borde (1 px, no 2: el de 2 asomaba por
        // dentro y no cuadraba con el borde de al lado, 25-09).
        let claro = mezcla(edge, 0x00FF_FFFF, 90);
        let brazo = 14u32.min(w / 4).min(self.height / 4);
        let h = self.height;
        for (bx, by) in [(x + r, y), (x + w - r - brazo, y), (x + r, y + h - 1), (x + w - r - brazo, y + h - 1)] {
            p.rect(bx, by, brazo, 1, claro);
        }
        for (bx, by) in [(x, y + r), (x + w - 1, y + r), (x, y + h - r - brazo), (x + w - 1, y + h - r - brazo)] {
            p.rect(bx, by, 1, brazo, claro);
        }
        // Los segmentos, antes del primer boton: tres barritas inclinadas.
        let primero = self.boton_x(0);
        if primero > x + 120 {
            let sx = primero - 34;
            let sy = y + TITLE_H / 2 - 3;
            for k in 0..3u32 {
                let color = if k < 2 { acento } else { mezcla(acento, title_bg, 170) };
                for r in 0..6u32 {
                    p.rect(sx + k * 10 + (5 - r), sy + r, 5, 1, color);
                }
            }
        }
    }

    /// Los tres, dibujados con rectangulos porque la fuente no trae sus glifos
    /// y meterlos en la fuente por tres iconos seria tocar el generador para
    /// algo que se dibuja con cuatro `rect`.
    pub(crate) fn paint_buttons(&self, p: &bmo::Pantalla, fondo: u32) {
        if self.sin_marco {
            self.paint_pastilla(p);
            return;
        }
        for (i, b) in [Button::Minimize, Button::Maximize, Button::Close].into_iter().enumerate() {
            if b == Button::Close && !self.closable {
                continue;
            }
            let bx = self.boton_x(i as u32);
            let by = self.y + 2;
            let height = TITLE_H - 3;
            Self::paint_boton(p, b, bx, by, height, fondo, self.hover == Some(b), self.is_maximized());
        }
    }

    /// Un boton, en su caja: el fondo (realzado si hace falta) y su icono.
    #[allow(clippy::too_many_arguments)]
    fn paint_boton(p: &bmo::Pantalla, b: Button, bx: u32, by: u32, height: u32, fondo: u32, encima: bool, maximizada: bool) {
        {
            let realce = if encima {
                if b == Button::Close { CLOSE_HOVER } else { BTN_HOVER }
            } else {
                fondo
            };
            p.rect(bx, by, BTN_SIDE, height, realce);

            let cx = bx + BTN_SIDE / 2;
            let cy = by + height / 2;
            let ink = if encima && b == Button::Close { 0x00FF_FFFF } else { INK };
            match b {
                // Una raya. Es el icono universal y no necesita mas.
                Button::Minimize => p.rect(cx - 5, cy, 10, 1, ink),
                // Un cuadrado hueco; DOS solapados cuando ya esta maximizada,
                // que es como se dice "restaurar" en todas partes.
                Button::Maximize => {
                    if maximizada {
                        chrome_gap(p, cx - 5, cy - 3, 8, 8, ink);
                        chrome_gap(p, cx - 2, cy - 6, 8, 8, ink);
                    } else {
                        chrome_gap(p, cx - 4, cy - 4, 9, 9, ink);
                    }
                }
                // El aspa, pixel a pixel: no hay primitiva de linea diagonal y
                // dieciseis puntos no piden una.
                Button::Close => {
                    for k in 0..9u32 {
                        p.punto(cx - 4 + k, cy - 4 + k, ink);
                        p.punto(cx - 4 + k, cy + 4 - k, ink);
                    }
                }
            }
        }
    }

    /// El borde de un pixel, todo alrededor.
    fn paint_borde(&self, p: &bmo::Pantalla, color: u32) {
        let (x, y, w, h) = (self.x, self.y, self.width, self.height);
        p.rect(x, y, w, 1, color);
        p.rect(x, y + h - 1, w, 1, color);
        p.rect(x, y, 1, h, color);
        p.rect(x + w - 1, y, 1, h, color);
    }

    /// El punto `d` del borde, recorrido en el sentido del reloj desde la
    /// esquina de arriba a la izquierda.
    fn en_el_borde(&self, d: u32) -> (u32, u32) {
        let (x, y, w, h) = (self.x, self.y, self.width.max(2), self.height.max(2));
        let d = d % (2 * (w + h) - 4);
        if d < w {
            (x + d, y)
        } else if d < w + h - 1 {
            (x + w - 1, y + d - w + 1)
        } else if d < 2 * w + h - 2 {
            (x + w - 1 - (d - w - h + 2), y + h - 1)
        } else {
            (x, y + h - 1 - (d - 2 * w - h + 3).min(h - 1))
        }
    }

    /// ** EL BORDE VIVO (01-10): con el puntero encima, dos luces corren por el
    /// borde en sentidos opuestos, del acento a nada; sin el, el borde quieto.
    /// `fase` en milisegundos. Solo toca el pixel del borde, que la app nunca
    /// pinta, y asi no hay que volver a pegar nada.
    pub(crate) fn paint_vivo(&self, p: &bmo::Pantalla, edge: u32, fase: u64, vivo: bool) {
        let base = if self.foco { mezcla_c(super::acento(), 0, 120) } else { mezcla_c(edge, 0, 60) };
        self.paint_borde(p, base);
        if !vivo {
            return;
        }
        let total = 2 * (self.width + self.height);
        let cabeza = fase as u32 % total;
        let acento = super::acento();
        const COLA: u32 = 160;
        for k in 0..COLA {
            let luz = mezcla_c(acento, 0x00FF_FFFF, 180 * k / COLA);
            let color = mezcla_c(base, luz, 255 * k / COLA);
            let (ax, ay) = self.en_el_borde(cabeza + total - COLA + k);
            p.punto_ya_marcado(ax, ay, color);
            let (bx, by) = self.en_el_borde(cabeza / 2 * 3 + total * 2 - k);
            p.punto_ya_marcado(bx, by, mezcla_c(base, MAGENTA, 255 * (COLA - k) / COLA / 2));
        }
    }

    /// ** LA PASTILLA DE LOS BOTONES (sin marco): asoma desde la derecha,
    /// abriendose a lo ancho segun [`Chrome::asoma`], y encima del contenido
    /// de la app. Con [`ASOMA`] entera lleva los tres botones; escondida, no
    /// se pinta (quien llama vuelve a pegar la app). Y la raya de agarrar,
    /// arriba en el centro.
    pub(crate) fn paint_pastilla(&self, p: &bmo::Pantalla) {
        if self.asoma == 0 || self.minimized || self.is_fullscreen() {
            return;
        }
        let (px, py, pw, ph) = self.pastilla();
        let a = self.asoma as u32;
        let ancho = (pw * a / ASOMA as u32).max(4);
        let x0 = px + pw - ancho;
        let fondo = 0x000D_1018;
        let acento = super::acento();
        p.rect(x0, py, ancho, ph, fondo);
        p.rect(x0, py, ancho, 1, mezcla_c(fondo, acento, 160));
        p.rect(x0, py + ph - 1, ancho, 1, mezcla_c(fondo, acento, 90));
        p.rect(x0, py, 1, ph, mezcla_c(fondo, acento, 160));
        p.rect(px + pw - 1, py, 1, ph, mezcla_c(fondo, acento, 90));
        // La raya de agarrar, que crece con la pastilla.
        let raya = 64 * a / ASOMA as u32;
        let cx = self.x + self.width / 2;
        p.rect(cx - raya / 2, self.y + 5, raya.max(1), 3, mezcla_c(0x0020_2430, acento, 200));
        if self.asoma < ASOMA {
            return;
        }
        for (i, b) in [Button::Minimize, Button::Maximize, Button::Close].into_iter().enumerate() {
            if b == Button::Close && !self.closable {
                continue;
            }
            Self::paint_boton(p, b, self.boton_x(i as u32), py + 2, ph - 4, fondo, self.hover == Some(b), self.is_maximized());
        }
    }

    /// Las tres rayitas en diagonal de la esquina. Un agarre invisible no
    /// existe: nadie prueba a estirar una ventana que no parece estirable.
    fn paint_corner_grip(&self, p: &bmo::Pantalla, color: u32) {
        if self.is_maximized() || self.sin_marco {
            return;
        }
        for k in 0..3u32 {
            let d = 4 + k * 4;
            p.rect(self.x + self.width - 4 - d, self.y + self.height - 6 - k * 4, d, 2, color);
        }
    }
}

/// Un rectangulo de un pixel de grosor. Cuatro `rect` y ya.
fn chrome_gap(p: &bmo::Pantalla, x: u32, y: u32, w: u32, h: u32, color: u32) {
    p.rect(x, y, w, 1, color);
    p.rect(x, y + h - 1, w, 1, color);
    p.rect(x, y, 1, h, color);
    p.rect(x + w - 1, y, 1, h, color);
}

/// `a` hacia `b`, `t` de 255. La mezcla de siempre, con el orden de `globo`.
fn mezcla_c(a: u32, b: u32, t: u32) -> u32 {
    super::globo::mezcla(a, b, t.min(256))
}
