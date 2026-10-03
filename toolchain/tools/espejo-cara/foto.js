// FOTO DE LA MAQUETA -- para el ESPEJO de cara. Corre en TU PC (no en
// BMO-X): el navegador es la regla con que se mide, como GnuCOBOL lo es en
// el ESPEJO de COBOL.
//
//   node foto.js <maqueta.html> <selector> <salida.png> [--clic <selector>]... [--medidas]
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

(async () => {
  const [html, sel, salida, ...resto] = process.argv.slice(2);
  if (!salida) { console.error('uso: node foto.js <maqueta.html> <selector> <salida.png> [--clic sel]... [--medidas]'); process.exit(2); }
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: 1300, height: 860 } });
  // Las maquetas son un trozo de HTML: se envuelve en un documento.
  const tmp = path.join(require('os').tmpdir(), 'espejo-cara-' + process.pid + '.html');
  fs.writeFileSync(tmp, '<!doctype html><html><head><meta charset="utf-8"></head><body>' + fs.readFileSync(html, 'utf8') + '</body></html>');
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
  await p.locator(sel).screenshot({ path: salida });
  fs.unlinkSync(tmp);
  await b.close();
})();
