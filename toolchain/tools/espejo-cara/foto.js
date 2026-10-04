// FOTO DE LA MAQUETA -- para el ESPEJO de cara. Corre en TU PC (no en
// BMO-X): el navegador es la regla con que se mide, como GnuCOBOL lo es en
// el ESPEJO de COBOL.
//
//   node foto.js <maqueta.html> <selector> <salida.png> [--clic <selector>]... [--medidas]
//   node foto.js <fichero.maqueta> maqueta <salida.png> --maqueta
//
// Con --maqueta las piezas (`<usa src="...">`) se componen con Shadow DOM,
// y `--estado abierta` abre ese bloque `@estado` (si no, se ve el reposo).
//
// Con --maqueta el fichero es un `.maqueta` de MAQUETA 2, y el navegador lo
// lee con lo minimo para leerlo COMO MAQUETA: sin margenes, `<maqueta>` como
// caja que mide lo que su arbol, y la letra mas parecida a la de la casa
// (Plex, como en las maquetas). Es la REGLA del ESPEJO; la otra foto la hace
// `maqueta --foto`.
//
// Hace la foto SOLO del elemento (`.ventana` en las maquetas de apps), a
// 1300 x 860, tras esperar a que la maqueta se asiente. Con --medidas
// escribe ademas la caja de cada elemento (x, y, ancho, alto, letra), que es
// de donde salen las medidas de `pintar.rs`.
//
// Pide `playwright` (npm i playwright) y su navegador.
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');

// Lo minimo para leer un `.maqueta` como maqueta, DENTRO de una pieza (la
// letra la carga el documento: las fuentes valen tambien en la sombra).
const SOMBRA = '<style>:host{display:block} maqueta{display:inline-block;width:max-content} island{display:block} usa{display:block}' +
  ' *{font-family:"IBM Plex Sans",sans-serif;line-height:normal;box-sizing:content-box;border-style:solid;border-width:0} span{display:block}</style>';

// `--estado abierta` (P3): el navegador no conoce `@estado` y se salta el
// bloque entero -- lo que ve es el REPOSO. Para ver otro estado, su bloque se
// abre en su sitio: es lo mismo que hace MAQUETA (las reglas del estado, en
// su orden, detras de las del reposo).
function abrirEstado(texto, nombre) {
  const re = new RegExp('@estado\\s+' + nombre + '\\s*\\{');
  const m = re.exec(texto);
  if (!m) return texto;
  let i = m.index + m[0].length, nivel = 1;
  while (i < texto.length && nivel > 0) {
    if (texto[i] === '{') nivel++;
    else if (texto[i] === '}') nivel--;
    i++;
  }
  return texto.slice(0, m.index) + texto.slice(m.index + m[0].length, i - 1) + texto.slice(i);
}

function componer(texto, dir, hondo) {
  if (hondo > 8) throw new Error('piezas demasiado hondas (o un ciclo)');
  const cerrado = texto.replace(/<usa\b([^>]*?)\/>/g, '<usa$1></usa>');
  return cerrado.replace(/<usa\b([^>]*)><\/usa>/g, (todo, attrs) => {
    const src = /src="([^"]+)"/.exec(attrs);
    if (!src) return todo;
    const ruta = path.join(dir, src[1]);
    const pieza = componer(fs.readFileSync(ruta, 'utf8'), path.dirname(ruta), hondo + 1);
    // La sombra va en una `<section>` y no en el `<usa>`: el navegador solo
    // deja poner Shadow DOM en unas etiquetas (`div`, `section`...) o en las
    // propias CON GUION, y `usa` no es ninguna. `<section>` no existe en
    // MAQUETA, asi que ninguna regla de la principal la puede tocar.
    return '<usa' + attrs + '><section><template shadowrootmode="open">' + SOMBRA + pieza + '</template></section></usa>';
  });
}

(async () => {
  const [html, sel, salida, ...resto] = process.argv.slice(2);
  if (!salida) { console.error('uso: node foto.js <maqueta.html> <selector> <salida.png> [--clic sel]... [--medidas]'); process.exit(2); }
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: 1300, height: 860 } });
  // Las maquetas son un trozo de HTML: se envuelve en un documento.
  const tmp = path.join(require('os').tmpdir(), 'espejo-cara-' + process.pid + '.html');
  const comoMaqueta = resto.includes('--maqueta');
  const reset = comoMaqueta
    ? '<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=IBM+Plex+Sans:wght@400;500;600;700&display=swap">' +
      '<style>html,body{margin:0;padding:0;background:#000} maqueta{display:inline-block;width:max-content} island{display:block} usa{display:block}' +
      ' *{font-family:"IBM Plex Sans",sans-serif;line-height:normal;box-sizing:content-box;border-style:solid;border-width:0} span{display:block}</style>'
    : '';
  // ** LAS PIEZAS (`<usa src>`, 04-10): cada una entra con Shadow DOM
  // declarativo, que es el aislamiento de MAQUETA dicho en HTML -- sus reglas
  // no salen de ella y las de la principal no entran. Y `<usa .../>` se
  // reescribe a `<usa ...></usa>`: en HTML una etiqueta propia NO se cierra
  // sola, y el navegador meteria a los hermanos de detras dentro de ella.
  const ie = resto.indexOf('--estado');
  const original = fs.readFileSync(html, 'utf8');
  const fuente = comoMaqueta && ie >= 0 ? abrirEstado(original, resto[ie + 1]) : original;
  const cuerpo = comoMaqueta ? componer(fuente, path.dirname(html), 0) : fuente;
  fs.writeFileSync(tmp, '<!doctype html><html><head><meta charset="utf-8">' + reset + '</head><body>' + cuerpo + '</body></html>');
  await p.goto('file://' + tmp);
  await p.waitForTimeout(900);
  for (let i = 0; i < resto.length; i++) {
    if (resto[i] === '--clic') { await p.click(resto[++i]); await p.waitForTimeout(300); }
  }
  if (resto.includes('--medidas')) {
    const filas = await p.evaluate((sel) => {
      const v = document.querySelector(sel).getBoundingClientRect();
      const out = [];
      document.querySelectorAll(sel + ' *').forEach((e) => {
        const q = e.getBoundingClientRect();
        if (!q.width || e.closest('svg') && e.tagName !== 'svg') return;
        const cs = getComputedStyle(e);
        const nombre = e.tagName.toLowerCase() + (e.id ? '#' + e.id : '') + (e.className && typeof e.className === 'string' ? '.' + e.className.trim().replace(/\s+/g, '.') : '');
        out.push([nombre, (q.x - v.x).toFixed(1), (q.y - v.y).toFixed(1), q.width.toFixed(1), q.height.toFixed(1), cs.fontSize, cs.lineHeight, cs.fontWeight, (e.innerText || '').slice(0, 24).replace(/\n/g, '|')].join('\t'));
      });
      return out;
    }, sel);
    console.log(filas.join('\n'));
  }
  // `.first()`: con piezas, las `<maqueta>` de dentro tambien casan (el
  // localizador entra en la sombra), y la foto es de la principal.
  await p.locator(sel).first().screenshot({ path: salida });
  fs.unlinkSync(tmp);
  await b.close();
})();
