/* LA ESTRELLA GATO -- el codigo de los seis estilos, aparte (PLAN_EL_HUD, HD4).

   Lo usan `maqueta_estrella_gato.html` (la galeria) y
   `maqueta_escritorio_mision.html` (el escritorio entero). Va en su propio
   fichero por la misma razon que en BMO-X la estrella viva va en su propia
   pieza: lo que pinta la estrella no sabe donde se pinta, y quien la pone en
   el escritorio no sabe como esta hecha.

   Cada estilo pinta en un <g> centrado en 0,0 (radio ~120) y devuelve su
   animacion `t -> ()`. */
(function () {
  'use strict';
  var NS = 'http://www.w3.org/2000/svg';
  function el(tag, at, padre, texto) {
    var e = document.createElementNS(NS, tag);
    for (var k in at) { e.setAttribute(k, at[k]); }
    if (texto != null) { e.textContent = texto; }
    if (padre) { padre.appendChild(e); }
    return e;
  }

  // ** LA FORMA: una cabeza de gato con sus orejas, centrada en 0,0 y de
  // radio ~110. No es el logo trazo a trazo: es lo que se RECONOCE de el --
  // las orejas, la cara, los ojos de neon en raya, la nariz y los bigotes.
  var CABEZA = 'M -104 18 C -104 -42 -76 -80 -50 -90 L -78 -158 L -16 -104 C -6 -106 6 -106 16 -104 ' +
    'L 78 -158 L 50 -90 C 76 -80 104 -42 104 18 C 104 82 58 118 0 118 C -58 118 -104 82 -104 18 Z';
  var OJOS = [[-40, 4], [40, 4]];
  var NARIZ = [0, 34];
  // Tres bigotes por lado: x1, y1, x2, y2 del lado derecho (el izquierdo es su espejo).
  var BIGOTES = [[30, 38, 132, 20], [32, 46, 136, 48], [30, 54, 126, 78]];
  function ojos(g, color, alto) {
    var o = el('g', { 'class': 'ojos' }, g);
    OJOS.forEach(function (p) {
      el('rect', { x: p[0] - 26, y: p[1] - 14, width: 52, height: 28, rx: 10, fill: color, opacity: .16 }, o);
      el('rect', { x: p[0] - 18, y: p[1] - (alto || 4), width: 36, height: (alto || 4) * 2, rx: 3, fill: color }, o);
    });
    return o;
  }
  function bigotes(g, at) {
    var b = el('g', at, g);
    BIGOTES.forEach(function (w) { [-1, 1].forEach(function (s) { el('line', { x1: s * w[0], y1: w[1], x2: s * w[2], y2: w[3] }, b); }); });
    return b;
  }
  // Un brillo: el desenfoque debajo y la forma nitida encima.
  function brillo(defs, id, std) {
    var f = el('filter', { id: id, x: '-60%', y: '-60%', width: '220%', height: '220%' }, defs);
    el('feGaussianBlur', { stdDeviation: std, result: 'b' }, f);
    var m = el('feMerge', {}, f); el('feMergeNode', { 'in': 'b' }, m); el('feMergeNode', { 'in': 'SourceGraphic' }, m);
    return 'url(#' + id + ')';
  }
  function niebla(defs, id, std) {
    var f = el('filter', { id: id, x: '-60%', y: '-60%', width: '220%', height: '220%' }, defs);
    el('feGaussianBlur', { stdDeviation: std }, f);
    return 'url(#' + id + ')';
  }
  function degradado(defs, id, paradas, at) {
    var gr = el('radialGradient', Object.assign({ id: id }, at || {}), defs);
    paradas.forEach(function (s) { el('stop', { offset: s[0], 'stop-color': s[1], 'stop-opacity': s[2] == null ? 1 : s[2] }, gr); });
    return 'url(#' + id + ')';
  }
  // Las puntas de una estrella del Webb: seis largas y dos cortas.
  function puntas(g, x, y, largo, color) {
    var p = el('g', { transform: 'translate(' + x + ' ' + y + ')' }, g);
    for (var a = 0; a < 6; a++) {
      var c = Math.cos(a * Math.PI / 3 + Math.PI / 2), s = Math.sin(a * Math.PI / 3 + Math.PI / 2);
      el('line', { x1: 0, y1: 0, x2: c * largo, y2: s * largo, stroke: color, 'stroke-width': 1.4, 'stroke-opacity': .8, 'stroke-linecap': 'round' }, p);
    }
    el('line', { x1: -largo * .45, y1: 0, x2: largo * .45, y2: 0, stroke: color, 'stroke-width': 1, 'stroke-opacity': .6 }, p);
    el('circle', { r: largo * .14, fill: '#FFFFFF' }, p);
    el('circle', { r: largo * .32, fill: color, opacity: .3 }, p);
    return p;
  }
  // Puntos sobre el contorno de la cabeza (para la constelacion, las orbitas...).
  var medidor = el('path', { d: CABEZA });
  var probador = el('svg', { width: 0, height: 0, style: 'position:absolute' }, document.body);
  probador.appendChild(medidor);
  var LARGO = medidor.getTotalLength();
  function sobre(f) { var q = medidor.getPointAtLength((((f % 1) + 1) % 1) * LARGO); return [q.x, q.y]; }
  function dentro(x, y) { var pt = probador.createSVGPoint(); pt.x = x; pt.y = y; return medidor.isPointInFill(pt); }
  function semilla(s) { return function () { s = (s * 1103515245 + 12345) & 0x7fffffff; return s / 0x7fffffff; }; }
  // Un punto de una elipse girada: centro, radios, giro (rad) y angulo.
  function enElipse(cx, cy, rx, ry, giro, a) {
    var x = rx * Math.cos(a), y = ry * Math.sin(a);
    return [cx + x * Math.cos(giro) - y * Math.sin(giro), cy + x * Math.sin(giro) + y * Math.cos(giro)];
  }

  // == LOS SEIS ESTILOS: cada uno pinta en `g` (centrado en 0,0, escala 1)
  // y devuelve su animacion `t -> ()`. ===================================
  var ESTILOS = [
    { n: 'ECLIPSE', ref: 'Gargantua, el agujero negro de Interstellar, y los eclipses de la NASA',
      como: 'El gato NEGRO tapa a su estrella. La gravedad dobla el disco de luz por ENCIMA de las orejas y por debajo de la barbilla, como Gargantua; un lado brilla mas porque gira hacia ti. Un diamante de luz recorre su borde.',
      donde: 'cpu', hecho: 'CPU: degradados radiales y arcos de bmo-pinta; la corona gira y respira con @secuencia',
      pinta: function (g, id) {
        var d = el('defs', {}, g), luz = brillo(d, id + 'b', 5), r = semilla(3);
        var corona = el('circle', { cx: 0, cy: -10, r: 236, fill: degradado(d, id + 'c', [['50%', '#FFFFFF', 0], ['57%', '#FFE7B0', .95], ['66%', '#FF2E88', .55], ['82%', '#8C52FF', .18], ['100%', '#3DA5FF', 0]]) }, g);
        var rayos = el('g', {}, g);
        for (var i = 0; i < 28; i++) {
          var a = i / 28 * Math.PI * 2 + r() * .1, l = 150 + r() * 95;
          el('line', { x1: Math.cos(a) * 112, y1: -10 + Math.sin(a) * 112, x2: Math.cos(a) * l, y2: -10 + Math.sin(a) * l, stroke: i % 3 ? '#FFE7B0' : '#FF2E88', 'stroke-width': .8 + r() * 1.8, 'stroke-opacity': .18 + r() * .25, 'stroke-linecap': 'round' }, rayos);
        }
        var dop = el('linearGradient', { id: id + 'd', x1: '0%', x2: '100%' }, d);
        [['0%', '#FFFFFF'], ['45%', '#FFE7B0'], ['100%', '#FF7A4C']].forEach(function (s) { el('stop', { offset: s[0], 'stop-color': s[1] }, dop); });
        var atras = el('ellipse', { cx: 0, cy: 0, rx: 205, ry: 30, fill: 'none', stroke: 'url(#' + id + 'd)', 'stroke-width': 5, 'stroke-opacity': .7, filter: luz }, g);
        var lente = el('path', { d: 'M -150 -6 C -158 -236 158 -236 150 -6', fill: 'none', stroke: 'url(#' + id + 'd)', 'stroke-width': 5, filter: luz }, g);
        el('path', { d: 'M -138 8 C -142 160 142 160 138 8', fill: 'none', stroke: '#FFE7B0', 'stroke-width': 2, 'stroke-opacity': .45, filter: luz }, g);
        el('path', { d: CABEZA, fill: '#000', stroke: '#FFF3D6', 'stroke-width': 1.6, filter: luz }, g);
        el('path', { d: CABEZA, fill: '#000' }, g);
        var frente = el('path', { d: 'M -205 0 A 205 30 0 0 0 205 0', fill: 'none', stroke: 'url(#' + id + 'd)', 'stroke-width': 7, filter: luz }, g);
        bigotes(g, { stroke: '#FFE7B0', 'stroke-opacity': .22, 'stroke-width': 1 });
        var o = ojos(g, '#5EF2E6');
        var diamante = el('circle', { r: 5, fill: '#FFFFFF', filter: luz }, g);
        return function (t) {
          var k = 1 + 0.04 * Math.sin(t / 900);
          corona.setAttribute('r', 236 * k);
          rayos.setAttribute('transform', 'rotate(' + (t / 300) % 360 + ' 0 -10)');
          atras.setAttribute('stroke-opacity', .5 + .3 * Math.sin(t / 700));
          lente.setAttribute('stroke-width', 4 + 1.5 * Math.sin(t / 800));
          frente.setAttribute('stroke-width', 6 + 2 * Math.sin(t / 700));
          var q = sobre(t / 16000); diamante.setAttribute('cx', q[0]); diamante.setAttribute('cy', q[1]);
          diamante.setAttribute('r', 3 + 3 * Math.abs(Math.sin(t / 400)));
          o.setAttribute('opacity', (t % 6000) > 5850 ? .1 : 1);
        };
      } },
    { n: 'CONSTELACION', ref: 'los mapas del cielo de Hevelius y las cartas estelares',
      como: 'El gato es una constelacion con nombre: estrellas sobre su contorno que titilan, lineas que se trazan solas, bigotes de estrellas y una nariz de oro. De vez en cuando una estrella fugaz le cruza la cara.',
      donde: 'cpu', hecho: 'CPU: puntos y lineas de bmo-dibujo sobre el camino aplanado al compilar; cada estrella titila con su @secuencia',
      pinta: function (g, id) {
        var d = el('defs', {}, g), luz = brillo(d, id + 'b', 3), r = semilla(11);
        el('circle', { cx: 0, cy: -10, r: 190, fill: degradado(d, id + 'n', [['0%', '#3B2A7A', .55], ['60%', '#1A1240', .3], ['100%', '#04030A', 0]]) }, g);
        for (var c = 60; c <= 240; c += 60) { el('circle', { cx: 0, cy: 0, r: c, fill: 'none', stroke: '#2B2250', 'stroke-dasharray': '2 6' }, g); }
        for (var a = 0; a < 12; a++) { el('line', { x1: 0, y1: 0, x2: 240 * Math.cos(a * Math.PI / 6), y2: 240 * Math.sin(a * Math.PI / 6), stroke: '#161236' }, g); }
        el('ellipse', { cx: 0, cy: 0, rx: 240, ry: 70, fill: 'none', stroke: '#FF2E88', 'stroke-opacity': .25, 'stroke-dasharray': '1 5', transform: 'rotate(-14)' }, g);
        var N = 26, pts = [];
        for (var i = 0; i < N; i++) { pts.push(sobre(i / N)); }
        var linea = el('polyline', { points: pts.concat([pts[0]]).map(function (p) { return p.join(','); }).join(' '), fill: 'none', stroke: '#3DA5FF', 'stroke-width': 1.3, 'stroke-opacity': .85, filter: luz }, g);
        var largo = 0; for (var k = 0; k < N; k++) { var p1 = pts[k], p2 = pts[(k + 1) % N]; largo += Math.hypot(p2[0] - p1[0], p2[1] - p1[1]); }
        linea.setAttribute('stroke-dasharray', largo);
        var bg = bigotes(g, { stroke: '#3DA5FF', 'stroke-opacity': .45, 'stroke-dasharray': '3 4' }), estrellas = [];
        BIGOTES.forEach(function (w) { [-1, 1].forEach(function (s) { estrellas.push(el('circle', { cx: s * w[2], cy: w[3], r: 2.6, fill: '#F2F7F9', filter: luz }, g)); }); });
        pts.forEach(function (p, i) { estrellas.push(el('circle', { cx: p[0], cy: p[1], r: i % 5 === 0 ? 4.2 : 1.8 + r() * 1.4, fill: i % 7 === 3 ? '#FFD45E' : '#F2F7F9', filter: luz }, g)); });
        el('text', { x: pts[0][0] - 46, y: pts[0][1] + 4, fill: '#9A96B8', 'font-size': 9, 'letter-spacing': 1.5 }, g, 'ALFA');
        el('text', { x: pts[5][0] + 10, y: pts[5][1] - 6, fill: '#9A96B8', 'font-size': 9, 'letter-spacing': 1.5 }, g, 'BETA');
        el('path', { d: 'M -6 30 L 6 30 L 0 38 Z', fill: '#FFD45E', filter: luz }, g);
        OJOS.forEach(function (o) {
          el('circle', { cx: o[0], cy: o[1], r: 18, fill: '#5EF2E6', opacity: .15 }, g);
          el('circle', { cx: o[0] - 6, cy: o[1], r: 5, fill: '#5EF2E6', filter: luz }, g);
          el('circle', { cx: o[0] + 7, cy: o[1], r: 3, fill: '#5EF2E6', filter: luz }, g);
        });
        el('text', { x: 0, y: 176, 'text-anchor': 'middle', fill: '#9A96B8', 'font-size': 12, 'letter-spacing': 4 }, g, 'FELIS  NEONIS');
        var fugaz = el('line', { stroke: '#F2F7F9', 'stroke-width': 1.6, 'stroke-linecap': 'round', filter: luz }, g);
        return function (t) {
          linea.setAttribute('stroke-dashoffset', largo * (1 - Math.min(1, (t % 9000) / 4000)));
          estrellas.forEach(function (s, i) { s.setAttribute('opacity', .4 + .6 * Math.abs(Math.sin(t / 520 + i * 1.7))); });
          bg.setAttribute('stroke-dashoffset', -t / 60);
          var f = (t % 7000) / 900;
          if (f < 1) { var x = -200 + f * 380, y = -190 + f * 230; fugaz.setAttribute('x1', x); fugaz.setAttribute('y1', y); fugaz.setAttribute('x2', x - 48); fugaz.setAttribute('y2', y - 29); fugaz.setAttribute('opacity', 1 - f); }
          else { fugaz.setAttribute('opacity', 0); }
        };
      } },
    { n: 'NEBULOSA', ref: 'los Pilares de la Creacion del Hubble y las estrellas del Webb',
      como: 'El gato hecho de gas: nubes que respiran, cientos de particulas que giran -- cian en el corazon, magenta y violeta hacia el borde -- y dos pilares de polvo con el filo encendido. Los ojos son estrellas recien nacidas con las seis puntas del Webb.',
      donde: 'gpu', hecho: '3060: un sombreador de particulas (VERRANO); la CPU pinta UN fotograma quieto y en reposo ninguno',
      // `forma` (0..1): 0 es el gas suelto, 1 es el gato. El INICIO la sube.
      pinta: function (g, id) {
        var d = el('defs', {}, g), humo = niebla(d, id + 'h', 16), fino = niebla(d, id + 'f', 4), luz = brillo(d, id + 'b', 3), r = semilla(42);
        var cp = el('clipPath', { id: id + 'p' }, d); el('path', { d: CABEZA, transform: 'scale(1.06)' }, cp);
        var aura = el('path', { d: CABEZA, fill: '#8C52FF', opacity: .2, transform: 'scale(1.2)', filter: humo }, g);
        var nubes = el('g', { 'clip-path': 'url(#' + id + 'p)' }, g), ns = [];
        [['#FF2E88', -70, -100], ['#8C52FF', 60, -96], ['#3DA5FF', -50, 50], ['#FF2E88', 70, 60], ['#5EF2E6', 0, -10], ['#8C52FF', 0, 90], ['#3DA5FF', -80, -20], ['#FF2E88', 85, -10]].forEach(function (n, i) {
          ns.push([el('circle', { cx: n[1], cy: n[2], r: 46 + (i % 3) * 10, fill: n[0], opacity: .4, filter: humo }, nubes), n[1], n[2], i]);
        });
        // Los pilares: polvo oscuro que sube desde la barbilla, con el filo
        // que la luz de las estrellas nuevas ioniza.
        var PILARES = ['M -70 120 C -78 70 -50 36 -58 -6 C -60 -26 -40 -32 -36 -10 C -30 30 -42 76 -30 120 Z',
          'M 34 120 C 28 90 52 64 46 34 C 44 20 60 16 64 32 C 70 62 60 96 70 120 Z'];
        PILARES.forEach(function (p) {
          el('path', { d: p, fill: 'none', stroke: '#FFD45E', 'stroke-width': 3, 'stroke-opacity': .55, filter: fino }, nubes);
          el('path', { d: p, fill: '#120A1E', opacity: .78, filter: fino }, nubes);
        });
        var ps = [];
        function gas(x, y, col, rad, op, orb) {
          var a = r() * 6.28, lejos = 150 + r() * 130;
          ps.push([el('circle', { cx: x, cy: y, r: rad, fill: col, opacity: op }, g), x, y, r() * 6.28, orb, Math.cos(a) * lejos, Math.sin(a) * lejos]);
        }
        while (ps.length < 560) {
          var x = (r() - .5) * 230, y = -160 + r() * 280;
          if (!dentro(x, y)) { continue; }
          var q = Math.hypot(x, (y + 10) * .9) / 120, col = q < .35 ? (r() < .5 ? '#F2F7F9' : '#5EF2E6') : q < .7 ? (r() < .5 ? '#3DA5FF' : '#FF2E88') : (r() < .5 ? '#8C52FF' : '#FF2E88');
          gas(x, y, col, .7 + r() * 2, .3 + r() * .6, 2 + r() * 8);
        }
        for (var b = 0; b < 150; b++) {
          var s = sobre(b / 150 + r() * .004), j = (r() - .5) * 6;
          gas(s[0] + j, s[1] + j, r() < .5 ? '#FF2E88' : '#F2F7F9', .8 + r() * 1.4, .5 + r() * .5, 1.5);
        }
        var borde = el('path', { d: CABEZA, fill: 'none', stroke: '#5EF2E6', 'stroke-width': 2, 'stroke-opacity': .35, filter: humo }, g);
        var nacidas = [puntas(g, -40, 4, 30, '#5EF2E6'), puntas(g, 40, 4, 30, '#5EF2E6'), puntas(g, -64, -70, 12, '#FFD45E'), puntas(g, 58, 72, 10, '#F2F7F9'),
          puntas(g, -50, -12, 7, '#F2F7F9'), puntas(g, 76, -40, 8, '#FFD45E'), puntas(g, -20, 84, 6, '#5EF2E6')];
        return function (t, forma) {
          var f = forma == null ? 1 : forma, u = 1 - f, e = f * f * (3 - 2 * f);
          ps.forEach(function (p) {
            var a = p[3] + t / (1500 + p[4] * 120);
            p[0].setAttribute('cx', p[1] + Math.cos(a) * p[4] + p[5] * (1 - e));
            p[0].setAttribute('cy', p[2] + Math.sin(a * 1.3) * p[4] + p[6] * (1 - e));
          });
          ns.forEach(function (n) { n[0].setAttribute('cx', n[1] + Math.cos(t / 2600 + n[3]) * 14); n[0].setAttribute('cy', n[2] + Math.sin(t / 3100 + n[3]) * 10); });
          nubes.setAttribute('opacity', e);
          aura.setAttribute('opacity', .2 * e + .1 * u);
          borde.setAttribute('stroke-opacity', (.25 + .15 * Math.sin(t / 900)) * e);
          nacidas.forEach(function (n, i) { n.setAttribute('opacity', (.65 + .35 * Math.sin(t / 700 + i * 2)) * (i < 2 ? Math.max(0, e * 2 - 1) : e)); });
        };
      } },
    { n: 'HOLOGRAMA', ref: 'el holograma de Leia y los planos de nave de la ciencia ficcion',
      como: 'El gato proyectado en tres dimensiones: una malla que GIRA (meridianos y paralelos de luz), el cono del proyector, una barra que lo escanea y el fallo de color -- magenta corrido contra cian -- cuando la transmision tiembla.',
      donde: 'cpu', hecho: 'CPU: lineas recortadas a la forma, la malla son elipses que cambian de ancho; el glitch es un @estado y el escaneo baja con @secuencia',
      pinta: function (g, id) {
        var d = el('defs', {}, g), luz = brillo(d, id + 'b', 3);
        var cp = el('clipPath', { id: id + 'h' }, d); el('path', { d: CABEZA }, cp);
        var cono = el('linearGradient', { id: id + 'k', x1: '0', y1: '1', x2: '0', y2: '0' }, d);
        [['0%', '#5EF2E6', .35], ['100%', '#5EF2E6', 0]].forEach(function (s) { el('stop', { offset: s[0], 'stop-color': s[1], 'stop-opacity': s[2] }, cono); });
        el('path', { d: 'M -120 176 L -110 -40 L 110 -40 L 120 176 Z', fill: 'url(#' + id + 'k)' }, g);
        el('ellipse', { cx: 0, cy: 178, rx: 130, ry: 20, fill: 'none', stroke: '#5EF2E6', 'stroke-width': 2, filter: luz }, g);
        var aro = el('ellipse', { cx: 0, cy: 178, rx: 150, ry: 26, fill: 'none', stroke: '#5EF2E6', 'stroke-opacity': .5, 'stroke-dasharray': '10 6 2 6' }, g);
        el('text', { x: 0, y: 222, 'text-anchor': 'middle', fill: '#5EF2E6', 'font-size': 10, 'letter-spacing': 3, opacity: .7 }, g, 'BMO-X  //  FELIS 01  //  TRANSMISION');
        function capa(color, at) {
          var c = el('g', at, g), malla = el('g', { 'clip-path': 'url(#' + id + 'h)' }, c), mer = [];
          for (var y = -160; y < 120; y += 6) { el('line', { x1: -110, y1: y, x2: 110, y2: y, stroke: color, 'stroke-opacity': .45, 'stroke-width': 1.2 }, malla); }
          for (var k = 0; k < 7; k++) { mer.push(el('ellipse', { cx: 0, cy: 10, rx: 10, ry: 112, fill: 'none', stroke: color, 'stroke-opacity': .5 }, malla)); }
          var barra = el('rect', { x: -110, y: -160, width: 220, height: 12, fill: color, opacity: .55 }, malla);
          el('path', { d: CABEZA, fill: 'none', stroke: color, 'stroke-width': 1.6, filter: luz }, c);
          bigotes(c, { stroke: color, 'stroke-opacity': .6 });
          return { g: c, mer: mer, barra: barra };
        }
        var fantasma = capa('#FF2E88', { opacity: 0 }), cuerpo = capa('#5EF2E6', {});
        ojos(cuerpo.g, '#FF2E88');
        return function (t) {
          [cuerpo, fantasma].forEach(function (c) {
            c.mer.forEach(function (m, k) { m.setAttribute('rx', Math.abs(Math.cos(t / 2200 + k * Math.PI / 7)) * 104); });
            c.barra.setAttribute('y', -160 + ((t / 12) % 300));
          });
          aro.setAttribute('stroke-dashoffset', t / 30);
          var glitch = (t % 3700) > 3500, dx = glitch ? Math.sin(t / 7) * 8 : 0;
          cuerpo.g.setAttribute('transform', glitch ? 'translate(' + (-dx / 2) + ' 0)' : '');
          fantasma.g.setAttribute('transform', 'translate(' + (dx + 3) + ' 0)');
          fantasma.g.setAttribute('opacity', glitch ? .7 : .12);
          cuerpo.g.setAttribute('opacity', glitch ? .6 : .9 + .1 * Math.sin(t / 60));
        };
      } },
    { n: 'ORBITAS', ref: 'las orbitas de Kepler y los diagramas de mision de la NASA',
      como: 'El gato dibujado por la mecanica celeste: la nariz es un sol, la cara su orbita grande, cada oreja una orbita inclinada con su satelite y su estela, los bigotes son transferencias por donde viajan sondas, y los ojos dos planetas con anillo.',
      donde: 'cpu', hecho: 'CPU: elipses y arcos de SVG (S2); cada satelite es un punto calculado sobre su orbita, la estela son sus ultimas posiciones',
      pinta: function (g, id) {
        var d = el('defs', {}, g), luz = brillo(d, id + 'b', 4);
        el('path', { d: CABEZA, fill: '#0B0920', stroke: '#2B2250', 'stroke-width': 1 }, g);
        var ORB = [[0, 14, 104, 100, 0, 1 / 2600, '#F2F7F9', 5], [-50, -118, 46, 15, -1.12, -1 / 1500, '#FF2E88', 4], [50, -118, 46, 15, 1.12, 1 / 1500, '#FF2E88', 4]];
        var sats = ORB.map(function (o) {
          el('ellipse', { cx: o[0], cy: o[1], rx: o[2], ry: o[3], fill: 'none', stroke: o === ORB[0] ? '#3DA5FF' : '#FF2E88', 'stroke-width': 1.4, 'stroke-dasharray': o === ORB[0] ? '4 5' : '', filter: luz, transform: 'rotate(' + o[4] * 180 / Math.PI + ' ' + o[0] + ' ' + o[1] + ')' }, g);
          return [el('polyline', { fill: 'none', stroke: o[6], 'stroke-width': 2, 'stroke-opacity': .5, 'stroke-linecap': 'round' }, g), el('circle', { r: o[7], fill: o[6], filter: luz }, g), o];
        });
        var bg = bigotes(g, { stroke: '#3DA5FF', 'stroke-opacity': .5, 'stroke-dasharray': '2 5' }), sondas = [];
        bg.querySelectorAll('line').forEach(function (l, i) { sondas.push([l, el('circle', { r: 2, fill: '#5EF2E6', filter: luz }, g), i]); });
        el('circle', { cx: NARIZ[0], cy: NARIZ[1], r: 16, fill: '#FFD45E', opacity: .2 }, g);
        el('circle', { cx: NARIZ[0], cy: NARIZ[1], r: 7, fill: '#FFD45E', filter: luz }, g);
        var anillos = OJOS.map(function (o, i) {
          el('circle', { cx: o[0], cy: o[1], r: 12, fill: i ? '#3DA5FF' : '#5EF2E6', filter: luz }, g);
          return el('ellipse', { cx: o[0], cy: o[1], rx: 25, ry: 6, fill: 'none', stroke: '#F2F7F9', 'stroke-width': 1.6 }, g);
        });
        el('text', { x: 108, y: -24, fill: '#9A96B8', 'font-size': 9, 'letter-spacing': 1.5 }, g, 'PERIAPSIS');
        el('text', { x: -54, y: -176, 'text-anchor': 'middle', fill: '#FF2E88', 'font-size': 9, 'letter-spacing': 1.5 }, g, 'i 64');
        return function (t) {
          sats.forEach(function (s) {
            var o = s[2], a = t * o[5], estela = [];
            for (var k = 9; k >= 0; k--) { estela.push(enElipse(o[0], o[1], o[2], o[3], o[4], a - k * .07 * Math.sign(o[5])).join(',')); }
            s[0].setAttribute('points', estela.join(' '));
            var p = enElipse(o[0], o[1], o[2], o[3], o[4], a); s[1].setAttribute('cx', p[0]); s[1].setAttribute('cy', p[1]);
          });
          sondas.forEach(function (s) {
            var f = ((t / 3000) + s[2] * .17) % 1, l = s[0];
            s[1].setAttribute('cx', +l.getAttribute('x1') + (l.getAttribute('x2') - l.getAttribute('x1')) * f);
            s[1].setAttribute('cy', +l.getAttribute('y1') + (l.getAttribute('y2') - l.getAttribute('y1')) * f);
            s[1].setAttribute('opacity', Math.sin(f * Math.PI));
          });
          anillos.forEach(function (a, i) { a.setAttribute('transform', 'rotate(' + (-18 + 10 * Math.sin(t / 1800 + i)) + ' ' + OJOS[i][0] + ' ' + OJOS[i][1] + ')'); });
        };
      } },
    { n: 'SOL DE PLASMA', ref: 'las fotos del Sol del SDO (la NASA en ultravioleta)',
      como: 'El gato es una estrella de verdad: la superficie gira y hierve en granulos, el borde se oscurece y le crecen espiculas como pelo, de cada oreja sale un bucle de plasma por el que caen gotas, y los ojos son manchas solares. Cada tanto, una llamarada; mas de tarde en tarde, una eyeccion que se aleja.',
      donde: 'gpu', hecho: '3060: la superficie es ruido en un sombreador (VERRANO); la CPU pinta UN fotograma quieto y en reposo ninguno',
      pinta: function (g, id) {
        var d = el('defs', {}, g), luz = brillo(d, id + 'b', 4), humo = niebla(d, id + 'h', 14), r = semilla(7);
        var rayos = el('g', {}, g);
        for (var i = 0; i < 22; i++) {
          var a = i / 22 * Math.PI * 2, l = 160 + r() * 80;
          el('path', { d: 'M ' + Math.cos(a - .05) * 100 + ' ' + Math.sin(a - .05) * 100 + ' L ' + Math.cos(a) * l + ' ' + Math.sin(a) * l + ' L ' + Math.cos(a + .05) * 100 + ' ' + Math.sin(a + .05) * 100 + ' Z', fill: '#FF2E88', opacity: .08 + r() * .1 }, rayos);
        }
        // La eyeccion: una cascara que sale de un lado y se deshace.
        var cme = el('path', { fill: 'none', stroke: '#FF2E88', 'stroke-width': 3, filter: luz }, g);
        el('path', { d: CABEZA, fill: '#FF2E88', opacity: .45, transform: 'scale(1.14)', filter: humo }, g);
        // Las espiculas: el pelo del gato hecho de plasma, en todo el borde.
        var espiculas = [];
        for (var k = 0; k < 110; k++) {
          var s0 = sobre(k / 110);
          if (s0[1] < -92) { continue; }   // en las orejas no: alli salen los bucles
          var s1 = sobre(k / 110 + .002), nx = s1[1] - s0[1], ny = s0[0] - s1[0], nl = Math.hypot(nx, ny) || 1;
          espiculas.push([el('line', { x1: s0[0], y1: s0[1], stroke: k % 4 ? '#FFB36B' : '#FFF3D6', 'stroke-width': 1.5, 'stroke-opacity': .85, 'stroke-linecap': 'round' }, g), s0[0], s0[1], -nx / nl, -ny / nl, r() * 6.28]);
        }
        var cp = el('clipPath', { id: id + 'p' }, d); el('path', { d: CABEZA }, cp);
        el('path', { d: CABEZA, fill: degradado(d, id + 's', [['0%', '#FFF3D6'], ['35%', '#FFD45E'], ['70%', '#FF2E88'], ['100%', '#5A1030']], { cx: '45%', cy: '40%', r: '65%' }) }, g);
        var granos = el('g', { 'clip-path': 'url(#' + id + 'p)' }, g), gs = [];
        for (var j = 0; j < 130; j++) {
          var x = (r() - .5) * 230, y = -150 + r() * 270;
          gs.push([el('circle', { cx: x, cy: y, r: 4 + r() * 9, fill: '#FFF3D6', opacity: .12 }, granos), x, y, r() * 6.28]);
        }
        ['M -74 62 q 16 -12 30 4 t 28 -2', 'M 34 84 q 14 8 26 -4'].forEach(function (f) { el('path', { d: f, fill: 'none', stroke: '#5A1030', 'stroke-width': 3, 'stroke-opacity': .5, 'stroke-linecap': 'round' }, granos); });
        el('path', { d: CABEZA, fill: degradado(d, id + 'o', [['60%', '#5A1030', 0], ['100%', '#2A0814', .75]]) }, g);
        var bucles = [];
        // Cada bucle sale del borde y vuelve a el, abombado HACIA FUERA: de la
        // punta de la oreja al costado de la cara, y uno abajo.
        [[-78, -158, -100, -50], [78, -158, 100, -50], [-62, 98, 10, 118]].forEach(function (b) {
          var p = el('path', { fill: 'none', stroke: '#FFD45E', 'stroke-width': 3.5, 'stroke-opacity': .85, 'stroke-linecap': 'round', filter: luz }, g);
          bucles.push([p, b, el('circle', { r: 2.6, fill: '#FFF3D6', filter: luz }, g)]);
        });
        // Las manchas: la sombra (umbra) en raya, como los ojos del logo, y la
        // penumbra con sus estrias alrededor.
        OJOS.forEach(function (o) {
          el('ellipse', { cx: o[0], cy: o[1], rx: 28, ry: 13, fill: '#7A1838', opacity: .7 }, g);
          for (var a = 0; a < 24; a++) {
            var c = Math.cos(a / 24 * Math.PI * 2), s = Math.sin(a / 24 * Math.PI * 2);
            el('line', { x1: o[0] + c * 19, y1: o[1] + s * 6, x2: o[0] + c * 28, y2: o[1] + s * 13, stroke: '#2A0814', 'stroke-opacity': .5 }, g);
          }
          el('rect', { x: o[0] - 18, y: o[1] - 4, width: 36, height: 8, rx: 3, fill: '#2A0814' }, g);
        });
        el('path', { d: 'M -6 30 L 6 30 L 0 37 Z', fill: '#5A1030', opacity: .8 }, g);
        var llama = el('circle', { cx: 86, cy: -40, r: 0, fill: '#FFFFFF', filter: luz }, g);
        var onda = el('circle', { cx: 86, cy: -40, r: 0, fill: 'none', stroke: '#FFF3D6', 'stroke-width': 2 }, g);
        return function (t) {
          rayos.setAttribute('transform', 'rotate(' + (-t / 400) % 360 + ')');
          // La superficie GIRA: los granulos se van a la derecha y vuelven por la izquierda.
          gs.forEach(function (q) {
            q[0].setAttribute('cx', ((q[1] + t / 90 + 115) % 230 + 230) % 230 - 115 + Math.cos(t / 1400 + q[3]) * 4);
            q[0].setAttribute('opacity', .06 + .14 * Math.abs(Math.sin(t / 800 + q[3])));
          });
          espiculas.forEach(function (s) {
            var l = 7 + 12 * Math.abs(Math.sin(t / 500 + s[5]));
            s[0].setAttribute('x2', s[1] + s[3] * l); s[0].setAttribute('y2', s[2] + s[4] * l);
          });
          bucles.forEach(function (b, i) {
            var h = 70 + 30 * Math.sin(t / 1100 + i * 2), x1 = b[1][0], y1 = b[1][1], x2 = b[1][2], y2 = b[1][3];
            var mx = (x1 + x2) / 2, my = (y1 + y2) / 2, nl = Math.hypot(mx, my + 20) || 1;
            var cx = mx + mx / nl * h, cy = my + (my + 20) / nl * h;
            b[0].setAttribute('d', 'M ' + x1 + ' ' + y1 + ' Q ' + cx + ' ' + cy + ' ' + x2 + ' ' + y2);
            var f = ((t / 1800) + i * .3) % 1, u = 1 - f;
            b[2].setAttribute('cx', u * u * x1 + 2 * u * f * cx + f * f * x2);
            b[2].setAttribute('cy', u * u * y1 + 2 * u * f * cy + f * f * y2);
          });
          var e = (t % 6500) / 900;
          llama.setAttribute('r', e < 1 ? 10 * Math.sin(e * Math.PI) : 0);
          onda.setAttribute('r', e < 1.6 ? e * 40 : 0);
          onda.setAttribute('stroke-opacity', e < 1.6 ? 1 - e / 1.6 : 0);
          var c = (t % 11000) / 4000;
          if (c < 1) {
            var R = 110 + c * 140;
            cme.setAttribute('d', 'M ' + (R * Math.cos(-.9)) + ' ' + (R * Math.sin(-.9)) + ' A ' + R + ' ' + R + ' 0 0 1 ' + (R * Math.cos(.3)) + ' ' + (R * Math.sin(.3)));
            cme.setAttribute('stroke-opacity', .8 * (1 - c)); cme.setAttribute('stroke-width', 2 + 6 * c);
          } else { cme.setAttribute('stroke-opacity', 0); }
        };
      } }
  ];


  window.EstrellaGato = { ESTILOS: ESTILOS, el: el, semilla: semilla, brillo: brillo, niebla: niebla, degradado: degradado, CABEZA: CABEZA, sobre: sobre, dentro: dentro };
})();
