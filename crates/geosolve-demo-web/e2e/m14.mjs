// SPDX-License-Identifier: GPL-3.0-or-later

import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtemp, mkdir, readFile, readdir, rm, stat } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { basename, extname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const crate = resolve(fileURLToPath(new URL('..', import.meta.url)));
const dist = join(crate, 'dist');
const artifacts = resolve(crate, '../../target/m14-e2e-artifacts');
const downloads = join(artifacts, 'downloads');
const chromium = process.env.CHROMIUM || 'chromium';

class Cdp {
  constructor(url) {
    this.nextId = 1;
    this.pending = new Map();
    this.listeners = new Map();
    this.socket = new WebSocket(url);
  }

  async open() {
    await new Promise((resolveOpen, reject) => {
      const timeout = setTimeout(() => reject(new Error('CDP connection timed out')), 10_000);
      this.socket.addEventListener('open', () => {
        clearTimeout(timeout);
        resolveOpen();
      }, { once: true });
      this.socket.addEventListener('error', reject, { once: true });
    });
    this.socket.addEventListener('message', (event) => {
      const message = JSON.parse(event.data);
      if (message.id) {
        const pending = this.pending.get(message.id);
        if (!pending) return;
        this.pending.delete(message.id);
        clearTimeout(pending.timeout);
        if (message.error) pending.reject(new Error(JSON.stringify(message.error)));
        else pending.resolve(message.result);
        return;
      }
      for (const listener of this.listeners.get(message.method) || []) listener(message.params);
    });
    this.socket.addEventListener('close', () => {
      for (const pending of this.pending.values()) {
        clearTimeout(pending.timeout);
        pending.reject(new Error('CDP connection closed'));
      }
      this.pending.clear();
    });
    return this;
  }

  send(method, params = {}) {
    const id = this.nextId++;
    this.socket.send(JSON.stringify({ id, method, params }));
    return new Promise((resolveSend, reject) => {
      const timeout = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`CDP request timed out: ${method}`));
      }, 30_000);
      this.pending.set(id, { resolve: resolveSend, reject, timeout });
    });
  }

  on(method, listener) {
    const listeners = this.listeners.get(method) || [];
    listeners.push(listener);
    this.listeners.set(method, listeners);
  }

  close() {
    this.socket.close();
  }
}

class BrowserPage {
  constructor(cdp, viewport, touch) {
    this.cdp = cdp;
    this.viewport = viewport;
    this.touch = touch;
    this.errors = [];
  }

  async initialize(url) {
    this.cdp.on('Runtime.exceptionThrown', (event) => this.errors.push(`exception: ${event.exceptionDetails.text}`));
    this.cdp.on('Runtime.consoleAPICalled', (event) => {
      if (event.type === 'error') this.errors.push(`console: ${event.args.map((arg) => arg.value || arg.description).join(' ')}`);
    });
    this.cdp.on('Log.entryAdded', (event) => {
      if (event.entry.level === 'error') this.errors.push(`log: ${event.entry.text}`);
    });
    this.cdp.on('Network.loadingFailed', (event) => {
      if (!event.canceled) this.errors.push(`network: ${event.errorText} ${event.blockedReason || ''}`);
    });
    await Promise.all([
      this.cdp.send('Page.enable'),
      this.cdp.send('Runtime.enable'),
      this.cdp.send('Log.enable'),
      this.cdp.send('Network.enable'),
      this.resize(this.viewport.width, this.viewport.height),
    ]);
    if (this.touch) {
      await this.cdp.send('Emulation.setTouchEmulationEnabled', { enabled: true, maxTouchPoints: 1 });
    }
    await this.cdp.send('Page.navigate', { url });
    await this.waitFor(`document.querySelector('#playground-root')?.dataset.e2eReady === 'true'`, 30_000);
    await this.waitFor(`document.querySelector('#solve-badge')?.textContent === 'accepted'`, 30_000);
  }

  async evaluate(expression) {
    const response = await this.cdp.send('Runtime.evaluate', {
      expression,
      returnByValue: true,
      awaitPromise: true,
    });
    if (response.exceptionDetails) throw new Error(response.exceptionDetails.text);
    return response.result.value;
  }

  async waitFor(expression, timeout = 10_000) {
    const started = Date.now();
    while (Date.now() - started < timeout) {
      if (await this.evaluate(expression)) return;
      await new Promise((resolveWait) => setTimeout(resolveWait, 25));
    }
    throw new Error(`Timed out waiting for ${expression}`);
  }

  async resize(width, height) {
    this.viewport = { width, height };
    await this.cdp.send('Emulation.setDeviceMetricsOverride', {
      width,
      height,
      deviceScaleFactor: 1,
      mobile: this.touch,
    });
  }

  async reload() {
    const previousOrigin = await this.evaluate('performance.timeOrigin');
    await this.cdp.send('Page.reload', { ignoreCache: true });
    await this.waitFor(`performance.timeOrigin !== ${previousOrigin} && document.querySelector('#playground-root')?.dataset.e2eReady === 'true'`, 30_000);
    await this.assertAccepted();
  }

  async setSelect(id, value) {
    await this.evaluate(`(() => { const element = document.querySelector(${JSON.stringify(`#${id}`)}); element.value = ${JSON.stringify(value)}; element.dispatchEvent(new Event('change', { bubbles: true })); return element.value; })()`);
  }

  async setInput(id, value) {
    await this.evaluate(`(() => { const element = document.querySelector(${JSON.stringify(`#${id}`)}); element.value = ${JSON.stringify(String(value))}; element.dispatchEvent(new Event('input', { bubbles: true })); return element.value; })()`);
  }

  async click(selector) {
    const before = Number(await this.evaluate(`document.querySelector('#playground-root').dataset.renderSequence`));
    await this.evaluate(`document.querySelector(${JSON.stringify(selector)}).click()`);
    await this.waitFor(`Number(document.querySelector('#playground-root').dataset.renderSequence) > ${before}`);
  }

  async clickObject(label, additive = false) {
    const before = Number(await this.evaluate(`document.querySelector('#playground-root').dataset.renderSequence`));
    const found = await this.evaluate(`(() => { const button = [...document.querySelectorAll('.object-row')].find((item) => item.textContent.includes(${JSON.stringify(label)})); if (!button) return false; button.dispatchEvent(new MouseEvent('click', { bubbles: true, shiftKey: ${additive} })); return true; })()`);
    assert.equal(found, true, `missing object row ${label}`);
    await this.waitFor(`Number(document.querySelector('#playground-root').dataset.renderSequence) > ${before}`);
  }

  async deleteObject(label) {
    const before = Number(await this.evaluate(`document.querySelector('#playground-root').dataset.renderSequence`));
    const found = await this.evaluate(`(() => { const row = [...document.querySelectorAll('.object-row')].find((item) => item.textContent.includes(${JSON.stringify(label)})); const button = row?.closest('.object-entry')?.querySelector('[data-action="delete-object"]'); if (!button) return false; button.click(); return true; })()`);
    assert.equal(found, true, `missing object delete button ${label}`);
    await this.waitFor(`Number(document.querySelector('#playground-root').dataset.renderSequence) > ${before}`);
  }

  async loadExample(kind, scale = '1') {
    await this.setSelect('alpha-example', kind);
    await this.setSelect('alpha-scale', scale);
    await this.click('[data-action="load-example"]');
    await this.assertAccepted();
    if (kind !== 'medium') {
      assert.match(
        await this.evaluate(`document.querySelector('#last-attempt').textContent`),
        new RegExp(`canonical ${kind}`, 'i'),
      );
    }
  }

  async assertAccepted() {
    const result = await this.evaluate(`(() => { const root = document.querySelector('#playground-root'); return { validity: root.dataset.hardValidity, residual: Number(root.dataset.hardResidualMax), badge: document.querySelector('#solve-badge').textContent }; })()`);
    assert.equal(result.validity, 'Valid');
    assert.equal(result.badge, 'accepted');
    assert.ok(Number.isFinite(result.residual) && result.residual <= 1e-9, JSON.stringify(result));
  }

  async exportJson() {
    await this.click('[data-action="export-json"]');
    return this.evaluate(`document.querySelector('#document-json').value`);
  }

  async point(label) {
    return this.evaluate(`(() => { const point = [...document.querySelectorAll('[data-point-id]')].find((item) => item.dataset.label === ${JSON.stringify(label)}); if (!point) return null; document.querySelector('#sketch-viewport').scrollIntoView({ block: 'center', inline: 'center' }); const rect = point.getBoundingClientRect(); return { x: Number(point.dataset.modelX), y: Number(point.dataset.modelY), clientX: rect.left + rect.width / 2, clientY: rect.top + rect.height / 2, id: point.dataset.pointId }; })()`);
  }

  async modelClient(x, y) {
    return this.evaluate(`(() => { const root = document.querySelector('#playground-root'); const viewport = document.querySelector('#sketch-viewport'); viewport.scrollIntoView({ block: 'center', inline: 'center' }); const svg = viewport.getBoundingClientRect(); const sx = 500 + (${x} - Number(root.dataset.viewportCenterX)) * Number(root.dataset.pixelsPerUnit); const sy = 350 - (${y} - Number(root.dataset.viewportCenterY)) * Number(root.dataset.pixelsPerUnit); return { x: svg.left + sx * svg.width / 1000, y: svg.top + sy * svg.height / 700 }; })()`);
  }

  async pointerClick(point) {
    const before = Number(await this.evaluate(`document.querySelector('#playground-root').dataset.renderSequence`));
    if (this.touch) {
      await this.evaluate(`(() => { const target = document.elementFromPoint(${point.x}, ${point.y}); if (!target?.closest('#sketch-viewport')) throw new Error('touch point missed viewport: ' + target?.tagName + '#' + target?.id); target.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, cancelable: true, clientX: ${point.x}, clientY: ${point.y}, pointerId: 41, pointerType: 'touch', isPrimary: true, button: 0, buttons: 1 })); document.elementFromPoint(${point.x}, ${point.y}).dispatchEvent(new PointerEvent('pointerup', { bubbles: true, cancelable: true, clientX: ${point.x}, clientY: ${point.y}, pointerId: 41, pointerType: 'touch', isPrimary: true, button: 0, buttons: 0 })); return true; })()`);
    } else {
      await this.cdp.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: point.x, y: point.y });
      await this.cdp.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: point.x, y: point.y, button: 'left', clickCount: 1 });
      await this.cdp.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: point.x, y: point.y, button: 'left', clickCount: 1 });
    }
    await this.waitFor(`Number(document.querySelector('#playground-root').dataset.renderSequence) > ${before}`);
  }

  async pointerCancel(point) {
    const before = Number(await this.evaluate(`document.querySelector('#playground-root').dataset.renderSequence`));
    await this.evaluate(`(() => { const target = document.elementFromPoint(${point.x}, ${point.y}); if (!target) return false; target.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, cancelable: true, clientX: ${point.x}, clientY: ${point.y}, pointerId: 73, pointerType: 'touch', isPrimary: true, button: 0, buttons: 1 })); document.elementFromPoint(${point.x}, ${point.y}).dispatchEvent(new PointerEvent('pointercancel', { bubbles: true, cancelable: true, clientX: ${point.x}, clientY: ${point.y}, pointerId: 73, pointerType: 'touch', isPrimary: true, button: 0, buttons: 0 })); return true; })()`);
    await this.waitFor(`Number(document.querySelector('#playground-root').dataset.renderSequence) > ${before}`);
  }

  async dragPoint(label, x, y, steps = 1) {
    const start = await this.point(label);
    assert.ok(start, `missing point ${label}`);
    const target = await this.modelClient(x, y);
    const before = Number(await this.evaluate(`document.querySelector('#playground-root').dataset.renderSequence`));
    if (this.touch) {
      await this.cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x: start.clientX, y: start.clientY, id: 1, radiusX: 8, radiusY: 8 }] });
      for (let index = 1; index <= steps; index++) {
        const fraction = index / steps;
        await this.cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x: start.clientX + (target.x - start.clientX) * fraction, y: start.clientY + (target.y - start.clientY) * fraction, id: 1, radiusX: 8, radiusY: 8 }] });
      }
      await this.cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
    } else {
      await this.cdp.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: start.clientX, y: start.clientY });
      await this.cdp.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: start.clientX, y: start.clientY, button: 'left', clickCount: 1 });
      for (let index = 1; index <= steps; index++) {
        const fraction = index / steps;
        await this.cdp.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: start.clientX + (target.x - start.clientX) * fraction, y: start.clientY + (target.y - start.clientY) * fraction, button: 'left', buttons: 1 });
      }
      await this.cdp.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: target.x, y: target.y, button: 'left', clickCount: 1 });
    }
    await this.waitFor(`Number(document.querySelector('#playground-root').dataset.renderSequence) > ${before}`);
  }

  async panCanvas(dx, dy) {
    const rect = await this.evaluate(`(() => { const rect = document.querySelector('#sketch-viewport').getBoundingClientRect(); return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 }; })()`);
    const before = Number(await this.evaluate(`document.querySelector('#playground-root').dataset.renderSequence`));
    if (this.touch) {
      await this.cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x: rect.x, y: rect.y, id: 1, radiusX: 8, radiusY: 8 }] });
      await this.cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x: rect.x + dx, y: rect.y + dy, id: 1, radiusX: 8, radiusY: 8 }] });
      await this.cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
    } else {
      await this.cdp.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: rect.x, y: rect.y });
      await this.cdp.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: rect.x, y: rect.y, button: 'left', clickCount: 1 });
      await this.cdp.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: rect.x + dx, y: rect.y + dy, button: 'left', buttons: 1 });
      await this.cdp.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: rect.x + dx, y: rect.y + dy, button: 'left', clickCount: 1 });
    }
    await this.waitFor(`Number(document.querySelector('#playground-root').dataset.renderSequence) > ${before}`);
  }

  async key(key, code, modifiers = 0) {
    const before = Number(await this.evaluate(`document.querySelector('#playground-root').dataset.renderSequence`));
    await this.cdp.send('Input.dispatchKeyEvent', { type: 'rawKeyDown', key, code, modifiers });
    await this.cdp.send('Input.dispatchKeyEvent', { type: 'keyUp', key, code, modifiers });
    await this.waitFor(`Number(document.querySelector('#playground-root').dataset.renderSequence) > ${before}`);
  }

  async upload(path) {
    const document = await this.cdp.send('DOM.getDocument', { depth: 1 });
    const input = await this.cdp.send('DOM.querySelector', { nodeId: document.root.nodeId, selector: '#document-file' });
    assert.notEqual(input.nodeId, 0);
    const before = Number(await this.evaluate(`document.querySelector('#playground-root').dataset.renderSequence`));
    await this.cdp.send('DOM.setFileInputFiles', { files: [path], nodeId: input.nodeId });
    await this.waitFor(`Number(document.querySelector('#playground-root').dataset.renderSequence) > ${before}`, 30_000);
  }

  assertNoErrors() {
    assert.deepEqual(this.errors, []);
  }
}

function mime(path) {
  return ({ '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.css': 'text/css', '.json': 'application/json' })[extname(path)] || 'application/octet-stream';
}

async function startServer() {
  const server = createServer(async (request, response) => {
    try {
      const pathname = new URL(request.url, 'http://localhost').pathname;
      const relative = pathname === '/' ? 'index.html' : pathname.slice(1);
      const file = resolve(dist, relative);
      if (!file.startsWith(`${dist}/`) && file !== join(dist, 'index.html')) throw new Error('invalid path');
      let data = await readFile(file);
      if (extname(file) === '.html') {
        data = Buffer.from(
          data
            .toString()
            .replace(/\s*<script>"use strict";[\s\S]*?new Client\(url\)\.start\(\);[\s\S]*?<\/script>/, ''),
        );
      }
      response.writeHead(200, { 'content-type': mime(file), 'cache-control': 'no-store' });
      response.end(data);
    } catch {
      response.writeHead(404);
      response.end('not found');
    }
  });
  await new Promise((resolveListen) => server.listen(0, '127.0.0.1', resolveListen));
  return { server, url: `http://127.0.0.1:${server.address().port}/` };
}

async function startChromium() {
  const profile = await mkdtemp(join(tmpdir(), 'geosolve-m14-'));
  const process = spawn(chromium, [
    '--headless=new',
    '--no-sandbox',
    '--disable-dev-shm-usage',
    '--disable-background-networking',
    '--disable-component-update',
    '--disable-default-apps',
    '--disable-extensions',
    '--disable-gpu',
    '--disable-features=PaintHolding',
    '--disable-sync',
    '--metrics-recording-only',
    '--no-first-run',
    '--noerrdialogs',
    '--no-zygote',
    '--ozone-platform=headless',
    '--renderer-process-limit=1',
    '--single-process',
    '--use-gl=disabled',
    '--remote-debugging-port=0',
    `--user-data-dir=${profile}`,
    'about:blank',
  ], { stdio: ['ignore', 'ignore', 'pipe'] });
  let stderr = '';
  try {
    const browserUrl = await new Promise((resolveUrl, reject) => {
      const timeout = setTimeout(() => reject(new Error(`Chromium startup timed out\n${stderr}`)), 20_000);
      process.stderr.on('data', (chunk) => {
        stderr += chunk.toString();
        const match = stderr.match(/DevTools listening on (ws:\/\/[^\s]+)/);
        if (match) {
          clearTimeout(timeout);
          resolveUrl(match[1]);
        }
      });
      process.once('exit', (code) => reject(new Error(`Chromium exited ${code}\n${stderr}`)));
    });
    const cdp = await new Cdp(browserUrl).open();
    return { process, profile, cdp, stderr: () => stderr };
  } catch (error) {
    process.kill('SIGKILL');
    await rm(profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
    throw error;
  }
}

async function openPage(browserUrl, url, viewport, touch) {
  const endpoint = new URL(browserUrl);
  let lastError;
  for (let attempt = 0; attempt < 3; attempt++) {
    const created = await fetch(`http://${endpoint.host}/json/new?${encodeURIComponent('about:blank')}`, { method: 'PUT' }).then((response) => response.json());
    const page = new BrowserPage(await new Cdp(created.webSocketDebuggerUrl).open(), viewport, touch);
    try {
      await page.initialize(url);
      return page;
    } catch (error) {
      lastError = error;
      await page.cdp.send('Page.close').catch(() => {});
      page.cdp.close();
      await new Promise((resolveWait) => setTimeout(resolveWait, 250));
    }
  }
  throw lastError;
}

function near(actual, expected, tolerance = 1e-7) {
  assert.ok(Math.abs(actual - expected) <= tolerance, `${actual} != ${expected}`);
}

async function assertDomMatchesJson(page, json) {
  const document = JSON.parse(json);
  const rendered = await page.evaluate(`[...document.querySelectorAll('[data-point-id]')].map((point) => ({ id: point.dataset.pointId, x: Number(point.dataset.modelX), y: Number(point.dataset.modelY) }))`);
  assert.equal(rendered.length, document.points.length);
  for (const point of document.points) {
    const match = rendered.find((item) => item.id === point.id);
    assert.ok(match, `missing rendered point ${point.id}`);
    const tolerance = Math.max(Math.abs(point.position[0]), Math.abs(point.position[1]), 1) * 1e-12;
    near(match.x, point.position[0], tolerance);
    near(match.y, point.position[1], tolerance);
  }
}

async function creationSuite(page) {
  await page.click('[data-action="new"]');
  await page.click('[data-tool="line"]');
  if (page.touch) {
    await page.pointerCancel(await page.modelClient(0, 0));
    assert.equal(JSON.parse(await page.exportJson()).points.length, 0);
    assert.match(await page.evaluate(`document.querySelector('#draft-status').textContent`), /line start/i);
  }

  await page.pointerClick(await page.modelClient(-4, 1));
  await page.pointerClick(await page.modelClient(-4, 1));
  assert.equal(JSON.parse(await page.exportJson()).points.length, 0);
  assert.equal(await page.evaluate(`document.querySelector('#undo-draft').disabled`), false);
  await page.click('[data-action="undo-draft"]');
  await page.pointerClick(await page.modelClient(-3, 2));

  await page.click('[data-tool="point"]');
  await page.pointerClick(await page.modelClient(-4, 3));

  await page.click('[data-tool="polyline"]');
  for (const point of [[-2, 2], [-1, 3], [0, 2]]) await page.pointerClick(await page.modelClient(...point));
  assert.equal(await page.evaluate(`document.querySelector('[data-draft-kind="polyline"]') !== null`), true);
  await page.click('[data-action="finish-draft"]');

  await page.click('[data-tool="rectangle"]');
  await page.pointerClick(await page.modelClient(1, 2));
  await page.pointerClick(await page.modelClient(2, 3));

  await page.click('[data-tool="circle"]');
  await page.pointerClick(await page.modelClient(3, 2));
  await page.pointerClick(await page.modelClient(4, 2));

  await page.click('[data-tool="arc"]');
  await page.pointerClick(await page.modelClient(3, -1));
  await page.pointerClick(await page.modelClient(4, -1));
  await page.pointerClick(await page.modelClient(3, 0));

  await page.click('[data-tool="quadratic"]');
  for (const point of [[-4, -2], [-3, 0], [-2, -2]]) await page.pointerClick(await page.modelClient(...point));

  await page.click('[data-tool="cubic"]');
  for (const point of [[-1, -2], [0, 0], [1, -3], [2, -2]]) await page.pointerClick(await page.modelClient(...point));

  const created = JSON.parse(await page.exportJson());
  const kinds = created.curves.map((curve) => curve.definition.kind);
  assert.equal(created.points.length, 19);
  assert.equal(created.curves.length, 10);
  assert.equal(kinds.filter((kind) => kind === 'line').length, 5);
  for (const kind of ['polyline', 'circle', 'circular_arc', 'quadratic_bezier', 'cubic_bezier']) {
    assert.equal(kinds.includes(kind), true, `missing directly created ${kind}`);
  }
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.historyCursor`), '8');
  assert.equal(await page.evaluate(`document.querySelector('[data-draft-kind]') === null`), true);
  await page.assertAccepted();
}

async function stressExampleSuite(page) {
  await page.loadExample('stress-compass');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.equalityDof`), '1');
  assert.match(await page.evaluate(`document.querySelector('#object-list').textContent`), /symmetric tips/i);
  await page.dragPoint('Compass tip A', 2 * Math.sqrt(3), -2, 6);
  const compassA = await page.point('Compass tip A');
  const compassB = await page.point('Compass tip B');
  assert.ok(compassA.x > 3.4 && compassA.y < -1.9);
  near(Math.hypot(compassA.x, compassA.y), 4, 1e-8);
  near(Math.hypot(compassB.x, compassB.y), 4, 1e-8);
  const axis = [Math.sqrt(3) / 2, 0.5];
  const projection = compassA.x * axis[0] + compassA.y * axis[1];
  near(compassB.x, 2 * projection * axis[0] - compassA.x, 1e-8);
  near(compassB.y, 2 * projection * axis[1] - compassA.y, 1e-8);
  await page.clickObject('Compass opening angle 60 deg');
  await page.setSelect('dimension-mode', 'Driving');
  await page.setInput('dimension-value', Math.PI / 3);
  await page.click('[data-action="apply-dimension"]');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.equalityDof`), '0');

  await page.loadExample('stress-bridge');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.boundedDof`), '1');
  assert.match(await page.evaluate(`document.querySelector('#object-list').textContent`), /C1 endpoint tangency/);
  await page.dragPoint('Bridge left seam', 0.25, -0.5, 6);
  const left = await page.point('Bridge left seam');
  const right = await page.point('Bridge right seam');
  assert.ok(left.x > 0.2 && left.y < -0.4);
  near(left.y, -2 * left.x, 1e-8);
  near(right.x, left.x, 1e-8);
  near(right.y, left.y, 1e-8);
  await page.clickObject('Bridge equal seam handles');
  await page.click('[data-action="toggle-suppressed"]');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.boundedDof`), '0');
  await page.click('[data-action="toggle-suppressed"]');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.boundedDof`), '1');
  await page.dragPoint('Bridge left seam', -1, 2, 4);
  const projected = await page.point('Bridge left seam');
  assert.ok(Math.hypot(projected.x + 1, projected.y - 2) > 0.1);
  near(projected.y, -2 * projected.x, 1e-8);
  assert.match(await page.evaluate(`document.querySelector('#last-attempt').textContent`), /projected|committed/i);

  await page.loadExample('motion-cam');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.boundedDof`), '2');
  const rightRollerBefore = await page.point('Right roller center');
  await page.dragPoint('Left roller center', 0, 3, 12);
  const leftRoller = await page.point('Left roller center');
  const rightRollerAfter = await page.point('Right roller center');
  assert.ok(Math.abs(leftRoller.x) < 0.08 && Math.abs(leftRoller.y - 3) < 0.08, JSON.stringify(leftRoller));
  near(rightRollerAfter.x, rightRollerBefore.x, 1e-8);
  near(rightRollerAfter.y, rightRollerBefore.y, 1e-8);

  await page.loadExample('motion-orbit');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.boundedDof`), '1');
  for (const [x, y] of [
    [2, Math.sqrt(12)],
    [0, 4],
    [-2, Math.sqrt(12)],
    [-4, 0],
    [-2, -Math.sqrt(12)],
    [0, -4],
    [2, -Math.sqrt(12)],
    [4, 0],
  ]) {
    await page.dragPoint('Orbit satellite center', x, y, 10);
    const satellite = await page.point('Orbit satellite center');
    near(satellite.x, x, 0.08);
    near(satellite.y, y, 0.08);
    near(Math.hypot(satellite.x, satellite.y), 4, 1e-8);
  }
  await page.assertAccepted();
}

async function historySuite(page) {
  await page.click('[data-action="new"]');
  await page.click('[data-tool="rectangle"]');
  await page.pointerClick(await page.modelClient(0, 0));
  await page.pointerClick(await page.modelClient(4, 3));
  assert.equal(
    await page.evaluate(`document.querySelector('#playground-root').dataset.historyCursor`),
    '1',
    await page.evaluate(`JSON.stringify({ last: document.querySelector('#last-attempt').textContent, draft: document.querySelector('#draft-status').textContent, tool: document.querySelector('#sketch-viewport').dataset.tool, points: document.querySelectorAll('[data-point-id]').length })`),
  );
  await page.clickObject('edge_1');
  await page.setSelect('dimension-kind', 'Length');
  await page.setSelect('dimension-mode', 'Driving');
  await page.setInput('dimension-value', 4);
  await page.setInput('dimension-label', 'width_dimension');
  await page.click('[data-action="apply-dimension"]');
  await page.clickObject('edge_2');
  await page.setInput('dimension-value', 3);
  await page.setInput('dimension-label', 'height_dimension');
  await page.click('[data-action="apply-dimension"]');
  const initialWidth = JSON.parse(await page.exportJson()).scalars.find((item) => item.label === 'width_dimension target').value;

  await page.clickObject('width_dimension');
  await page.setInput('dimension-value', 6);
  await page.click('[data-action="apply-dimension"]');
  await page.clickObject('height_dimension');
  await page.click('[data-action="toggle-suppressed"]');

  await page.click('[data-action="zoom-fit"]');
  for (let index = 0; index < 7; index++) await page.click('[data-action="zoom-out"]');
  await page.click('[data-tool="point"]');
  await page.pointerClick(await page.modelClient(9, 9));
  const pointE = await page.point('Point 5');
  assert.ok(pointE);
  await page.click('[data-action="delete"]');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.historyCursor`), '7');
  const finalJson = await page.exportJson();

  await page.key('z', 'KeyZ', 2);
  assert.equal((await page.point('Point 5')).id, pointE.id);
  await page.key('z', 'KeyZ', 2);
  assert.equal(await page.point('Point 5'), null);
  await page.key('z', 'KeyZ', 2);
  assert.equal(JSON.parse(await page.exportJson()).dimensions.find((item) => item.label.includes('height_dimension')).suppressed, false);
  await page.key('z', 'KeyZ', 2);
  const restoredWidth = JSON.parse(await page.exportJson()).scalars.find((item) => item.label === 'width_dimension target');
  assert.equal(restoredWidth.value, initialWidth);
  for (let index = 0; index < 3; index++) await page.key('z', 'KeyZ', 2);
  assert.equal(JSON.parse(await page.exportJson()).points.length, 0);

  for (let index = 0; index < 7; index++) await page.key('y', 'KeyY', 2);
  assert.equal(await page.exportJson(), finalJson);
  assert.equal(await page.evaluate(`localStorage.getItem('geosolve.sketch-playground.accepted.v1')`), finalJson);
  return finalJson;
}

async function scaleWorkflow(page, scaleText) {
  const scale = Number(scaleText);

  await page.loadExample('a1', scaleText);
  await page.click('[data-action="zoom-fit"]');
  const visible = await page.evaluate(`(() => { const viewport = document.querySelector('#sketch-viewport').getBoundingClientRect(); return [...document.querySelectorAll('[data-point-id]')].every((point) => { const rect = point.getBoundingClientRect(); return rect.right >= viewport.left && rect.left <= viewport.right && rect.bottom >= viewport.top && rect.top <= viewport.bottom; }); })()`);
  assert.equal(visible, true, `A1 points not visible at ${scaleText}`);
  await page.clickObject('width-4');
  await page.setInput('dimension-value', 6 * scale);
  await page.click('[data-action="apply-dimension"]');
  const scaledA1 = await page.exportJson();
  const widthTarget = JSON.parse(scaledA1).scalars.find((item) => item.label.endsWith('.width'));
  assert.equal(widthTarget.value, 6 * scale);
  await assertDomMatchesJson(page, scaledA1);

  await page.loadExample('a2', scaleText);
  const initialA2 = await page.exportJson();
  await page.dragPoint('A2 C', 0, 3 * scale);
  const draggedA2 = await page.exportJson();
  assert.notEqual(draggedA2, initialA2);
  await assertDomMatchesJson(page, draggedA2);
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.equalityDof`), '1');

  await page.loadExample('a3', scaleText);
  const scaledA3 = await page.exportJson();
  assert.equal(await page.evaluate(`document.querySelectorAll('[data-contact-id]').length`), 2);
  await page.clickObject('A3 line contact');
  await page.clickObject('A3 circle contact');
  await page.setInput('contact-parameter', 1.1);
  await page.click('[data-action="apply-branch-state"]');
  assert.equal(await page.exportJson(), scaledA3);
  await page.setInput('contact-parameter', '');

  await page.loadExample('a4', scaleText);
  const initialRadius = JSON.parse(await page.exportJson()).scalars.find((item) => item.label === 'A4 free circle radius').value;
  await page.dragPoint('A4 circle center', 8 * scale, scale, 4);
  const scaledA4 = await page.exportJson();
  const scaledRadius = JSON.parse(scaledA4).scalars.find((item) => item.label === 'A4 free circle radius').value;
  assert.ok(scaledRadius > 0 && scaledRadius !== initialRadius);
  const scaledContacts = await page.evaluate(`[...document.querySelectorAll('[data-contact-id]')].map((item) => [Number(item.dataset.modelX), Number(item.dataset.modelY)])`);
  near(scaledContacts[0][0], scaledContacts[1][0], Math.max(scale, 1) * 1e-9);
  near(scaledContacts[0][1], scaledContacts[1][1], Math.max(scale, 1) * 1e-9);
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.equalityDof`), '2');
  await page.dragPoint('A4 circle center', -8 * scale, 0);
  assert.equal(await page.exportJson(), scaledA4);

  await page.loadExample('a5', scaleText);
  const stableA5Controls = {
    p2: await page.point('A5 P2'),
    p3: await page.point('A5 P3'),
  };
  const diagonal = Math.SQRT2 * scale;
  await page.dragPoint('A5 line B', diagonal, diagonal, 8);
  const draggedLineEnd = await page.point('A5 line B');
  near(Math.hypot(draggedLineEnd.x, draggedLineEnd.y), 2 * scale, Math.max(scale, 1) * 1e-8);
  assert.ok(draggedLineEnd.x > 0 && draggedLineEnd.y > 0);
  const stableA5ControlsAfter = {
    p2: await page.point('A5 P2'),
    p3: await page.point('A5 P3'),
  };
  for (const key of ['p2', 'p3']) {
    near(stableA5ControlsAfter[key].x, stableA5Controls[key].x, Math.max(scale, 1) * 1e-9);
    near(stableA5ControlsAfter[key].y, stableA5Controls[key].y, Math.max(scale, 1) * 1e-9);
  }
  assert.doesNotMatch(await page.evaluate(`document.querySelector('#last-attempt').textContent`), /NumericalFailure|opposite branch/);

  await page.loadExample('a5', scaleText);
  const initialB = await page.point('A5 line B');
  await page.dragPoint('A5 P1', scale, 0.5 * scale, 8);
  const scaledB = await page.point('A5 line B');
  assert.ok(scaledB.x !== initialB.x || scaledB.y !== initialB.y);
  const scaledA5 = await page.exportJson();
  const invalidA5 = JSON.parse(scaledA5);
  invalidA5.points.find((point) => point.label === 'A5 P1').position = [...invalidA5.points.find((point) => point.label === 'A5 P0').position];
  await page.setInput('document-json', JSON.stringify(invalidA5));
  await page.click('[data-action="import-json"]');
  assert.equal(await page.exportJson(), scaledA5);

  await page.loadExample('a1', scaleText);
  const scaledA6 = await page.exportJson();
  await page.clickObject('edge_1');
  await page.setSelect('dimension-kind', 'Length');
  await page.setSelect('dimension-mode', 'Driving');
  await page.setInput('dimension-value', 5 * scale);
  await page.setInput('dimension-label', 'width-5');
  await page.click('[data-action="apply-dimension"]');
  assert.equal(await page.exportJson(), scaledA6);
  assert.equal(await page.evaluate(`document.querySelectorAll('#last-attempt li').length`), 2);
  const conflictLabels = await page.evaluate(`[...document.querySelectorAll('#last-attempt li')].map((item) => item.textContent)`);

  await page.loadExample('a1', scaleText);
  await page.clickObject('width-4');
  await page.setInput('dimension-value', 6 * scale);
  await page.click('[data-action="apply-dimension"]');
  await page.clickObject('height_dimension');
  await page.click('[data-action="toggle-suppressed"]');
  await page.click('[data-action="zoom-fit"]');
  for (let index = 0; index < 7; index++) await page.click('[data-action="zoom-out"]');
  await page.click('[data-tool="point"]');
  await page.pointerClick(await page.modelClient(9 * scale, 9 * scale));
  const scaledE = await page.point('Point 5');
  assert.ok(scaledE);
  await page.click('[data-action="delete"]');
  await page.key('z', 'KeyZ', 2);
  assert.equal((await page.point('Point 5')).id, scaledE.id);
  await page.key('y', 'KeyY', 2);
  assert.equal(await page.point('Point 5'), null);

  await page.loadExample('a8', scaleText);
  const scaledA8 = await page.exportJson();
  const a8Report = await page.evaluate(`(() => { const root = document.querySelector('#playground-root'); return [root.dataset.rank, root.dataset.equalityDof, root.dataset.boundedDof]; })()`);
  await page.setInput('document-json', scaledA8);
  await page.click('[data-action="import-json"]');
  assert.equal(await page.exportJson(), scaledA8);

  await page.loadExample('corpus', scaleText);
  const corpusJson = await page.exportJson();
  const corpus = JSON.parse(corpusJson);
  assert.deepEqual(new Set(corpus.curves.map((item) => item.definition.kind)), new Set(['line', 'polyline', 'circle', 'circular_arc', 'quadratic_bezier', 'cubic_bezier']));
  const corpusConstraints = new Set(corpus.constraints.map((item) => item.definition.kind));
  for (const kind of ['fixed_point', 'coincident', 'horizontal', 'vertical', 'point_on_curve', 'parallel', 'perpendicular', 'equal_length', 'equal_radius', 'midpoint', 'symmetric_about_line', 'line_circle_tangency', 'circle_arc_tangency', 'line_curve_tangency', 'curve_curve_contact', 'curve_curve_tangency']) assert.ok(corpusConstraints.has(kind), `missing corpus constraint ${kind}`);
  assert.deepEqual(new Set(corpus.dimensions.map((item) => item.definition.kind)), new Set(['point_distance', 'curve_length', 'radius', 'diameter', 'oriented_angle']));
  await assertDomMatchesJson(page, corpusJson);
  const corpusReport = await page.evaluate(`(() => { const root = document.querySelector('#playground-root'); return [root.dataset.rank, root.dataset.equalityDof, root.dataset.boundedDof]; })()`);
  await page.click('[data-action="zoom-fit"]');
  const centerBeforePan = await page.evaluate(`document.querySelector('#playground-root').dataset.viewportCenterX`);
  await page.click('[data-tool="pan"]');
  await page.panCanvas(40, 20);
  assert.notEqual(await page.evaluate(`document.querySelector('#playground-root').dataset.viewportCenterX`), centerBeforePan);
  await page.click('[data-action="zoom-in"]');
  await page.click('[data-tool="select"]');
  await page.dragPoint('corpus point', 80 * scale, 2 * scale, 2);
  const editedCorpus = await page.exportJson();
  assert.notEqual(editedCorpus, corpusJson);
  await assertDomMatchesJson(page, editedCorpus);
  await page.assertAccepted();

  await page.loadExample('a1', scaleText);
  const scaledA9 = await page.exportJson();
  await page.clickObject('width-4');
  await page.setInput('dimension-value', -scale);
  await page.click('[data-action="apply-dimension"]');
  assert.equal(await page.exportJson(), scaledA9);
  await page.setInput('document-json', '{invalid scaled import');
  await page.click('[data-action="import-json"]');
  assert.equal(await page.exportJson(), scaledA9);

  await page.resize(844, 390);
  await page.click('[data-action="zoom-in"]');
  await page.resize(page.touch ? 390 : 1440, page.touch ? 844 : 1000);
  await page.assertAccepted();
  const a8Data = JSON.parse(scaledA8);
  return {
    ids: [a8Data.id, ...a8Data.points.map((item) => item.id), ...a8Data.scalars.map((item) => item.id), ...a8Data.curves.map((item) => item.id), ...a8Data.contacts.map((item) => item.id), ...a8Data.source_order],
    branches: a8Data.curves.map((item) => [item.id, item.definition.branch_direction, item.definition.branch_directions, item.definition.sweep]),
    contacts: a8Data.contacts.map((item) => [item.id, item.curve, item.domain, item.winding, item.neighborhood, item.tangent_orientation]),
    sourceKinds: [...a8Data.constraints.map((item) => [item.source_id, item.definition.kind, item.definition.side, item.definition.mode, item.definition.endpoint]), ...a8Data.dimensions.map((item) => [item.source_id, item.mode, item.suppressed])],
    rank: a8Report,
    conflictLabels,
    corpus: {
      ids: [corpus.id, ...corpus.points.map((item) => item.id), ...corpus.scalars.map((item) => item.id), ...corpus.curves.map((item) => item.id), ...corpus.contacts.map((item) => item.id), ...corpus.source_order],
      branches: corpus.curves.map((item) => [item.id, item.definition.branch_direction, item.definition.branch_directions, item.definition.sweep]),
      contacts: corpus.contacts.map((item) => [item.id, item.curve, item.domain, item.winding, item.neighborhood, item.tangent_orientation]),
      rank: corpusReport,
    },
  };
}

async function scenarioSuite(page, name) {
  await page.loadExample('a1');
  const originalIds = JSON.parse(await page.exportJson()).points.map((point) => point.id);
  await page.clickObject('width-4');
  await page.setInput('dimension-value', 6);
  await page.click('[data-action="apply-dimension"]');
  await page.clickObject('height_dimension');
  await page.setInput('dimension-value', 2.5);
  await page.click('[data-action="apply-dimension"]');
  const a1 = await page.exportJson();
  assert.deepEqual(JSON.parse(a1).points.map((point) => point.id), originalIds);
  assert.ok(JSON.parse(a1).scalars.some((item) => item.label.endsWith('.width') && item.value === 6));
  assert.match(await page.evaluate(`document.querySelector('#object-list').textContent`), /ref [0-9.]+/);
  await assertDomMatchesJson(page, a1);

  await page.loadExample('a2');
  const beforeA2 = await page.exportJson();
  const beforeB = await page.point('A2 B');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.equalityDof`), '1');
  await page.dragPoint('A2 C', 0, 3);
  const b = await page.point('A2 B');
  const afterA2 = await page.exportJson();
  assert.notEqual(afterA2, beforeA2);
  near(b.x, beforeB.x, 1e-9);
  near(b.y, beforeB.y, 1e-9);
  await assertDomMatchesJson(page, afterA2);
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.equalityDof`), '1');

  await page.loadExample('a3');
  const a3 = await page.exportJson();
  const a3Data = JSON.parse(a3);
  assert.equal(a3Data.contacts.length, 2);
  assert.ok(a3Data.contacts.every((contact) => contact.winding === 0 && contact.neighborhood === 'interior' && contact.tangent_orientation === 'aligned'));
  const contacts = await page.evaluate(`[...document.querySelectorAll('[data-contact-id]')].map((item) => [Number(item.dataset.modelX), Number(item.dataset.modelY)])`);
  assert.equal(contacts.length, 2);
  near(contacts[0][0], contacts[1][0], 1e-9);
  near(contacts[0][1], contacts[1][1], 1e-9);
  await page.clickObject('A3 line contact');
  await page.clickObject('A3 circle contact');
  await page.setInput('contact-parameter', 1.1);
  await page.click('[data-action="apply-branch-state"]');
  assert.equal(await page.exportJson(), a3);
  assert.match(await page.evaluate(`document.querySelector('#last-attempt').textContent`), /domain|bound|not changed/i);
  await page.setInput('contact-parameter', '');

  await page.loadExample('a4');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.equalityDof`), '2');
  const initialFreeRadius = JSON.parse(await page.exportJson()).scalars.find((scalar) => scalar.label === 'A4 free circle radius').value;
  await page.dragPoint('A4 circle center', 8, 1, 4);
  const a4 = await page.exportJson();
  const a4Data = JSON.parse(a4);
  const freeRadius = a4Data.scalars.find((scalar) => scalar.label === 'A4 free circle radius').value;
  assert.ok(freeRadius > 0 && freeRadius !== initialFreeRadius);
  const a4Contacts = await page.evaluate(`[...document.querySelectorAll('[data-contact-id]')].map((item) => [Number(item.dataset.modelX), Number(item.dataset.modelY)])`);
  assert.equal(a4Contacts.length, 2);
  near(a4Contacts[0][0], a4Contacts[1][0], 1e-9);
  near(a4Contacts[0][1], a4Contacts[1][1], 1e-9);
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.equalityDof`), '2');
  await page.dragPoint('A4 circle center', -8, 0);
  assert.equal(await page.exportJson(), a4);
  assert.match(await page.evaluate(`document.querySelector('#last-attempt').textContent`), /rejected|retained/i);

  await page.loadExample('a5');
  const stableA5Controls = {
    p2: await page.point('A5 P2'),
    p3: await page.point('A5 P3'),
  };
  await page.dragPoint('A5 line B', Math.SQRT2, Math.SQRT2, 8);
  const draggedLineEnd = await page.point('A5 line B');
  near(Math.hypot(draggedLineEnd.x, draggedLineEnd.y), 2, 1e-8);
  assert.ok(draggedLineEnd.x > 0 && draggedLineEnd.y > 0);
  const stableA5ControlsAfter = {
    p2: await page.point('A5 P2'),
    p3: await page.point('A5 P3'),
  };
  for (const key of ['p2', 'p3']) {
    near(stableA5ControlsAfter[key].x, stableA5Controls[key].x, 1e-9);
    near(stableA5ControlsAfter[key].y, stableA5Controls[key].y, 1e-9);
  }
  assert.doesNotMatch(await page.evaluate(`document.querySelector('#last-attempt').textContent`), /NumericalFailure|opposite branch/);

  await page.loadExample('a5');
  const initialTangentEnd = await page.point('A5 line B');
  await page.dragPoint('A5 P1', 1, 0.5, 8);
  const tangentEnd = await page.point('A5 line B');
  assert.ok(tangentEnd.x !== initialTangentEnd.x || tangentEnd.y !== initialTangentEnd.y);
  const a5 = await page.exportJson();
  const invalidA5 = JSON.parse(a5);
  invalidA5.points.find((point) => point.label === 'A5 P1').position = [...invalidA5.points.find((point) => point.label === 'A5 P0').position];
  await page.setInput('document-json', JSON.stringify(invalidA5));
  await page.click('[data-action="import-json"]');
  assert.equal(await page.exportJson(), a5);

  await page.loadExample('a1');
  const a6 = await page.exportJson();
  await page.clickObject('edge_1');
  await page.setSelect('dimension-kind', 'Length');
  await page.setSelect('dimension-mode', 'Driving');
  await page.setInput('dimension-value', 5);
  await page.setInput('dimension-label', 'width-5');
  await page.click('[data-action="apply-dimension"]');
  assert.equal(await page.exportJson(), a6);
  const conflictText = await page.evaluate(`document.querySelector('#last-attempt').textContent`);
  assert.match(conflictText, /Complete/);
  assert.match(conflictText, /width-4/);
  assert.deepEqual(
    await page.evaluate(`[...document.querySelectorAll('#last-attempt li')].map((item) => item.textContent)`),
    ['width-4', 'width-5'],
  );

  const beforeUndo = await historySuite(page);
  await page.reload();
  assert.equal(await page.exportJson(), beforeUndo);

  await reportedRegressionSuite(page);
  await creationSuite(page);
  await stressExampleSuite(page);

  let scaleFingerprint;
  for (const scale of ['0.000001', '1', '1000000']) {
    const fingerprint = await scaleWorkflow(page, scale);
    if (scaleFingerprint) assert.deepEqual(fingerprint, scaleFingerprint);
    else scaleFingerprint = fingerprint;
  }
  page.assertNoErrors();
  console.log(`${name}: A1-A10 pointer/touch, keyboard, recovery, scale, and resize paths passed`);
}

async function reportedRegressionSuite(page) {
  await page.click('[data-action="new"]');
  await page.click('[data-tool="line"]');
  await page.pointerClick(await page.modelClient(0, 0));
  await page.pointerClick(await page.modelClient(2, 0));
  await page.click('[data-tool="select"]');
  await page.dragPoint('Line control 2', -2, 0, 5);
  const freeEnd = await page.point('Line control 2');
  assert.ok(freeEnd.x < -1.9);
  assert.ok(Math.abs(freeEnd.y) < 0.05, JSON.stringify(freeEnd));
  assert.doesNotMatch(await page.evaluate(`document.querySelector('#last-attempt').textContent`), /opposite branch|NumericalFailure/);

  await page.click('[data-action="new"]');
  await page.click('[data-tool="rectangle"]');
  await page.pointerClick(await page.modelClient(0, 0));
  await page.pointerClick(await page.modelClient(4, 3));
  await page.click('[data-tool="select"]');
  const freeRectangle = JSON.parse(await page.exportJson());
  assert.equal(freeRectangle.dimensions.length, 0);
  assert.equal(freeRectangle.scalars.length, 0);
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.equalityDof`), '4');
  const originalBottomLeft = await page.point('Rectangle 1.bottom_left');
  const originalTopRight = await page.point('Rectangle 1.top_right');
  await page.dragPoint('Rectangle 1.bottom_left', 1, 1, 2);
  const movedBottomLeft = await page.point('Rectangle 1.bottom_left');
  const movedTopRight = await page.point('Rectangle 1.top_right');
  assert.ok(movedBottomLeft.x > 0.9 && movedBottomLeft.y > 0.9);
  near(movedTopRight.x, originalTopRight.x, 1e-8);
  near(movedTopRight.y, originalTopRight.y, 1e-8);
  assert.notEqual(movedTopRight.x - movedBottomLeft.x, originalTopRight.x - originalBottomLeft.x);
  assert.notEqual(movedTopRight.y - movedBottomLeft.y, originalTopRight.y - originalBottomLeft.y);
  await page.assertAccepted();
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.equalityDof`), '4');

  await page.loadExample('a1');
  const beforeConstraintDelete = await page.exportJson();
  const deletedConstraint = JSON.parse(beforeConstraintDelete).constraints.find((item) => item.label.includes('bottom_horizontal'));
  assert.ok(deletedConstraint);
  await page.deleteObject('bottom_horizontal');
  const afterConstraintDelete = JSON.parse(await page.exportJson());
  assert.equal(afterConstraintDelete.constraints.length, JSON.parse(beforeConstraintDelete).constraints.length - 1);
  assert.equal(afterConstraintDelete.constraints.some((item) => item.id === deletedConstraint.id), false);
  await page.key('z', 'KeyZ', 2);
  assert.equal(await page.exportJson(), beforeConstraintDelete);
  const rectangleObjects = [
    'bottom_left',
    'bottom_right',
    'top_right',
    'top_left',
    'edge_1',
    'edge_2',
    'edge_3',
    'edge_4',
  ];
  for (let index = 0; index < rectangleObjects.length; index++) {
    await page.clickObject(rectangleObjects[index], index !== 0);
  }
  await page.click('[data-action="delete"]');
  const deleted = JSON.parse(await page.exportJson());
  for (const collection of ['points', 'scalars', 'curves', 'contacts', 'constraints', 'dimensions', 'source_order']) {
    assert.equal(deleted[collection].length, 0, `${collection} survived full rectangle deletion`);
  }
}

async function fileSuite(page, browser, name) {
  await page.loadExample('a8');
  const canonical = await page.exportJson();
  await browser.send('Browser.setDownloadBehavior', { behavior: 'allow', downloadPath: downloads, eventsEnabled: true });
  await rm(join(downloads, 'geosolve-sketch.json'), { force: true });
  await page.click('[data-action="download-json"]');
  await waitForFile(downloads, 'geosolve-sketch.json');
  const downloadedPath = join(downloads, 'geosolve-sketch.json');
  assert.equal(await readFile(downloadedPath, 'utf8'), canonical);
  await page.click('[data-action="new"]');
  await page.upload(downloadedPath);
  assert.equal(await page.exportJson(), canonical);
  page.assertNoErrors();
  console.log(`${name}: lossless download/upload passed`);
}

async function recoverySuite(page, name) {
  await page.loadExample('a1');
  await page.clickObject('width-4');
  await page.setInput('dimension-value', 6);
  await page.click('[data-action="apply-dimension"]');
  await page.key('z', 'KeyZ', 2);
  const accepted = await page.exportJson();
  const acceptedData = JSON.parse(accepted);
  assert.equal(await page.evaluate(`document.querySelector('#redo').disabled`), false);
  const beforeNegative = await page.evaluate(`(() => { const root = document.querySelector('#playground-root'); return { revision: root.dataset.authoritativeRevision, historyLength: root.dataset.historyLength, historyCursor: root.dataset.historyCursor, redo: document.querySelector('#redo').disabled, svg: document.querySelector('#sketch-viewport').innerHTML, audit: document.querySelector('#playground-audit').innerHTML, storage: localStorage.getItem('geosolve.sketch-playground.accepted.v1') }; })()`);
  await page.clickObject('width-4');
  await page.setInput('dimension-value', -1);
  await page.click('[data-action="apply-dimension"]');
  assert.equal(await page.exportJson(), accepted);
  const retained = await page.evaluate(`(() => { const root = document.querySelector('#playground-root'); return { revision: root.dataset.authoritativeRevision, historyLength: root.dataset.historyLength, historyCursor: root.dataset.historyCursor, redo: document.querySelector('#redo').disabled, svg: document.querySelector('#sketch-viewport').innerHTML, audit: document.querySelector('#playground-audit').innerHTML, storage: localStorage.getItem('geosolve.sketch-playground.accepted.v1') }; })()`);
  assert.deepEqual(retained, beforeNegative);
  const cases = [];
  cases.push('{not JSON');
  cases.push(JSON.stringify({ ...acceptedData, version: 2 }));
  const duplicate = structuredClone(acceptedData);
  duplicate.points[1].id = duplicate.points[0].id;
  cases.push(JSON.stringify(duplicate));
  const dangling = structuredClone(acceptedData);
  dangling.curves[0].definition.start = 'ffffffffffffffffffffffffffffffff';
  cases.push(JSON.stringify(dangling));
  cases.push(accepted.replace('"model_scale":10.0', '"model_scale":1e999'));
  const oversized = structuredClone(acceptedData);
  oversized.points[0].label = 'x'.repeat(1025);
  cases.push(JSON.stringify(oversized));
  for (const payload of cases) {
    await page.setInput('document-json', payload);
    await page.click('[data-action="import-json"]');
    assert.equal(await page.exportJson(), accepted);
    const after = await page.evaluate(`(() => { const root = document.querySelector('#playground-root'); return { revision: root.dataset.authoritativeRevision, historyLength: root.dataset.historyLength, historyCursor: root.dataset.historyCursor, redo: document.querySelector('#redo').disabled, svg: document.querySelector('#sketch-viewport').innerHTML, audit: document.querySelector('#playground-audit').innerHTML, storage: localStorage.getItem('geosolve.sketch-playground.accepted.v1') }; })()`);
    assert.deepEqual(after, retained);
    assert.match(await page.evaluate(`document.querySelector('#last-attempt').textContent`), /failed atomically|retained/i);
  }

  await page.evaluate(`window.__geosolveSetItem = Storage.prototype.setItem; Storage.prototype.setItem = function () { throw new DOMException('forced quota failure', 'QuotaExceededError'); }`);
  await page.key('y', 'KeyY', 2);
  assert.match(await page.evaluate(`document.querySelector('#storage-status').textContent`), /rejected the save/i);
  assert.equal(await page.evaluate(`localStorage.getItem('geosolve.sketch-playground.accepted.v1')`), accepted);
  await page.evaluate(`Storage.prototype.setItem = window.__geosolveSetItem; delete window.__geosolveSetItem`);
  await page.click('[data-action="zoom-in"]');
  let recoveredSave = await page.exportJson();
  assert.equal(await page.evaluate(`localStorage.getItem('geosolve.sketch-playground.accepted.v1')`), recoveredSave);

  await page.evaluate(`window.__geosolveSetItem = Storage.prototype.setItem; Storage.prototype.setItem = function (key, value) { if (key.includes('backup')) throw new DOMException('forced backup failure', 'QuotaExceededError'); return window.__geosolveSetItem.call(this, key, value); }`);
  await page.clickObject('width-4');
  await page.setInput('dimension-value', 7);
  await page.click('[data-action="apply-dimension"]');
  assert.match(await page.evaluate(`document.querySelector('#storage-status').textContent`), /backup will retry/i);
  await page.evaluate(`Storage.prototype.setItem = window.__geosolveSetItem; delete window.__geosolveSetItem`);
  await page.click('[data-action="zoom-out"]');
  recoveredSave = await page.exportJson();
  assert.equal(await page.evaluate(`localStorage.getItem('geosolve.sketch-playground.accepted.backup.v1')`), recoveredSave);

  await page.evaluate(`localStorage.setItem('geosolve.sketch-playground.accepted.v1', '{corrupt autosave')`);
  await page.reload();
  assert.equal(await page.exportJson(), recoveredSave);
  assert.match(await page.evaluate(`document.querySelector('#last-attempt').textContent`), /recovered the last valid backup/i);
  assert.equal(await page.evaluate(`localStorage.getItem('geosolve.sketch-playground.accepted.v1')`), '{corrupt autosave');
  await page.evaluate(`localStorage.setItem('geosolve.sketch-playground.accepted.v1', localStorage.getItem('geosolve.sketch-playground.accepted.backup.v1'))`);
  page.assertNoErrors();
  console.log(`${name}: invalid edit/import, autosave retry, and backup recovery passed`);
}

async function branchHistoryRecoverySuite(page, name) {
  await page.loadExample('a3');
  await page.clickObject('A3 line contact');
  await page.clickObject('A3 circle contact');
  await page.setInput('contact-winding', 0);
  await page.setInput('second-contact-winding', 1);
  await page.click('[data-action="apply-branch-state"]');
  const branched = await page.exportJson();
  assert.deepEqual(JSON.parse(branched).contacts.map((contact) => contact.winding), [0, 1]);
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.historyCursor`), '1');

  await page.key('z', 'KeyZ', 2);
  assert.deepEqual(JSON.parse(await page.exportJson()).contacts.map((contact) => contact.winding), [0, 0]);
  await page.key('y', 'KeyY', 2);
  assert.equal(await page.exportJson(), branched);
  assert.equal(await page.evaluate(`localStorage.getItem('geosolve.sketch-playground.accepted.v1')`), branched);
  await page.reload();
  assert.equal(await page.exportJson(), branched);

  await page.evaluate(`localStorage.setItem('geosolve.sketch-playground.accepted.v1', '{corrupt branch autosave')`);
  await page.reload();
  assert.equal(await page.exportJson(), branched);
  assert.deepEqual(JSON.parse(branched).contacts.map((contact) => contact.winding), [0, 1]);
  assert.match(await page.evaluate(`document.querySelector('#last-attempt').textContent`), /recovered the last valid backup/i);
  await page.evaluate(`localStorage.setItem('geosolve.sketch-playground.accepted.v1', localStorage.getItem('geosolve.sketch-playground.accepted.backup.v1'))`);
  page.assertNoErrors();
  console.log(`${name}: branch undo/redo, autosave, reload, and backup recovery passed`);
}

async function renderBudgets(page) {
  for (const [kind, budget] of [['a8', 75], ['medium', 400]]) {
    await page.loadExample(kind);
    const samples = [];
    for (let index = 0; index < 12; index++) {
      const selector = index % 2 === 0 ? '[data-action="zoom-in"]' : '[data-action="zoom-out"]';
      samples.push(await page.evaluate(`(() => { const started = performance.now(); document.querySelector(${JSON.stringify(selector)}).click(); return performance.now() - started; })()`));
    }
    samples.sort((a, b) => a - b);
    const p95 = samples[Math.ceil(samples.length * 0.95) - 1];
    console.log(`${kind}/render: p95=${p95.toFixed(3)}ms budget=${budget}ms`);
    assert.ok(p95 <= budget, `${kind} render p95 ${p95}ms exceeded ${budget}ms`);
  }
  page.assertNoErrors();
}

async function waitForFile(directory, name) {
  const started = Date.now();
  let priorSize = -1;
  while (Date.now() - started < 10_000) {
    const files = await readdir(directory);
    if (files.includes(name) && !files.some((file) => file.endsWith('.crdownload'))) {
      const size = (await stat(join(directory, name))).size;
      if (size > 0 && size === priorSize) return;
      priorSize = size;
    }
    await new Promise((resolveWait) => setTimeout(resolveWait, 25));
  }
  throw new Error(`download ${name} did not complete`);
}

let server;
let chrome;
try {
  await mkdir(downloads, { recursive: true });
  await rm(downloads, { recursive: true, force: true });
  await mkdir(downloads, { recursive: true });
  const serving = await startServer();
  server = serving.server;
  chrome = await startChromium();
  const desktop = await openPage(chrome.cdp.socket.url, serving.url, { width: 1440, height: 1000 }, false);
  await scenarioSuite(desktop, 'desktop');
  await fileSuite(desktop, chrome.cdp, 'desktop');
  await recoverySuite(desktop, 'desktop');
  await branchHistoryRecoverySuite(desktop, 'desktop');
  await renderBudgets(desktop);
  const mobile = await openPage(chrome.cdp.socket.url, serving.url, { width: 390, height: 844 }, true);
  await scenarioSuite(mobile, 'mobile');
  await fileSuite(mobile, chrome.cdp, 'mobile');
  await recoverySuite(mobile, 'mobile');
  await branchHistoryRecoverySuite(mobile, 'mobile');
  desktop.cdp.close();
  mobile.cdp.close();
  console.log('M14 browser E2E passed');
} catch (error) {
  console.error(error.stack || error);
  if (chrome) console.error(chrome.stderr());
  process.exitCode = 1;
} finally {
  if (server) await new Promise((resolveClose) => server.close(resolveClose));
  if (chrome) {
    chrome.cdp.close();
    if (chrome.process.exitCode === null) {
      chrome.process.kill('SIGTERM');
      await new Promise((resolveExit) => {
        const timeout = setTimeout(resolveExit, 2_000);
        chrome.process.once('exit', () => {
          clearTimeout(timeout);
          resolveExit();
        });
      });
      if (chrome.process.exitCode === null) {
        chrome.process.kill('SIGKILL');
        await new Promise((resolveExit) => chrome.process.once('exit', resolveExit));
      }
    }
    await rm(chrome.profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
  }
}
