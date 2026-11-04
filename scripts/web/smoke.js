// A browser smoke test for a TAIM web build (scripts/web/build.sh counter).
//
//   node scripts/web/smoke.js target/web/counter
//
// Serves the directory on localhost, opens it in headless Chromium on the
// WebGL path (SwiftShader, so no GPU is needed) and checks that:
//   - nothing is reported to the console as an error, and no page error;
//   - the canvas drew something other than one flat colour;
//   - clicking the "+1" button changes what is drawn, twice.
// Exits non-zero with the reason on failure.
const http = require('http');
const fs = require('fs');
const path = require('path');
const { chromium } = require('playwright');

const root = path.resolve(process.argv[2] || 'target/web/counter');
const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm' };

function fail(reason) {
  console.error(`web smoke FAILED: ${reason}`);
  process.exit(1);
}

const server = http.createServer((request, response) => {
  const name = decodeURIComponent(new URL(request.url, 'http://localhost').pathname);
  const file = path.join(root, name === '/' ? 'index.html' : name);
  if (!file.startsWith(root) || !fs.existsSync(file)) {
    response.writeHead(404).end();
    return;
  }
  response.writeHead(200, { 'Content-Type': types[path.extname(file)] || 'application/octet-stream' });
  fs.createReadStream(file).pipe(response);
});

server.listen(0, '127.0.0.1', async () => {
  const url = `http://localhost:${server.address().port}/`;
  const browser = await chromium.launch({
    args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'],
  });
  try {
    const page = await browser.newPage({ viewport: { width: 640, height: 420 } });
    const errors = [];
    page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
    page.on('pageerror', (e) => errors.push(String(e)));

    await page.goto(url);
    await page.waitForSelector('canvas', { timeout: 20000 });
    await page.waitForTimeout(4000);

    const shot = () => page.locator('canvas').screenshot();
    const flat = (png) => {
      // A PNG of one colour compresses to almost nothing.
      return png.length < 2000;
    };

    const first = await shot();
    if (flat(first)) fail('the canvas is blank');

    // counter lays out "−1  Reset  +1" centred; "+1" is right of centre.
    await page.mouse.click(386, 238);
    await page.waitForTimeout(500);
    const second = await shot();
    if (second.equals(first)) fail('clicking "+1" did not change the picture');

    await page.mouse.click(386, 238);
    await page.waitForTimeout(500);
    const third = await shot();
    if (third.equals(second)) fail('a second click did not change the picture');

    if (errors.length) fail(`console errors:\n  ${errors.join('\n  ')}`);
    console.log('web smoke passed: drew, responded to two clicks, no console errors');
  } finally {
    await browser.close();
    server.close();
  }
});
