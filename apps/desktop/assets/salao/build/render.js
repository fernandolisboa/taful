// Renders the Salão style's images (D19) from src/sprites.html into
// ../img/, and writes the icons as SVG files.
//
// Usage: node apps/desktop/assets/salao/build/render.js
// Needs Playwright with Chromium, and Inter installed (the dealer button).
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');

const OUT = path.join(__dirname, '..', 'img');

// Icons the app tints itself (GPUI draws an SVG in the text color).
const svg = (body) => `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">${body}</svg>\n`;
const ICONS = {
  // Suits, lock and check: the mockup's (design-directions/src/kit.js).
  suit_s: '<path d="M12 2.2S3.6 8.6 3.6 14c0 2.7 2 4.5 4.3 4.5 1.3 0 2.5-.6 3.2-1.6-.1 2-.8 3.6-2.1 4.9h6c-1.3-1.3-2-2.9-2.1-4.9.7 1 1.9 1.6 3.2 1.6 2.3 0 4.3-1.8 4.3-4.5C20.4 8.6 12 2.2 12 2.2z"/>',
  suit_h: '<path d="M12 21.3S3.4 15.6 3.4 9.5C3.4 6.2 5.9 4 8.6 4c1.6 0 2.8.8 3.4 1.9C12.6 4.8 13.8 4 15.4 4c2.7 0 5.2 2.2 5.2 5.5 0 6.1-8.6 11.8-8.6 11.8z"/>',
  suit_d: '<path d="M12 2.2c1.8 3.7 4.3 7 7.4 9.8-3.1 2.8-5.6 6.1-7.4 9.8-1.8-3.7-4.3-7-7.4-9.8C7.7 9.2 10.2 5.9 12 2.2z"/>',
  suit_c: '<circle cx="12" cy="7" r="4.3"/><circle cx="6.9" cy="13.7" r="4.3"/><circle cx="17.1" cy="13.7" r="4.3"/><path d="M12 10.5c0 5-.8 8.3-2.5 11.3h5c-1.7-3-2.5-6.3-2.5-11.3z"/><circle cx="12" cy="12.2" r="2.2"/>',
  lock: '<path d="M7 10V7.5a5 5 0 0110 0V10h1.5v10.5h-13V10H7zm2 0h6V7.5a3 3 0 00-6 0V10z" fill-rule="evenodd"/>',
  check: '<path d="M5 12.5l4.5 4.5L19 7.5" fill="none" stroke="#000" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"/>',
  // The Taful mark ("Gravata", the author's pick of 2026-11-20), in two
  // parts so each takes its own color.
  logo_wings: '<path d="M9.89 9L7.36 12L9.89 15C7.89 15.55 5.6 16.65 3.6 17.8C2.6 18.4 1.6 17.8 1.6 16.75L1.6 7.25C1.6 6.2 2.6 5.6 3.6 6.2C5.6 7.35 7.89 8.45 9.89 9ZM14.11 9C16.11 8.45 18.4 7.35 20.4 6.2C21.4 5.6 22.4 6.2 22.4 7.25L22.4 16.75C22.4 17.8 21.4 18.4 20.4 17.8C18.4 16.65 16.11 15.55 14.11 15L16.64 12Z"/>',
  logo_knot: '<path d="M12 8.2L15.2 12L12 15.8L8.8 12Z"/>',
};

// Re-encodes a PNG as WebP at `quality` (0-1) with the browser's encoder.
async function toWebp(page, png, quality) {
  const url = await page.evaluate(async ([b64, q]) => {
    const img = new Image();
    img.src = 'data:image/png;base64,' + b64;
    await img.decode();
    const c = document.createElement('canvas');
    c.width = img.naturalWidth;
    c.height = img.naturalHeight;
    c.getContext('2d').drawImage(img, 0, 0);
    return c.toDataURL('image/webp', q);
  }, [png.toString('base64'), quality]);
  return Buffer.from(url.split(',')[1], 'base64');
}

(async () => {
  fs.mkdirSync(OUT, { recursive: true });
  for (const [name, body] of Object.entries(ICONS)) {
    fs.writeFileSync(path.join(OUT, name + '.svg'), svg(body));
  }
  const browser = await chromium.launch();
  const url = 'file://' + path.join(__dirname, 'src', 'sprites.html');
  const probe = await browser.newPage();
  await probe.goto(url);
  const scales = await probe.$$eval('[data-out]', (els) => [...new Set(els.map((e) => Number(e.dataset.scale || 2)))]);
  await probe.close();
  for (const scale of scales) {
    const ctx = await browser.newContext({ viewport: { width: 1700, height: 1000 }, deviceScaleFactor: scale });
    const page = await ctx.newPage();
    await page.goto(url);
    await page.evaluate(() => document.fonts.ready);
    await page.waitForTimeout(200);
    for (const box of await page.$$('[data-out]')) {
      const [name, own] = await box.evaluate((e) => [e.dataset.out, Number(e.dataset.scale || 2)]);
      if (own !== scale) continue;
      const png = await box.screenshot({ omitBackground: true });
      // The big gradients are stored as WebP (a tenth of the PNG).
      const webp = await box.evaluate((e) => e.dataset.webp);
      const bytes = webp ? await toWebp(page, png, Number(webp)) : png;
      const file = path.join(OUT, name + (webp ? '.webp' : '.png'));
      fs.writeFileSync(file, bytes);
      console.log(String(bytes.length).padStart(8), path.basename(file));
    }
    await ctx.close();
  }
  await browser.close();
})();
