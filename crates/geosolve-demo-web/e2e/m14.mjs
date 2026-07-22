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

  async clickObjectExact(label, additive = false) {
    const before = Number(await this.evaluate(`document.querySelector('#playground-root').dataset.renderSequence`));
    const found = await this.evaluate(`(() => { const button = [...document.querySelectorAll('.object-row')].find((item) => item.children[1]?.textContent === ${JSON.stringify(label)}); if (!button) return false; button.dispatchEvent(new MouseEvent('click', { bubbles: true, shiftKey: ${additive} })); return true; })()`);
    assert.equal(found, true, `missing exact object row ${label}`);
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
      const prefix = ['shaft-bearing', 'block-base'].includes(kind) ? 'accepted spatial' : 'canonical';
      assert.match(
        await this.evaluate(`document.querySelector('#last-attempt').textContent`),
        new RegExp(`${prefix} ${kind}`, 'i'),
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

  async configurationHandle(label) {
    return this.evaluate(`(() => { const handle = [...document.querySelectorAll('[data-configuration-handle]')].find((item) => item.dataset.label === ${JSON.stringify(label)}); if (!handle) return null; document.querySelector('#sketch-viewport').scrollIntoView({ block: 'center', inline: 'center' }); const rect = handle.getBoundingClientRect(); return { x: Number(handle.dataset.modelX), y: Number(handle.dataset.modelY), clientX: rect.left + rect.width / 2, clientY: rect.top + rect.height / 2, curve: handle.dataset.configurationCurveId, kind: handle.dataset.configurationHandle }; })()`);
  }

  async modelClient(x, y) {
    return this.evaluate(`(() => { const root = document.querySelector('#playground-root'); const viewport = document.querySelector('#sketch-viewport'); viewport.scrollIntoView({ block: 'center', inline: 'center' }); const svg = viewport.getBoundingClientRect(); const sx = 500 + (${x} - Number(root.dataset.viewportCenterX)) * Number(root.dataset.pixelsPerUnit); const sy = 350 - (${y} - Number(root.dataset.viewportCenterY)) * Number(root.dataset.pixelsPerUnit); return { x: svg.left + sx * svg.width / 1000, y: svg.top + sy * svg.height / 700 }; })()`);
  }

  async pointerClick(point) {
    const before = Number(await this.evaluate(`document.querySelector('#playground-root').dataset.renderSequence`));
    const target = await this.evaluate(`(() => { const target = document.elementFromPoint(${point.x}, ${point.y}); return { tag: target?.tagName, id: target?.id, inViewport: target?.closest('#sketch-viewport') !== null }; })()`);
    assert.equal(target.inViewport, true, `pointer point missed viewport: ${JSON.stringify(target)} at ${JSON.stringify(point)}`);
    if (this.touch) {
      await this.evaluate(`(() => { const target = document.elementFromPoint(${point.x}, ${point.y}); if (!target?.closest('#sketch-viewport')) throw new Error('touch point missed viewport: ' + target?.tagName + '#' + target?.id); target.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, cancelable: true, clientX: ${point.x}, clientY: ${point.y}, pointerId: 41, pointerType: 'touch', isPrimary: true, button: 0, buttons: 1 })); document.elementFromPoint(${point.x}, ${point.y}).dispatchEvent(new PointerEvent('pointerup', { bubbles: true, cancelable: true, clientX: ${point.x}, clientY: ${point.y}, pointerId: 41, pointerType: 'touch', isPrimary: true, button: 0, buttons: 0 })); return true; })()`);
    } else {
      await this.cdp.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: point.x, y: point.y });
      await this.cdp.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: point.x, y: point.y, button: 'left', clickCount: 1 });
      await this.cdp.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: point.x, y: point.y, button: 'left', clickCount: 1 });
    }
    await this.waitFor(`Number(document.querySelector('#playground-root').dataset.renderSequence) > ${before}`);
  }

  async boxSelect(first, second) {
    const start = await this.modelClient(...first);
    const end = await this.modelClient(...second);
    const before = Number(await this.evaluate(`document.querySelector('#playground-root').dataset.renderSequence`));
    if (this.touch) {
      await this.cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x: start.x, y: start.y, id: 1, radiusX: 8, radiusY: 8 }] });
      await this.cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: [{ x: end.x, y: end.y, id: 1, radiusX: 8, radiusY: 8 }] });
      await this.cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
    } else {
      await this.cdp.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: start.x, y: start.y });
      await this.cdp.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: start.x, y: start.y, button: 'left', clickCount: 1 });
      await this.cdp.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: end.x, y: end.y, button: 'left', buttons: 1 });
      await this.cdp.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: end.x, y: end.y, button: 'left', clickCount: 1 });
    }
    await this.waitFor(`Number(document.querySelector('#playground-root').dataset.renderSequence) > ${before}`);
  }

  async hoverModel(x, y) {
    assert.equal(this.touch, false, 'hover preview uses the desktop pointer path');
    const point = await this.modelClient(x, y);
    const before = Number(await this.evaluate(`document.querySelector('#playground-root').dataset.renderSequence`));
    await this.cdp.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: point.x, y: point.y });
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

  async dragConfigurationHandle(label, x, y, steps = 4) {
    const start = await this.configurationHandle(label);
    assert.ok(start, `missing configuration handle ${label}`);
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

  async burstDragPoint(label, x, y, steps = 40) {
    assert.equal(this.touch, false, 'burst drag profiling uses the desktop pointer path');
    const start = await this.point(label);
    assert.ok(start, `missing point ${label}`);
    const target = await this.modelClient(x, y);
    await this.cdp.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: start.clientX, y: start.clientY });
    await this.cdp.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: start.clientX, y: start.clientY, button: 'left', clickCount: 1 });
    assert.equal(await this.evaluate(`document.querySelector('#playground-root').dataset.detailRefresh`), 'deferred');
    assert.match(await this.evaluate(`document.querySelector('#playground-audit').textContent`), /refreshes when the drag is released/i);
    const before = Number(await this.evaluate(`document.querySelector('#playground-root').dataset.renderSequence`));
    const started = Date.now();
    await this.evaluate(`(() => { const viewport = document.querySelector('#sketch-viewport'); for (let index = 1; index <= ${steps}; index++) { const fraction = index / ${steps}; viewport.dispatchEvent(new PointerEvent('pointermove', { bubbles: true, cancelable: true, clientX: ${start.clientX} + (${target.x} - ${start.clientX}) * fraction, clientY: ${start.clientY} + (${target.y} - ${start.clientY}) * fraction, pointerId: 1, pointerType: 'mouse', isPrimary: true, button: -1, buttons: 1 })); } return true; })()`);
    await this.waitFor(`Number(document.querySelector('#playground-root').dataset.renderSequence) > ${before}`, 30_000);
    const elapsed = Date.now() - started;
    const after = Number(await this.evaluate(`document.querySelector('#playground-root').dataset.renderSequence`));
    assert.equal(after - before, 1, `${steps} queued pointer moves produced ${after - before} renders`);
    await this.cdp.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: target.x, y: target.y, button: 'left', clickCount: 1 });
    await this.waitFor(`document.querySelector('#playground-root').dataset.detailRefresh !== 'deferred' && Number(document.querySelector('#playground-root').dataset.renderSequence) > ${after}`, 30_000);
    assert.doesNotMatch(await this.evaluate(`document.querySelector('#playground-audit').textContent`), /refreshes when the drag is released/i);
    return { elapsed, renders: after - before };
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
    '--no-proxy-server',
    '--ozone-platform=headless',
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
    let page;
    try {
      const created = await fetch(`http://${endpoint.host}/json/new?${encodeURIComponent('about:blank')}`, { method: 'PUT' }).then((response) => response.json());
      page = new BrowserPage(await new Cdp(created.webSocketDebuggerUrl).open(), viewport, touch);
      await page.initialize(url);
      return page;
    } catch (error) {
      lastError = error;
      if (page) {
        await page.cdp.send('Page.close').catch(() => {});
        page.cdp.close();
      }
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

async function acceptedBrowserEvidence(page) {
  const canonicalJson = await page.exportJson();
  const evidence = await page.evaluate(`(() => {
    const root = document.querySelector('#playground-root');
    const attributes = (element, names) => Object.fromEntries(names.map((name) => [name, element.getAttribute(name)]));
    return {
      revision: root.dataset.authoritativeRevision,
      historyLength: root.dataset.historyLength,
      historyCursor: root.dataset.historyCursor,
      hardValidity: root.dataset.hardValidity,
      hardResidualMax: root.dataset.hardResidualMax,
      badge: document.querySelector('#solve-badge').textContent,
      points: [...document.querySelectorAll('[data-point-id]')].map((point) => attributes(point, ['data-point-id', 'data-model-x', 'data-model-y'])),
      contacts: [...document.querySelectorAll('[data-contact-id]')].map((contact) => attributes(contact, ['data-contact-id', 'data-model-x', 'data-model-y'])),
      curves: [...document.querySelectorAll('.playground-curve')].map((curve) => attributes(curve, ['data-curve-id', 'data-span-id', 'data-visible-start', 'data-visible-end', 'd', 'cx', 'cy', 'r'])),
      trimMarkers: [...document.querySelectorAll('[data-derived-trim-marker]')].map((marker) => attributes(marker, ['cx', 'cy'])),
      profileOverlays: [...document.querySelectorAll('.visual-profile-overlay')].map((overlay) => attributes(overlay, ['d', 'fill-rule'])),
      profileStatus: root.getAttribute('data-profile-status'),
      profileFaces: root.getAttribute('data-profile-face-count'),
    };
  })()`);
  return { canonicalJson, ...evidence };
}

async function assertAcceptedEvidenceRetained(page, before, context) {
  const after = await acceptedBrowserEvidence(page);
  assert.deepEqual(after, before, `${context} changed accepted browser evidence`);
  await page.assertAccepted();
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

  await page.loadExample('motion-trammel');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.boundedDof`), '1');
  await page.dragPoint('Trammel elliptic tracer T', 0, 1.25, 10);
  const trammelA = await page.point('Trammel horizontal slider A');
  const trammelB = await page.point('Trammel vertical slider B');
  near(trammelA.x, 0, 0.08);
  near(trammelA.y, 0, 1e-8);
  near(trammelB.x, 0, 1e-8);
  near(trammelB.y, 5, 0.08);

  await page.loadExample('motion-scotch-yoke');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.equalityDof`), '1');
  await page.dragPoint('Yoke crank pin P', 0, 5, 8);
  const yokePin = await page.point('Yoke crank pin P');
  const yokeSlider = await page.point('Yoke horizontal slider S');
  near(Math.hypot(yokePin.x, yokePin.y), 5, 1e-8);
  near(yokeSlider.x, yokePin.x, 1e-8);
  near(yokeSlider.y, -6, 1e-8);

  await page.loadExample('motion-rotating-square');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.equalityDof`), '1');
  const rotatedSide = 3 / Math.sqrt(2);
  await page.dragPoint('Rotating square corner B', rotatedSide, rotatedSide, 8);
  const squareB = await page.point('Rotating square corner B');
  const squareC = await page.point('Rotating square corner C');
  near(Math.hypot(squareB.x, squareB.y), 3, 1e-8);
  near(squareC.x, 0, 0.08);
  near(squareC.y, 2 * rotatedSide, 0.08);

  await page.loadExample('motion-scissor');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.equalityDof`), '1');
  await page.dragPoint('Scissor base slider B', 2, 0, 8);
  const scissorUpper = await page.point('Scissor upper joint U');
  const scissorLower = await page.point('Scissor lower joint L');
  near(scissorUpper.x, -1, 0.08);
  near(scissorUpper.y, 4, 0.08);
  near(scissorLower.x, -1, 0.08);
  near(scissorLower.y, -4, 0.08);

  await page.loadExample('motion-scissor-tower');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.equalityDof`), '1');
  if (page.touch) {
    await page.dragPoint('Tower level 0 right', 2, 0, 10);
  } else {
    const profile = await page.burstDragPoint('Tower level 0 right', 2, 0);
    console.log(`tower/burst-drag: ${profile.elapsed}ms renders=${profile.renders} budget=100ms`);
    assert.ok(profile.elapsed <= 100, `tower burst drag ${profile.elapsed}ms exceeded 100ms`);
  }
  const towerTopLeft = await page.point('Tower level 5 left');
  const towerTopRight = await page.point('Tower level 5 right');
  near(towerTopLeft.x, -4, 0.08);
  near(towerTopLeft.y, 40, 0.08);
  near(towerTopRight.x, 2, 0.08);
  near(towerTopRight.y, 40, 0.08);

  await page.loadExample('motion-peaucellier');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.equalityDof`), '1');
  await page.dragPoint('Peaucellier circular input P', 6, 2 * Math.sqrt(3), 10);
  const peaucellierOutput = await page.point('Peaucellier straight-line output Q');
  near(peaucellierOutput.x, 2, 1e-8);
  near(peaucellierOutput.y, 2 / Math.sqrt(3), 0.08);

  await page.loadExample('diagnostic-rank-drop');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.leftNullity`), '1');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.equalityDof`), '1');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.structuralClassification`), 'Well');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.structuralLeftNullity`), '0');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.structuralRightNullity`), '0');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.requestedBackend`), 'Auto');
  assert.ok((await page.evaluate(`document.querySelector('#playground-root').dataset.actualBackend`)).length > 0);
  assert.match(await page.evaluate(`document.querySelector('#playground-solve-status').textContent`), /structural classWell/i);

  await page.loadExample('diagnostic-endpoint-bound');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.equalityDof`), '2');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.boundedDof`), '0');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.oneSidedMotion`), 'Exists');
  assert.match(await page.evaluate(`document.querySelector('#object-list').textContent`), /Endpoint-fixed contact t=1/);

  await page.loadExample('diagnostic-redundancy');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.leftNullity`), '1');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.structuralClassification`), 'Over');
  assert.match(await page.evaluate(`document.querySelector('#playground-audit').textContent`), /redundant/i);
  await page.assertAccepted();
}

async function newDomainExampleSuite(page) {
  const releaseContract = await page.evaluate(`(() => {
    const panel = document.querySelector('#release-contract');
    return {
      text: panel.textContent,
      links: [...panel.querySelectorAll('a')].map((link) => link.getAttribute('href')),
    };
  })()`);
  assert.match(releaseContract.text, /0\.2\.0 supported preview/);
  assert.match(releaseContract.text, /reads v1-v4, writes canonical v4/);
  assert.match(releaseContract.text, /Construction and NURBS/);
  assert.match(releaseContract.text, /Visual profiles/);
  assert.match(releaseContract.text, /Failure handoff/);
  assert.deepEqual(releaseContract.links, ['API_COMPATIBILITY.md', 'M32_SCALE_PERFORMANCE.md']);

  assert.deepEqual(
    await page.evaluate(`[...document.querySelectorAll('#alpha-example option')].map((option) => option.value).filter((value) => ['conic-gallery', 'conic-tangency', 'conic-circle-limit', 'shaft-bearing', 'block-base', 'm28-trimmed-fillet'].includes(value))`),
    ['conic-gallery', 'conic-tangency', 'conic-circle-limit', 'shaft-bearing', 'block-base', 'm28-trimmed-fillet'],
  );

  await page.loadExample('conic-gallery', '0.000001');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.exampleMode`), 'sketch');
  assert.ok(Number(await page.evaluate(`document.querySelectorAll('.playground-curve').length`)) >= 5);
  assert.match(await page.evaluate(`document.querySelector('#object-list').textContent`), /Ellipse - full periodic conic/);
  assert.match(await page.evaluate(`document.querySelector('#object-list').textContent`), /Hyperbola - negative branch reversed trim/);
  const conicJson = await page.exportJson();
  assert.equal(JSON.parse(conicJson).curves.length, 5);
  assert.equal(await page.evaluate(`localStorage.getItem('geosolve.sketch-playground.accepted.v1')`), conicJson);

  const storageBeforeSpatial = await page.evaluate(`localStorage.getItem('geosolve.sketch-playground.accepted.v1')`);
  await page.loadExample('shaft-bearing', '1000000');
  const spatial = await page.evaluate(`(() => { const root = document.querySelector('#playground-root'); return { mode: root.dataset.exampleMode, rank: root.dataset.rank, gauge: root.dataset.totalGaugeDof, internal: root.dataset.totalInternalMobility, profileStatus: root.getAttribute('data-profile-status'), profilePanel: getComputedStyle(document.querySelector('#profile-analysis-section')).display, pointTool: getComputedStyle(document.querySelector('[data-tool="point"]')).display, persistence: getComputedStyle(document.querySelector('.persistence-section')).display, bodies: document.querySelectorAll('[data-spatial-body-id]').length, axes: document.querySelectorAll('[data-spatial-axis-id]').length, planes: document.querySelectorAll('[data-spatial-plane-id]').length, audit: document.querySelectorAll('#playground-audit [data-source-id]').length }; })()`);
  assert.deepEqual(spatial, {
    mode: 'spatial',
    rank: '6',
    gauge: '0',
    internal: '0',
    profileStatus: null,
    profilePanel: 'none',
    pointTool: 'none',
    persistence: 'none',
    bodies: 2,
    axes: 2,
    planes: 1,
    audit: 4,
  });
  assert.match(await page.evaluate(`document.querySelector('#object-list').textContent`), /Shaft hinge coordinate.*phase.*winding/s);
  assert.match(await page.evaluate(`document.querySelector('#object-list').textContent`), /Shaft winding 2 mode.*retained.*normalized/s);
  assert.match(await page.evaluate(`document.querySelector('#playground-solve-status').textContent`), /physical rank6.*gauge DOF0.*internal mobility0/s);
  assert.equal(await page.evaluate(`localStorage.getItem('geosolve.sketch-playground.accepted.v1')`), storageBeforeSpatial);

  const centerBeforePan = await page.evaluate(`document.querySelector('#playground-root').dataset.viewportCenterX`);
  await page.panCanvas(35, -20);
  assert.notEqual(await page.evaluate(`document.querySelector('#playground-root').dataset.viewportCenterX`), centerBeforePan);
  const zoomBefore = await page.evaluate(`document.querySelector('#playground-root').dataset.pixelsPerUnit`);
  await page.click('[data-action="zoom-in"]');
  assert.notEqual(await page.evaluate(`document.querySelector('#playground-root').dataset.pixelsPerUnit`), zoomBefore);
  await page.key('z', 'KeyZ', 2);
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.exampleMode`), 'spatial');
  assert.match(await page.evaluate(`document.querySelector('#last-attempt').textContent`), /Undo is unavailable in the read-only spatial view/);
  assert.equal(await page.evaluate(`localStorage.getItem('geosolve.sketch-playground.accepted.v1')`), storageBeforeSpatial);

  await page.click('[data-action="new"]');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.exampleMode`), 'sketch');
  assert.notEqual(await page.evaluate(`getComputedStyle(document.querySelector('[data-tool="point"]')).display`), 'none');
  assert.equal(JSON.parse(await page.exportJson()).points.length, 0);
  assert.notEqual(await page.evaluate(`localStorage.getItem('geosolve.sketch-playground.accepted.v1')`), storageBeforeSpatial);
  page.assertNoErrors();
  console.log('desktop: M19 editable conic and M20 read-only spatial workflows passed');
}

async function m28VisibleTrimSuite(page, mobileSmoke = false) {
  await page.loadExample('m28-trimmed-fillet');
  const canonical = await page.exportJson();
  const document = JSON.parse(canonical);
  assert.equal(document.version, 4);
  assert.equal(document.trim_views.length, 2);
  const line = document.curves.find((curve) => curve.label === 'M28 trimmed line parent');
  const circle = document.curves.find((curve) => curve.label === 'M28 trimmed circle parent');
  const association = document.constraints.find((constraint) => constraint.label === 'M28 trimmed line-circle fillet.association');
  assert.ok(line && circle && association, JSON.stringify({
    curves: document.curves.map((curve) => [curve.label, curve.definition.kind]),
    constraints: document.constraints.map((constraint) => [constraint.label, constraint.definition.kind]),
  }));
  const arcId = association.definition.arc;
  const rendered = await page.evaluate(`(() => {
    const read = (id) => {
      const paths = [...document.querySelectorAll('.playground-curve')].filter((path) => path.dataset.curveId === id);
      return paths.map((path) => ({
        start: Number(path.dataset.visibleStart),
        end: Number(path.dataset.visibleEnd),
        span: path.dataset.spanId,
        deletion: path.dataset.deletePolicy,
      }));
    };
    const markers = [...document.querySelectorAll('[data-derived-trim-marker]')];
    return {
      line: read(${JSON.stringify(line.id)}),
      circle: read(${JSON.stringify(circle.id)}),
      markerCount: markers.length,
      markerPointerEvents: markers.map((marker) => getComputedStyle(marker).pointerEvents),
      markerHandles: markers.filter((marker) => marker.hasAttribute('data-configuration-handle')).length,
      outputHandles: [...document.querySelectorAll('[data-configuration-curve-id]')].filter((handle) => handle.dataset.configurationCurveId === ${JSON.stringify(arcId)}).length,
      trimViews: document.querySelector('#playground-root').dataset.trimViewCount,
      visibleIntervals: document.querySelector('#playground-root').dataset.visibleIntervalCount,
      status: document.querySelector('#playground-solve-status').textContent,
      objects: document.querySelector('#object-list').textContent,
    };
  })()`);
  assert.equal(rendered.line.length, 1);
  assert.equal(rendered.circle.length, 1);
  near(rendered.line[0].start, 0.5, 1e-9);
  near(rendered.line[0].end, 1, 1e-12);
  near(rendered.circle[0].start, -Math.PI, 1e-12);
  near(rendered.circle[0].end, 0, 1e-12);
  assert.equal(rendered.line[0].span, '0');
  assert.equal(rendered.line[0].deletion, 'underlying-curve');
  assert.equal(rendered.markerCount, 2);
  assert.deepEqual(rendered.markerPointerEvents, ['none', 'none']);
  assert.equal(rendered.markerHandles, 0);
  assert.equal(rendered.outputHandles, 0);
  assert.equal(rendered.trimViews, '2');
  assert.equal(rendered.visibleIntervals, '3');
  assert.match(rendered.status, /2 trim view\(s\).*3 visible interval\(s\)/i);
  assert.match(rendered.objects, /fillet owner.*contact/i);
  assert.match(rendered.objects, /deletion targets underlying CurveId/i);

  await page.click('[data-action="clear-selection"]');
  await page.pointerClick(await page.modelClient(1, 1));
  assert.equal(await page.evaluate(`document.querySelector('#selection-summary').textContent`), 'Nothing selected');
  await page.pointerClick(await page.modelClient(4.5, 1));
  assert.match(await page.evaluate(`document.querySelector('#selection-summary').textContent`), /M28 trimmed line parent/);

  if (!mobileSmoke) {
    await page.click('[data-action="clear-selection"]');
    await page.boxSelect([0.8, 0.8], [1.2, 1.2]);
    assert.equal(await page.evaluate(`document.querySelector('#selection-summary').textContent`), 'Nothing selected');
    await page.boxSelect([4, 0.8], [5, 1.2]);
    assert.match(await page.evaluate(`document.querySelector('#selection-summary').textContent`), /M28 trimmed line parent/);

    await page.evaluate(`document.querySelector('#document-json').value = ${JSON.stringify(canonical)}`);
    await page.click('[data-action="import-json"]');
    assert.equal(await page.exportJson(), canonical);
    assert.equal(await page.evaluate(`localStorage.getItem('geosolve.sketch-playground.accepted.v1')`), canonical);
    await page.reload();
    assert.equal(await page.exportJson(), canonical);
    assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.trimViewCount`), '2');

    await page.deleteObject('M28 trimmed line-circle fillet.association');
    const explodedJson = await page.exportJson();
    const exploded = JSON.parse(explodedJson);
    assert.equal(exploded.constraints.some((constraint) => constraint.id === association.id), false);
    assert.equal(exploded.curves.find((curve) => curve.id === arcId)?.definition.kind, 'circular_arc');
    assert.equal(exploded.trim_views.length, 2);
    assert.equal(exploded.trim_views.every((view) => view.start.kind === 'fixed' && view.end.kind === 'fixed'), true);
    const explodedView = await page.evaluate(`(() => ({
      derived: document.querySelectorAll('[data-derived-trim-marker]').length,
      arcHandles: [...document.querySelectorAll('[data-configuration-curve-id]')].filter((handle) => handle.dataset.configurationCurveId === ${JSON.stringify(arcId)}).length,
      lineStart: Number([...document.querySelectorAll('.playground-curve')].find((path) => path.dataset.curveId === ${JSON.stringify(line.id)}).dataset.visibleStart),
      lineEnd: Number([...document.querySelectorAll('.playground-curve')].find((path) => path.dataset.curveId === ${JSON.stringify(line.id)}).dataset.visibleEnd),
    }))()`);
    assert.equal(explodedView.derived, 0);
    assert.equal(explodedView.arcHandles, 2);
    near(explodedView.lineStart, rendered.line[0].start, 1e-12);
    near(explodedView.lineEnd, rendered.line[0].end, 1e-12);
    await page.assertAccepted();
  }
  page.assertNoErrors();
  console.log(`${mobileSmoke ? 'mobile' : 'desktop'}: M28 visible trims, interaction, persistence${mobileSmoke ? '' : ', and explosion'} passed`);
}

const m30Scenarios = [
  ['construction-supporting-offset', 2, 2],
  ['construction-exact-offset', 1, 1],
  ['construction-entity-mirror', 1, 1],
  ['construction-directed-angle', 1, 1],
  ['fillet-line-line-reference', 1, 1],
  ['fillet-line-circle', 1, 1],
  ['fillet-line-bezier', 1, 1],
  ['fillet-nurbs-line', 3, 3],
  ['nurbs-quarter-circle', 4, 4],
  ['nurbs-local-support', 13, 13],
  ['nurbs-periodic', 13, 12],
  ['nurbs-differential', 10, 10],
];

async function assertM30Uat(page, kind, equalityDof, boundedDof) {
  const state = await page.evaluate(`(() => { const root = document.querySelector('#playground-root'); const panel = document.querySelector('#uat-panel'); return { key: root.dataset.exampleKey, expectedEquality: Number(root.dataset.uatEqualityDof), expectedBounded: Number(root.dataset.uatBoundedDof), equality: Number(root.dataset.equalityDof), bounded: Number(root.dataset.boundedDof), hidden: panel.hidden, title: document.querySelector('#uat-title').textContent, instructions: document.querySelector('#uat-instructions').textContent, primaryDrag: document.querySelector('#uat-primary-drag').textContent }; })()`);
  assert.equal(state.key, kind);
  assert.equal(state.expectedEquality, equalityDof);
  assert.equal(state.expectedBounded, boundedDof);
  assert.equal(state.equality, equalityDof);
  assert.equal(state.bounded, boundedDof);
  assert.equal(state.hidden, false);
  assert.ok(state.title.length > 0 && state.instructions.length > 0 && state.primaryDrag.length > 0, JSON.stringify(state));
}

async function m30Drag(page, kind, equalityDof, boundedDof, pointLabel, target, steps = 4) {
  await page.loadExample(kind);
  await assertM30Uat(page, kind, equalityDof, boundedDof);
  const beforeJson = await page.exportJson();
  const before = await page.point(pointLabel);
  assert.ok(before, `missing M30 drag point ${pointLabel}`);
  const destination = typeof target === 'function' ? target(before) : target;
  await page.dragPoint(pointLabel, destination[0], destination[1], steps);
  const after = await page.point(pointLabel);
  assert.ok(after, `lost M30 drag point ${pointLabel}`);
  assert.ok(Math.hypot(after.x - before.x, after.y - before.y) > 1e-4, `${kind} did not move ${pointLabel}`);
  assert.notEqual(await page.exportJson(), beforeJson, `${kind} drag did not change accepted JSON`);
  await page.assertAccepted();
}

async function m30DesktopSuite(page) {
  assert.equal(page.touch, false, 'M30 projected drag suite uses the desktop pointer path');
  assert.deepEqual(
    await page.evaluate(`[...document.querySelectorAll('#alpha-example option')].map((option) => option.value).filter((value) => ${JSON.stringify(m30Scenarios.map(([kind]) => kind))}.includes(value))`),
    m30Scenarios.map(([kind]) => kind),
  );

  await m30Drag(page, 'construction-supporting-offset', 2, 2, 'Supporting offset draggable target end', [3.5, 0]);
  await m30Drag(page, 'construction-exact-offset', 1, 1, 'Exact offset draggable source end', [-3, 3], 8);
  await m30Drag(page, 'construction-entity-mirror', 1, 1, 'Mirror source draggable end', [-2, 1 + Math.sqrt(10)], 8);
  const mirrorCurveCount = JSON.parse(await page.exportJson()).curves.length;
  await page.clickObjectExact('Mirror source line');
  await page.clickObjectExact('Mirror construction axis', true);
  await page.click('[data-action="create-mirror"]');
  assert.equal(JSON.parse(await page.exportJson()).curves.length, mirrorCurveCount + 1);
  await page.assertAccepted();

  const cutAngle = 170 * Math.PI / 180;
  await m30Drag(page, 'construction-directed-angle', 1, 1, 'Directed angle draggable branch-cut tip', [3 * Math.cos(cutAngle), 3 * Math.sin(cutAngle)], 8);
  await page.clickObjectExact('Directed angle reference / branch cut');
  assert.equal(await page.evaluate(`document.querySelector('#dimension-mode').value`), 'Reference');
  await page.setSelect('dimension-mode', 'Driving');
  await page.setSelect('angle-orientation', 'Clockwise');
  await page.setInput('dimension-value', 5 * Math.PI / 180);
  await page.click('[data-action="apply-dimension"]');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.equalityDof`), '0');
  await page.assertAccepted();

  await m30Drag(page, 'fillet-line-line-reference', 1, 1, 'M27 reference-radius untrimmed fillet.center', [-2, 2], 8);
  await page.clickObjectExact('M27 reference-radius untrimmed fillet.association');
  assert.equal(await page.evaluate(`document.querySelector('#fillet-controls').hidden`), false);
  await page.setSelect('fillet-order', 'Second then first');
  await page.setSelect('fillet-sweep', 'Clockwise');
  await page.setSelect('fillet-radius-mode', 'Driving');
  await page.click('[data-action="apply-fillet"]');
  await page.assertAccepted();

  for (const [kind, label, delta, steps] of [
    ['fillet-line-circle', 'Interactive line-circle fillet.center', [0.2, 0.15], 8],
    ['fillet-line-bezier', 'Interactive line-Bezier fillet.center', [0.2, 0.15], 8],
    ['fillet-nurbs-line', 'Interactive NURBS-line fillet.center', [0.04, 0.03], 8],
  ]) {
    const dof = kind === 'fillet-nurbs-line' ? 3 : 1;
    await m30Drag(page, kind, dof, dof, label, (point) => [point.x + delta[0], point.y + delta[1]], steps);
    assert.equal(Number(await page.evaluate(`document.querySelector('#playground-root').dataset.trimViewCount`)), 2);
  }

  await m30Drag(page, 'nurbs-quarter-circle', 4, 4, 'NURBS quarter-circle weight lab control 2', [2.2, 1.8], 8);
  await page.clickObjectExact('NURBS quarter-circle weight lab');
  assert.equal(await page.evaluate(`document.querySelector('#nurbs-controls').hidden`), false);
  const pathBeforeWeight = await page.evaluate(`[...document.querySelectorAll('[data-curve-id="' + document.querySelector('#playground-root').dataset.selectedNurbs + '"]')].map((path) => path.getAttribute('d')).join('|')`);
  await page.setInput('nurbs-weight-value', 0.45);
  await page.click('[data-action="set-nurbs-weight"]');
  const pathAfterWeight = await page.evaluate(`[...document.querySelectorAll('[data-curve-id="' + document.querySelector('#playground-root').dataset.selectedNurbs + '"]')].map((path) => path.getAttribute('d')).join('|')`);
  assert.notEqual(pathAfterWeight, pathBeforeWeight);
  await page.click('[data-action="set-nurbs-gauge"]');
  const pathAfterGauge = await page.evaluate(`[...document.querySelectorAll('[data-curve-id="' + document.querySelector('#playground-root').dataset.selectedNurbs + '"]')].map((path) => path.getAttribute('d')).join('|')`);
  assert.equal(pathAfterGauge, pathAfterWeight);
  await page.assertAccepted();

  await m30Drag(page, 'nurbs-local-support', 13, 13, 'Local-support NURBS control 3', [-1.2, -0.5], 8);
  await page.clickObjectExact('NURBS local-support and knot-insertion lab');
  const localBefore = JSON.parse(await page.exportJson()).curves.find((curve) => curve.label === 'NURBS local-support and knot-insertion lab').definition.controls.length;
  await page.setInput('nurbs-knot', 0.5);
  await page.click('[data-action="insert-nurbs-knot"]');
  const localAfter = JSON.parse(await page.exportJson()).curves.find((curve) => curve.label === 'NURBS local-support and knot-insertion lab').definition.controls.length;
  assert.equal(localAfter, localBefore + 1);
  await page.assertAccepted();

  await m30Drag(page, 'nurbs-periodic', 13, 12, 'Periodic NURBS control 4', [0.8, 2.4], 8);
  await page.clickObjectExact('Periodic NURBS explicit seam contact');
  const periodicBefore = JSON.parse(await page.exportJson()).contacts.find((contact) => contact.label === 'Periodic NURBS explicit seam contact');
  await page.click('[data-action="next-nurbs-span"]');
  const periodicAfter = JSON.parse(await page.exportJson()).contacts.find((contact) => contact.label === 'Periodic NURBS explicit seam contact');
  assert.notDeepEqual([periodicAfter.curve.segment, periodicAfter.winding], [periodicBefore.curve.segment, periodicBefore.winding]);
  await page.assertAccepted();

  await m30Drag(page, 'nurbs-differential', 10, 10, 'NURBS C2 draggable seam', [0.25, 0.2], 8);
  page.assertNoErrors();
  console.log('desktop: M30 construction, fillet, and NURBS projected UAT passed');
}

async function m30MobileSmokeSuite(page) {
  assert.equal(page.touch, true, 'M30 mobile smoke requires touch emulation');
  const listed = await page.evaluate(`[...document.querySelectorAll('#alpha-example option')].map((option) => option.value).filter((value) => ${JSON.stringify(m30Scenarios.map(([kind]) => kind))}.includes(value))`);
  assert.deepEqual(listed, m30Scenarios.map(([kind]) => kind));
  for (const [kind, equalityDof, boundedDof] of m30Scenarios) {
    await page.loadExample(kind);
    await assertM30Uat(page, kind, equalityDof, boundedDof);
    const overflow = await page.evaluate(`document.documentElement.scrollWidth - innerWidth`);
    assert.ok(overflow <= 1, `${kind} caused ${overflow}px mobile overflow`);
  }
  page.assertNoErrors();
  console.log('mobile: all twelve M30 examples load with responsive UAT metadata');
}

const m31Scenarios = [
  ['profile-all-families', 'Complete', 15, 15],
  ['profile-curved-topology', 'Complete', 2, 5],
  ['profile-fillet-trim', 'Complete', 3, 1],
  ['profile-nurbs-self-intersection', 'Complete', 1, 1],
  ['profile-incomplete', 'Truncated', 1, 1],
  ['profile-budget', 'Skipped', 2, 0],
];

const m31Budgets = [
  'render',
  'candidate-pairs',
  'intersection-subdivisions',
  'intersection-roots',
  'fragments',
  'integration-subdivisions',
  'containment-tests',
  'faces',
];

async function assertM31Uat(page, kind, status, families, minimumFaces) {
  const state = await page.evaluate(`(() => {
    const root = document.querySelector('#playground-root');
    return {
      key: root.dataset.exampleKey,
      expectedStatus: root.dataset.uatProfileStatus,
      expectedFamilies: Number(root.dataset.uatProfileFamilyCount),
      expectedMinimumFaces: Number(root.dataset.uatProfileMinimumFaceCount),
      hasFakeEqualityDof: root.hasAttribute('data-uat-equality-dof'),
      hasFakeBoundedDof: root.hasAttribute('data-uat-bounded-dof'),
      nativeStatus: root.dataset.profileStatus,
      nativeScope: root.dataset.profileScope,
      nativeFamilies: Number(root.dataset.profileFamilyCount),
      nativeFaces: Number(root.dataset.profileFaceCount),
      panelHidden: document.querySelector('#uat-panel').hidden,
      metricLabel: document.querySelector('#uat-metric-label').textContent,
      actionLabel: document.querySelector('#uat-action-label').textContent,
      title: document.querySelector('#uat-title').textContent,
      instructions: document.querySelector('#uat-instructions').textContent,
      diagnostics: document.querySelector('#profile-analysis').textContent,
    };
  })()`);
  assert.equal(state.key, kind);
  assert.equal(state.expectedStatus, status);
  assert.equal(state.expectedFamilies, families);
  assert.equal(state.expectedMinimumFaces, minimumFaces);
  assert.equal(state.hasFakeEqualityDof, false);
  assert.equal(state.hasFakeBoundedDof, false);
  assert.equal(state.nativeStatus, status);
  assert.equal(state.nativeScope, 'AllBuiltInPlanarCurves');
  assert.equal(state.nativeFamilies, families);
  assert.ok(state.nativeFaces >= minimumFaces, JSON.stringify(state));
  assert.equal(state.panelHidden, false);
  assert.equal(state.metricLabel, 'Expected profile');
  assert.equal(state.actionLabel, 'Expected families / faces');
  assert.ok(state.title.length > 0 && state.instructions.length > 0);
  assert.match(state.diagnostics, new RegExp(`native status${status}`, 'i'));

  const budgets = await page.evaluate(`(() => {
    const root = document.querySelector('#playground-root');
    return ${JSON.stringify(m31Budgets)}.map((name) => ({
      name,
      consumed: root.getAttribute('data-profile-' + name + '-consumed'),
      limit: root.getAttribute('data-profile-' + name + '-limit'),
    }));
  })()`);
  for (const budget of budgets) {
    assert.notEqual(budget.consumed, null, `${kind} missing ${budget.name} consumed`);
    assert.notEqual(budget.limit, null, `${kind} missing ${budget.name} limit`);
    assert.ok(Number.isFinite(Number(budget.consumed)) && Number(budget.consumed) >= 0, JSON.stringify(budget));
    assert.ok(Number.isFinite(Number(budget.limit)) && Number(budget.limit) >= 0, JSON.stringify(budget));
    assert.ok(Number(budget.consumed) <= Number(budget.limit), `${kind} exceeded ${budget.name}: ${JSON.stringify(budget)}`);
  }
}

async function assertM31NeutralRender(page) {
  const json = await page.exportJson();
  const before = await page.evaluate(`(() => {
    const root = document.querySelector('#playground-root');
    return {
      historyLength: root.dataset.historyLength,
      historyCursor: root.dataset.historyCursor,
      selection: document.querySelector('#selection-summary').textContent,
      primaryAutosave: localStorage.getItem('geosolve.sketch-playground.accepted.v1'),
      backupAutosave: localStorage.getItem('geosolve.sketch-playground.accepted.backup.v1'),
    };
  })()`);
  assert.notEqual(before.selection, 'Nothing selected');
  assert.equal(before.primaryAutosave, json);
  assert.equal(before.backupAutosave, json);
  await page.click('[data-action="zoom-in"]');
  await page.click('[data-action="zoom-out"]');
  assert.equal(await page.exportJson(), json);
  assert.deepEqual(await page.evaluate(`(() => {
    const root = document.querySelector('#playground-root');
    return {
      historyLength: root.dataset.historyLength,
      historyCursor: root.dataset.historyCursor,
      selection: document.querySelector('#selection-summary').textContent,
      primaryAutosave: localStorage.getItem('geosolve.sketch-playground.accepted.v1'),
      backupAutosave: localStorage.getItem('geosolve.sketch-playground.accepted.backup.v1'),
    };
  })()`), before);
}

async function m31DesktopSuite(page) {
  assert.equal(page.touch, false, 'M31 focused inspection uses the desktop page');
  assert.deepEqual(
    await page.evaluate(`[...document.querySelectorAll('#alpha-example option')].map((option) => option.value).filter((value) => ${JSON.stringify(m31Scenarios.map(([kind]) => kind))}.includes(value))`),
    m31Scenarios.map(([kind]) => kind),
  );

  await page.loadExample('profile-all-families');
  await assertM31Uat(page, 'profile-all-families', 'Complete', 15, 15);
  const diagnosticsLayout = await page.evaluate(`(() => {
    const rect = (selector) => { const value = document.querySelector(selector).getBoundingClientRect(); return { top: value.top, bottom: value.bottom, height: value.height }; };
    return {
      panel: rect('.diagnostics-panel'),
      summary: rect('.diagnostics-summary'),
      profile: rect('.profile-analysis-section'),
      audit: rect('.audit-section'),
      persistence: rect('.persistence-section'),
      release: rect('.release-contract'),
    };
  })()`);
  assert.ok(diagnosticsLayout.summary.height >= 190, JSON.stringify(diagnosticsLayout));
  assert.ok(diagnosticsLayout.profile.height >= 380, JSON.stringify(diagnosticsLayout));
  assert.ok(diagnosticsLayout.audit.height >= diagnosticsLayout.summary.height + diagnosticsLayout.profile.height - 2, JSON.stringify(diagnosticsLayout));
  assert.ok(diagnosticsLayout.release.height >= 220, JSON.stringify(diagnosticsLayout));
  assert.ok(diagnosticsLayout.panel.height >= diagnosticsLayout.summary.height + diagnosticsLayout.profile.height + diagnosticsLayout.release.height - 2, JSON.stringify(diagnosticsLayout));
  await page.clickObjectExact('Profile circle');
  await assertM31NeutralRender(page);
  const allFamilies = await page.evaluate(`(() => {
    const paths = [...document.querySelectorAll('.visual-profile-overlay')];
    return {
      paths: paths.length,
      adaptive: paths.some((path) => (path.getAttribute('d').match(/ L /g) || []).length >= 4),
      pointerEvents: paths.map((path) => getComputedStyle(path).pointerEvents),
      dataAttributes: paths.flatMap((path) => path.getAttributeNames().filter((name) => name.startsWith('data-'))),
      renderStatus: document.querySelector('#playground-root').dataset.profileRenderStatus,
      omitted: Number(document.querySelector('#playground-root').dataset.profileOmittedFaceCount),
    };
  })()`);
  assert.ok(allFamilies.paths >= 15, JSON.stringify(allFamilies));
  assert.equal(allFamilies.adaptive, true);
  assert.equal(allFamilies.pointerEvents.every((value) => value === 'none'), true);
  assert.deepEqual(allFamilies.dataAttributes, []);
  assert.equal(allFamilies.renderStatus, 'Complete');
  assert.equal(allFamilies.omitted, 0);

  await page.loadExample('profile-curved-topology');
  await assertM31Uat(page, 'profile-curved-topology', 'Complete', 2, 5);
  const curved = await page.evaluate(`(() => {
    const root = document.querySelector('#playground-root');
    return {
      intersections: Number(root.dataset.profileIntersectionCount),
      faces: Number(root.dataset.profileFaceCount),
      contours: Number(root.dataset.profileContourCount),
      curvedInteriorEvidence: [...document.querySelectorAll('.visual-profile-overlay')].some((path) => (path.getAttribute('d').match(/ L /g) || []).length >= 12),
      holePath: [...document.querySelectorAll('.visual-profile-overlay')].some((path) => (path.getAttribute('d').match(/M /g) || []).length >= 2),
      evenOdd: [...document.querySelectorAll('.visual-profile-overlay')].every((path) => path.getAttribute('fill-rule') === 'evenodd'),
      diagnostics: document.querySelector('#profile-analysis').textContent,
    };
  })()`);
  assert.ok(curved.intersections >= 4, JSON.stringify(curved));
  assert.ok(curved.faces >= 5 && curved.contours > curved.faces, JSON.stringify(curved));
  assert.equal(curved.curvedInteriorEvidence, true);
  assert.equal(curved.holePath, true);
  assert.equal(curved.evenOdd, true);
  assert.match(curved.diagnostics, /CounterClockwise|Clockwise/);

  await page.loadExample('profile-fillet-trim');
  await assertM31Uat(page, 'profile-fillet-trim', 'Complete', 3, 1);
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.profileStatus`), 'Complete');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.profileFamilyCount`), '3');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.profileIssueCount`), '0');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.trimViewCount`), '2');
  const filletDocument = JSON.parse(await page.exportJson());
  const filletLine = filletDocument.curves.find((curve) => curve.label === 'M28 trimmed line parent');
  const filletCircle = filletDocument.curves.find((curve) => curve.label === 'M28 trimmed circle parent');
  const filletOwner = filletDocument.constraints.find((constraint) => constraint.label === 'M28 trimmed line-circle fillet.association');
  const filletWeld = await page.evaluate(`(() => ({
    faces: Number(document.querySelector('#playground-root').dataset.profileFaceCount),
    overlays: document.querySelectorAll('.visual-profile-overlay').length,
    diagnostics: document.querySelector('#profile-analysis').textContent,
  }))()`);
  assert.ok(filletLine && filletCircle && filletOwner, JSON.stringify(filletDocument));
  assert.ok(filletWeld.faces >= 1 && filletWeld.overlays >= 1, JSON.stringify(filletWeld));
  for (const curve of [filletLine.id, filletCircle.id, filletOwner.definition.arc]) {
    assert.match(filletWeld.diagnostics, new RegExp(curve));
  }
  for (const angle of [-Math.PI / 4 - 0.08, -Math.PI / 4 + 0.08]) {
    await page.dragPoint('Profile fillet circle closure', 2 * Math.cos(angle), 2 * Math.sin(angle), 8);
    const movedFillet = await page.evaluate(`(() => { const root = document.querySelector('#playground-root'); return { solver: root.dataset.hardValidity, status: root.dataset.profileStatus, faces: Number(root.dataset.profileFaceCount), issues: Number(root.dataset.profileIssueCount), rendered: Number(root.dataset.profileRenderedFaceCount), omitted: Number(root.dataset.profileOmittedFaceCount), diagnostics: document.querySelector('#profile-analysis').textContent }; })()`);
    assert.equal(movedFillet.solver, 'Valid', JSON.stringify(movedFillet));
    assert.equal(movedFillet.status, 'Complete', JSON.stringify(movedFillet));
    assert.ok(movedFillet.faces >= 1 && movedFillet.rendered >= 1, JSON.stringify(movedFillet));
    assert.equal(movedFillet.issues, 0, JSON.stringify(movedFillet));
    assert.equal(movedFillet.omitted, 0, JSON.stringify(movedFillet));
  }

  await page.loadExample('profile-nurbs-self-intersection');
  await assertM31Uat(page, 'profile-nurbs-self-intersection', 'Complete', 1, 1);
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.profileSelfIntersectionCount`), '1');
  await page.clickObjectExact('Profile self-intersecting NURBS');
  const nurbsEditor = await page.evaluate(`(() => ({ hidden: document.querySelector('#nurbs-controls').hidden, controls: document.querySelectorAll('#nurbs-control option').length, weights: document.querySelectorAll('#nurbs-weight option').length, diagnostics: document.querySelector('#profile-analysis').textContent }))()`);
  assert.equal(nurbsEditor.hidden, false, JSON.stringify(nurbsEditor));
  assert.equal(nurbsEditor.controls, 4, JSON.stringify(nurbsEditor));
  assert.equal(nurbsEditor.weights, 4, JSON.stringify(nurbsEditor));
  assert.match(nurbsEditor.diagnostics, /Root 1 \(self\)/);
  const secondControl = await page.evaluate(`document.querySelectorAll('#nurbs-control option')[1].value`);
  await page.setSelect('nurbs-control', secondControl);
  const controlPosition = await page.evaluate(`({ x: Number(document.querySelector('#nurbs-control-x').value), y: Number(document.querySelector('#nurbs-control-y').value) })`);
  const historyBeforeControlEdit = Number(await page.evaluate(`document.querySelector('#playground-root').dataset.historyCursor`));
  await page.setInput('nurbs-control-x', controlPosition.x + 0.1);
  await page.setInput('nurbs-control-y', controlPosition.y + 0.05);
  await page.click('[data-action="set-nurbs-control"]');
  const editedNurbs = await page.evaluate(`(() => { const root = document.querySelector('#playground-root'); return { history: Number(root.dataset.historyCursor), solver: root.dataset.hardValidity, status: root.dataset.profileStatus, selfRoots: Number(root.dataset.profileSelfIntersectionCount), faces: Number(root.dataset.profileFaceCount), diagnostics: document.querySelector('#profile-analysis').textContent }; })()`);
  assert.equal(editedNurbs.history, historyBeforeControlEdit + 1, JSON.stringify(editedNurbs));
  assert.equal(editedNurbs.solver, 'Valid', JSON.stringify(editedNurbs));
  assert.equal(editedNurbs.status, 'Complete', JSON.stringify(editedNurbs));
  assert.equal(editedNurbs.selfRoots, 1, JSON.stringify(editedNurbs));
  assert.ok(editedNurbs.faces >= 1, JSON.stringify(editedNurbs));
  assert.match(editedNurbs.diagnostics, /Root 1 \(self\)/);

  await page.loadExample('profile-incomplete');
  await assertM31Uat(page, 'profile-incomplete', 'Truncated', 1, 1);
  const incomplete = await page.evaluate(`(() => {
    const root = document.querySelector('#playground-root');
    return {
      status: root.dataset.profileStatus,
      solver: root.dataset.hardValidity,
      rendered: Number(root.dataset.profileRenderedFaceCount),
      issues: Number(root.dataset.profileIssueCount),
      overlays: document.querySelectorAll('.visual-profile-overlay').length,
      diagnostics: document.querySelector('#profile-analysis').textContent,
    };
  })()`);
  assert.equal(incomplete.status, 'Truncated');
  assert.equal(incomplete.solver, 'Valid');
  assert.ok(incomplete.rendered >= 1 && incomplete.overlays >= 1, JSON.stringify(incomplete));
  assert.ok(incomplete.issues >= 1);
  assert.match(incomplete.diagnostics, /TangentIntersection/);

  await page.loadExample('profile-budget');
  await assertM31Uat(page, 'profile-budget', 'Skipped', 2, 0);
  const skipped = await page.evaluate(`(() => {
    const root = document.querySelector('#playground-root');
    return {
      status: root.dataset.profileStatus,
      faces: Number(root.dataset.profileFaceCount),
      overlays: document.querySelectorAll('.visual-profile-overlay').length,
      rootLimit: root.dataset.profileIntersectionRootsLimit,
      diagnostics: document.querySelector('#profile-analysis').textContent,
    };
  })()`);
  assert.equal(skipped.status, 'Skipped');
  assert.equal(skipped.faces, 0);
  assert.equal(skipped.overlays, 0);
  assert.equal(skipped.rootLimit, '0');
  assert.match(skipped.diagnostics, /IntersectionRootBudgetExceeded/);
  const budgetJson = await page.exportJson();
  await page.click('[data-action="copy-scene-capsule"]');
  const capsule = await page.evaluate(`document.querySelector('#document-json').value`);
  assert.match(capsule, /^GEOSOLVE_SCENE_V1\ncodec=lzss12-4-base64url\n/);
  assert.match(capsule, /profile_status=Skipped/);
  assert.match(capsule, /profile_options=[^\n]*,0,/);
  assert.ok(capsule.length < budgetJson.length, `${capsule.length} !< ${budgetJson.length}`);
  await page.loadExample('a1');
  await page.setInput('document-json', capsule);
  await page.click('[data-action="import-json"]');
  const restoredCapsule = await page.evaluate(`(() => { const root = document.querySelector('#playground-root'); return { status: root.dataset.profileStatus, rootLimit: root.dataset.profileIntersectionRootsLimit, hard: root.dataset.hardValidity, attempt: document.querySelector('#last-attempt').textContent }; })()`);
  assert.equal(restoredCapsule.status, 'Skipped', JSON.stringify(restoredCapsule));
  assert.equal(restoredCapsule.rootLimit, '0', JSON.stringify(restoredCapsule));
  assert.equal(restoredCapsule.hard, 'Valid', JSON.stringify(restoredCapsule));
  assert.match(restoredCapsule.attempt, /Compressed scene capsule imported/);
  assert.equal(await page.exportJson(), budgetJson);
  page.assertNoErrors();
  console.log('desktop: M31 all-family profile metadata, diagnostics, overlays, and neutrality passed');
}

async function m31MobileSmokeSuite(page) {
  assert.equal(page.touch, true, 'M31 mobile smoke requires touch emulation');
  for (const [kind, status, families, minimumFaces] of m31Scenarios) {
    await page.loadExample(kind);
    await assertM31Uat(page, kind, status, families, minimumFaces);
    const overflow = await page.evaluate(`document.documentElement.scrollWidth - innerWidth`);
    assert.ok(overflow <= 1, `${kind} caused ${overflow}px mobile overflow`);
  }
  page.assertNoErrors();
  console.log('mobile: all six M31 profile scenes load without horizontal overflow');
}

async function createOffsetThroughInspector(page, {
  example,
  existingLabel,
  sourceLabel,
  targetLabel,
  inspectorKind,
  definitionKind,
  createdLabel,
  expectedDof,
}) {
  await page.loadExample(example);
  const initial = JSON.parse(await page.exportJson());
  const existing = initial.dimensions.find((dimension) => dimension.label === existingLabel);
  const source = initial.curves.find((curve) => curve.label === sourceLabel);
  const target = initial.curves.find((curve) => curve.label === targetLabel);
  assert.ok(existing && source && target, `${example} is missing its public offset operands`);

  await page.deleteObject(existingLabel);
  assert.equal(JSON.parse(await page.exportJson()).dimensions.some((dimension) => dimension.id === existing.id), false);
  await page.clickObjectExact(sourceLabel);
  await page.clickObjectExact(targetLabel, true);
  await page.setSelect('dimension-kind', inspectorKind);
  await page.setSelect('dimension-mode', 'Driving');
  await page.setSelect('offset-side', 'Left');
  await page.setSelect('offset-orientation', 'Same');
  await page.setInput('dimension-value', 2);
  await page.setInput('dimension-label', createdLabel);
  await page.click('[data-action="apply-dimension"]');

  const createdDocument = JSON.parse(await page.exportJson());
  const created = createdDocument.dimensions.find((dimension) => dimension.label === createdLabel);
  assert.ok(created, `${example} did not create ${createdLabel}`);
  assert.equal(created.definition.kind, definitionKind);
  assert.equal(created.definition.source.curve, source.id);
  assert.equal(created.definition.target_segment.curve, target.id);
  assert.equal(created.definition.side, 'left');
  assert.equal(created.definition.orientation, 'same');
  assert.equal(created.mode, 'driving');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.historyCursor`), '2');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.equalityDof`), String(expectedDof));
  await page.assertAccepted();
}

async function m32DesktopSuite(page) {
  assert.equal(page.touch, false, 'M32 interaction and retention coverage is desktop-only');

  await page.loadExample('construction-supporting-offset');
  const resetBaseline = await acceptedBrowserEvidence(page);
  await page.dragPoint('Supporting offset draggable target end', 3.5, 0, 4);
  assert.notEqual((await acceptedBrowserEvidence(page)).canonicalJson, resetBaseline.canonicalJson);
  await page.click('[data-action="reload-example"]');
  assert.deepEqual(await acceptedBrowserEvidence(page), resetBaseline);

  await createOffsetThroughInspector(page, {
    example: 'construction-supporting-offset',
    existingLabel: 'Supporting-line offset / 2 DOF',
    sourceLabel: 'Supporting offset source',
    targetLabel: 'Supporting offset target',
    inspectorKind: 'Supporting offset',
    definitionKind: 'supporting_line_offset',
    createdLabel: 'M32 supporting offset creation',
    expectedDof: 2,
  });
  await createOffsetThroughInspector(page, {
    example: 'construction-exact-offset',
    existingLabel: 'Exact translated-segment offset / 1 rotational DOF',
    sourceLabel: 'Exact offset rotating source',
    targetLabel: 'Exact offset translated target',
    inspectorKind: 'Exact translated offset',
    definitionKind: 'exact_translated_segment_offset',
    createdLabel: 'M32 exact offset creation',
    expectedDof: 1,
  });

  await page.loadExample('nurbs-periodic');
  await page.clickObjectExact('Periodic NURBS explicit seam contact');
  const periodicJson = await page.exportJson();
  const periodicBefore = JSON.parse(periodicJson).contacts.find((contact) => contact.label === 'Periodic NURBS explicit seam contact');
  const seamBefore = await page.evaluate(`(() => { const marker = [...document.querySelectorAll('[data-contact-id]')].find((item) => item.dataset.contactId === ${JSON.stringify(periodicBefore.id)}); return [marker.dataset.modelX, marker.dataset.modelY]; })()`);
  await page.click('[data-action="next-nurbs-span"]');
  const periodicNext = JSON.parse(await page.exportJson()).contacts.find((contact) => contact.id === periodicBefore.id);
  assert.notDeepEqual([periodicNext.curve.segment, periodicNext.winding], [periodicBefore.curve.segment, periodicBefore.winding]);
  await page.click('[data-action="previous-nurbs-span"]');
  const periodicRestored = JSON.parse(await page.exportJson()).contacts.find((contact) => contact.id === periodicBefore.id);
  assert.deepEqual(periodicRestored, periodicBefore);
  assert.equal(await page.exportJson(), periodicJson);
  assert.deepEqual(await page.evaluate(`(() => { const marker = [...document.querySelectorAll('[data-contact-id]')].find((item) => item.dataset.contactId === ${JSON.stringify(periodicBefore.id)}); return [marker.dataset.modelX, marker.dataset.modelY]; })()`), seamBefore);
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.historyCursor`), '2');
  await page.assertAccepted();

  await page.loadExample('fillet-line-bezier');
  await page.clickObjectExact('Interactive line-Bezier fillet.association');
  assert.equal(await page.evaluate(`document.querySelector('#fillet-controls').hidden`), false);
  await page.setSelect('fillet-first-trim', 'Start');
  await page.setSelect('fillet-second-trim', 'End');
  await page.setSelect('fillet-order', 'Second then first');
  await page.setSelect('fillet-sweep', 'Clockwise');
  await page.click('[data-action="apply-fillet"]');
  let filletDocument = JSON.parse(await page.exportJson());
  let fillet = filletDocument.constraints.find((constraint) => constraint.label === 'Interactive line-Bezier fillet.association');
  assert.equal(fillet.definition.first_trim_endpoint, 'start');
  assert.equal(fillet.definition.second_trim_endpoint, 'end');
  assert.equal(fillet.definition.endpoint_order, 'second_then_first');
  assert.equal(filletDocument.curves.find((curve) => curve.id === fillet.definition.arc).definition.sweep, 'clockwise');

  await page.setInput('fillet-radius', 0.8);
  await page.setSelect('fillet-radius-mode', 'Driving');
  await page.click('[data-action="apply-fillet"]');
  filletDocument = JSON.parse(await page.exportJson());
  fillet = filletDocument.constraints.find((constraint) => constraint.id === fillet.id);
  const radiusDimension = filletDocument.dimensions.find((dimension) => dimension.definition.kind === 'radius' && dimension.definition.curve === fillet.definition.arc);
  assert.equal(radiusDimension.mode, 'driving');
  near(filletDocument.scalars.find((scalar) => scalar.id === radiusDimension.definition.target).value, 0.8, 1e-10);
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.trimViewCount`), '2');
  await page.assertAccepted();

  const beforeAggressiveFillet = await acceptedBrowserEvidence(page);
  await page.setInput('fillet-radius', 100);
  await page.click('[data-action="apply-fillet"]');
  await assertAcceptedEvidenceRetained(page, beforeAggressiveFillet, 'aggressive generic fillet radius');
  assert.match(await page.evaluate(`document.querySelector('#last-attempt').textContent`), /rejected|retained|not applied/i);

  await page.loadExample('profile-nurbs-self-intersection');
  await assertM31Uat(page, 'profile-nurbs-self-intersection', 'Complete', 1, 1);
  await page.clickObjectExact('Profile self-intersecting NURBS');
  const profileBefore = JSON.parse(await page.exportJson());
  const profileCurveBefore = profileBefore.curves.find((curve) => curve.label === 'Profile self-intersecting NURBS');
  const editableWeight = profileCurveBefore.definition.weights[1];
  await page.setSelect('nurbs-weight', await page.evaluate(`document.querySelectorAll('#nurbs-weight option')[1].value`));
  await page.setInput('nurbs-weight-value', 0.92);
  await page.click('[data-action="set-nurbs-weight"]');
  const weightedProfile = JSON.parse(await page.exportJson());
  near(weightedProfile.scalars.find((scalar) => scalar.id === editableWeight).value, 0.92, 1e-12);
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.profileStatus`), 'Complete');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.profileSelfIntersectionCount`), '1');

  await page.setInput('nurbs-knot', 0.2);
  await page.click('[data-action="insert-nurbs-knot"]');
  const refinedProfile = JSON.parse(await page.exportJson());
  const profileCurveAfter = refinedProfile.curves.find((curve) => curve.id === profileCurveBefore.id);
  assert.equal(profileCurveAfter.definition.controls.length, profileCurveBefore.definition.controls.length + 1);
  assert.equal(profileCurveAfter.definition.weights.length, profileCurveBefore.definition.weights.length + 1);
  assert.equal(profileCurveAfter.definition.span_ids.length, profileCurveBefore.definition.span_ids.length + 1);
  assert.equal(profileCurveBefore.definition.controls.every((id) => profileCurveAfter.definition.controls.includes(id)), true);
  assert.equal(profileCurveBefore.definition.weights.every((id) => profileCurveAfter.definition.weights.includes(id)), true);
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.profileStatus`), 'Complete');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.profileSelfIntersectionCount`), '1');
  await page.assertAccepted();

  const beforeInvalidWeight = await acceptedBrowserEvidence(page);
  await page.setInput('nurbs-weight-value', 0);
  await page.click('[data-action="set-nurbs-weight"]');
  await assertAcceptedEvidenceRetained(page, beforeInvalidWeight, 'invalid profile NURBS weight');
  assert.match(await page.evaluate(`document.querySelector('#last-attempt').textContent`), /not changed|not applied|positive|retained/i);

  const beforeInvalidKnot = await acceptedBrowserEvidence(page);
  await page.setInput('nurbs-knot', 2);
  await page.click('[data-action="insert-nurbs-knot"]');
  await assertAcceptedEvidenceRetained(page, beforeInvalidKnot, 'invalid profile NURBS knot');
  assert.match(await page.evaluate(`document.querySelector('#last-attempt').textContent`), /not changed|not applied|knot|retained/i);

  await page.loadExample('profile-budget');
  await page.click('[data-action="copy-scene-capsule"]');
  const capsule = await page.evaluate(`document.querySelector('#document-json').value`);
  const retainedCapsuleScene = await acceptedBrowserEvidence(page);
  const corruptCapsule = capsule.replace(/^(checksum=)([0-9a-f])/m, (_match, prefix, digit) => `${prefix}${digit === '0' ? '1' : '0'}`);
  const oversizedCapsule = capsule.replace(/^json_bytes=.*$/m, 'json_bytes=16777217');
  const overBudgetCapsule = capsule.replace(/^profile_options=.*$/m, 'profile_options=1000001,1,1,1,1,1,1,1');
  for (const [label, payload, error] of [
    ['corrupt capsule', corruptCapsule, /checksum mismatch/i],
    ['oversized capsule', oversizedCapsule, /size exceeds|capsule limit/i],
    ['over-budget capsule', overBudgetCapsule, /profile options exceed/i],
  ]) {
    await page.setInput('document-json', payload);
    await page.click('[data-action="import-json"]');
    await assertAcceptedEvidenceRetained(page, retainedCapsuleScene, label);
    assert.match(await page.evaluate(`document.querySelector('#last-attempt').textContent`), error);
  }

  page.assertNoErrors();
  console.log('desktop: M32 reset, creation/edit paths, rejected-operation retention, and capsule limits passed');
}

async function timedM32SceneLoad(page, kind) {
  await page.setSelect('alpha-example', kind);
  await page.setSelect('alpha-scale', '1');
  return page.evaluate(`(async () => {
    const root = document.querySelector('#playground-root');
    const before = Number(root.dataset.renderSequence);
    const started = performance.now();
    document.querySelector('[data-action="load-example"]').click();
    while (Number(root.dataset.renderSequence) <= before) {
      if (performance.now() - started > 30_000) throw new Error('M32 timed render did not complete');
      await new Promise((resolveFrame) => requestAnimationFrame(resolveFrame));
    }
    return {
      elapsed: performance.now() - started,
      before,
      after: Number(root.dataset.renderSequence),
    };
  })()`);
}

async function m32BrowserPerformanceSuite(page) {
  assert.equal(page.touch, false, 'M32 browser performance gates are desktop-only');
  const warmups = 2;
  const measuredSamples = 12;
  const workloads = [
    {
      kind: 'construction-supporting-offset',
      budget: 1_000,
      validate: () => assertM30Uat(page, 'construction-supporting-offset', 2, 2),
    },
    {
      kind: 'nurbs-local-support',
      budget: 2_000,
      validate: () => assertM30Uat(page, 'nurbs-local-support', 13, 13),
    },
    {
      kind: 'profile-all-families',
      budget: 10_000,
      validate: () => assertM31Uat(page, 'profile-all-families', 'Complete', 15, 15),
    },
    {
      kind: 'profile-nurbs-self-intersection',
      budget: 5_000,
      validate: () => assertM31Uat(page, 'profile-nurbs-self-intersection', 'Complete', 1, 1),
    },
  ];

  for (const workload of workloads) {
    const samples = [];
    for (let index = 0; index < warmups + measuredSamples; index++) {
      const sample = await timedM32SceneLoad(page, workload.kind);
      assert.ok(sample.after > sample.before, `${workload.kind} did not complete a timed render`);
      assert.ok(Number.isFinite(sample.elapsed) && sample.elapsed >= 0, JSON.stringify(sample));
      await page.assertAccepted();
      await workload.validate();
      if (index >= warmups) samples.push(sample.elapsed);
    }
    samples.sort((first, second) => first - second);
    const median = (samples[measuredSamples / 2 - 1] + samples[measuredSamples / 2]) / 2;
    const p95 = samples[Math.ceil(measuredSamples * 0.95) - 1];
    console.log(`m32/browser/${workload.kind}: warmups=${warmups} samples=${measuredSamples} median=${median.toFixed(3)}ms p95=${p95.toFixed(3)}ms budget=${workload.budget}ms`);
    assert.ok(p95 <= workload.budget, `${workload.kind} browser p95 ${p95}ms exceeded ${workload.budget}ms`);
  }
  page.assertNoErrors();
}

async function conicCreationSuite(page) {
  assert.equal(page.touch, false, 'complete conic creation suite uses desktop pointer previews');
  await page.click('[data-action="new"]');

  const acceptedSnapshot = () => page.evaluate(`(() => { const root = document.querySelector('#playground-root'); return { revision: root.dataset.authoritativeRevision, historyLength: root.dataset.historyLength, historyCursor: root.dataset.historyCursor, audit: document.querySelector('#playground-audit').innerHTML, storage: localStorage.getItem('geosolve.sketch-playground.accepted.v1') }; })()`);
  const drawConic = async (tool, points, configure) => {
    const historyBefore = Number(await page.evaluate(`document.querySelector('#playground-root').dataset.historyCursor`));
    await page.click(`[data-tool="${tool}"]`);
    assert.notEqual(await page.evaluate(`getComputedStyle(document.querySelector('#conic-options')).display`), 'none');
    await configure();
    for (const point of points.slice(0, -1)) await page.pointerClick(await page.modelClient(...point));
    const beforePreviewJson = await page.exportJson();
    const beforePreview = await acceptedSnapshot();
    await page.hoverModel(...points.at(-1));
    const previewState = await page.evaluate(`(() => ({ exists: document.querySelector('[data-draft-kind="${tool}"]') !== null, controls: document.querySelectorAll('.draft-control').length, status: document.querySelector('#draft-status').textContent, tool: document.querySelector('#sketch-viewport').dataset.tool, last: document.querySelector('#last-attempt').textContent }))()`);
    assert.equal(previewState.exists, true, `missing ${tool} draft preview: ${JSON.stringify(previewState)}`);
    assert.equal(await page.exportJson(), beforePreviewJson);
    assert.deepEqual(await acceptedSnapshot(), beforePreview);
    await page.pointerClick(await page.modelClient(...points.at(-1)));
    assert.equal(Number(await page.evaluate(`document.querySelector('#playground-root').dataset.historyCursor`)), historyBefore + 1);
    assert.equal(await page.evaluate(`document.querySelector('[data-draft-kind]') === null`), true);
    await page.assertAccepted();
  };

  await drawConic('ellipse', [[-5, 3], [-3, 3]], async () => {
    await page.setInput('conic-ratio', 0.5);
  });
  await drawConic('elliptical-arc', [[0, 3], [2, 3]], async () => {
    await page.setInput('conic-ratio', 0.6);
    await page.setInput('conic-arc-start', 0.2);
    await page.setInput('conic-arc-end', 2);
    await page.setSelect('conic-arc-sweep', 'Clockwise');
    assert.equal(await page.evaluate(`document.querySelector('#arc-sweep').value`), 'Clockwise');
  });
  await drawConic('rational-conic', [[3, 2], [4, 4], [5, 2]], async () => {
    await page.setInput('conic-weight', 0.7);
  });
  await drawConic('parabola', [[-4, -2], [-3, -2]], async () => {
    await page.setInput('conic-trim-start', 1.5);
    await page.setInput('conic-trim-end', -1);
  });
  await drawConic('hyperbola', [[1, -4], [3, -4]], async () => {
    await page.setInput('conic-trim-start', 0.8);
    await page.setInput('conic-trim-end', -0.6);
    await page.setInput('conic-semi-conjugate', 1.2);
    await page.setSelect('conic-hyperbola-branch', 'Negative branch');
  });

  const canonical = await page.exportJson();
  const document = JSON.parse(canonical);
  assert.equal(document.points.length, 10);
  assert.equal(document.scalars.length, 10);
  assert.deepEqual(document.curves.map((curve) => curve.definition.kind), [
    'ellipse',
    'elliptical_arc',
    'rational_quadratic_conic',
    'parabola_segment',
    'hyperbola_segment',
  ]);
  const scalar = (id) => document.scalars.find((item) => item.id === id);
  const [ellipse, arc, rational, parabola, hyperbola] = document.curves.map((curve) => curve.definition);
  assert.deepEqual(scalar(ellipse.minor_axis_ratio).domain, { kind: 'bounded', lower: Number.MIN_VALUE, upper: 1 });
  assert.equal(scalar(ellipse.minor_axis_ratio).unit, 'parameter');
  assert.equal(arc.sweep, 'clockwise');
  assert.equal(scalar(arc.start_angle).unit, 'angle');
  assert.equal(scalar(arc.start_angle).value, 0.2);
  assert.equal(scalar(arc.end_angle).value, 2);
  near(rational.weighted_middle[0], 4, 0.03);
  near(rational.weighted_middle[1], 4, 0.03);
  assert.equal(scalar(rational.middle_weight).value, 0.7);
  assert.equal(scalar(rational.middle_weight).domain.kind, 'bounded');
  assert.equal(document.points.some((point) => point.position[0] === rational.weighted_middle[0] && point.position[1] === rational.weighted_middle[1]), false);
  assert.equal(scalar(parabola.trim_start).value, 1.5);
  assert.equal(scalar(parabola.trim_end).value, -1);
  assert.equal(hyperbola.branch, 'negative');
  assert.equal(scalar(hyperbola.semi_conjugate).unit, 'length');
  assert.deepEqual(scalar(hyperbola.semi_conjugate).domain, { kind: 'positive' });
  assert.equal(scalar(hyperbola.trim_start).value, 0.8);
  assert.equal(scalar(hyperbola.trim_end).value, -0.6);
  assert.match(await page.evaluate(`document.querySelector('#sketch-viewport').textContent`), /Q_h homogeneous.*w=7\.000e-1/s);
  assert.equal(await page.evaluate(`localStorage.getItem('geosolve.sketch-playground.accepted.v1')`), canonical);

  await page.click('[data-action="delete"]');
  const deletedJson = await page.exportJson();
  const deleted = JSON.parse(deletedJson);
  assert.equal(deleted.curves.length, 4);
  assert.equal(deleted.points.length, 8);
  assert.equal(deleted.scalars.length, 7);
  await page.key('z', 'KeyZ', 2);
  assert.equal(await page.exportJson(), canonical);
  await page.key('y', 'KeyY', 2);
  assert.equal(await page.exportJson(), deletedJson);
  await page.key('z', 'KeyZ', 2);
  assert.equal(await page.exportJson(), canonical);
  await page.key('z', 'KeyZ', 2);
  assert.equal(JSON.parse(await page.exportJson()).curves.length, 4);
  await page.key('y', 'KeyY', 2);
  assert.equal(await page.exportJson(), canonical);

  await page.click('[data-tool="ellipse"]');
  await page.setInput('conic-ratio', 0);
  const beforeFailureJson = await page.exportJson();
  const beforeFailure = await acceptedSnapshot();
  await page.pointerClick(await page.modelClient(4, -4));
  await page.pointerClick(await page.modelClient(5, -4));
  assert.equal(await page.exportJson(), beforeFailureJson);
  assert.deepEqual(await acceptedSnapshot(), beforeFailure);
  assert.equal(await page.evaluate(`document.querySelectorAll('.draft-control').length`), 2);
  await page.pointerClick(await page.modelClient(6, -4));
  assert.equal(await page.evaluate(`document.querySelectorAll('.draft-control').length`), 2);
  assert.match(await page.evaluate(`document.querySelector('#last-attempt').textContent`), /already full/i);
  await page.setInput('conic-ratio', 'NaN');
  assert.notEqual(await page.evaluate(`getComputedStyle(document.querySelector('#conic-options-error')).display`), 'none');
  assert.match(await page.evaluate(`document.querySelector('#conic-options-error').textContent`), /finite number/i);
  await page.click('[data-action="finish-draft"]');
  assert.equal(await page.exportJson(), beforeFailureJson);
  assert.equal(await page.evaluate(`document.querySelectorAll('.draft-control').length`), 2);
  await page.setInput('conic-ratio', 0.4);
  assert.equal(await page.evaluate(`document.querySelector('[data-draft-kind="ellipse"]') !== null`), true);
  await page.click('[data-action="finish-draft"]');
  assert.equal(Number(await page.evaluate(`document.querySelector('#playground-root').dataset.historyCursor`)), 6);
  assert.equal(JSON.parse(await page.exportJson()).curves.length, 6);

  await page.click('[data-tool="select"]');
  assert.equal(Number(await page.evaluate(`document.querySelectorAll('[data-configuration-handle]').length`)), 7);
  let configurationHistory = 6;
  for (const [label, target] of [
    ['Elliptical arc 2 trim end', [0, 1.8]],
    ['Rational conic 3 Q_h homogeneous coordinate', [4.4, 4.7]],
    ['Parabola 4 trim start', [0, 2]],
    ['Hyperbola 5 trim start', [0, -1]],
  ]) {
    const before = await page.configurationHandle(label);
    await page.dragConfigurationHandle(label, ...target);
    configurationHistory += 1;
    assert.equal(Number(await page.evaluate(`document.querySelector('#playground-root').dataset.historyCursor`)), configurationHistory);
    const after = await page.configurationHandle(label);
    assert.ok(Math.hypot(after.x - before.x, after.y - before.y) > 1e-6, `${label} did not move`);
    await page.assertAccepted();
  }
  const configuredJson = await page.exportJson();
  const configured = JSON.parse(configuredJson);
  const configuredScalar = (id) => configured.scalars.find((item) => item.id === id);
  const configuredArc = configured.curves.find((curve) => curve.label === 'Elliptical arc 2').definition;
  const configuredRational = configured.curves.find((curve) => curve.label === 'Rational conic 3').definition;
  const configuredParabola = configured.curves.find((curve) => curve.label === 'Parabola 4').definition;
  const configuredHyperbola = configured.curves.find((curve) => curve.label === 'Hyperbola 5').definition;
  assert.notEqual(configuredScalar(configuredArc.end_angle).value, 2);
  near(configuredRational.weighted_middle[0], 4.4, 0.03);
  near(configuredRational.weighted_middle[1], 4.7, 0.03);
  assert.notEqual(configuredScalar(configuredParabola.trim_start).value, 1.5);
  assert.notEqual(configuredScalar(configuredHyperbola.trim_start).value, 0.8);
  assert.equal(await page.evaluate(`localStorage.getItem('geosolve.sketch-playground.accepted.v1')`), configuredJson);
  await page.key('z', 'KeyZ', 2);
  assert.notEqual(await page.exportJson(), configuredJson);
  await page.key('y', 'KeyY', 2);
  assert.equal(await page.exportJson(), configuredJson);

  await page.click('[data-action="new"]');
  await page.click('[data-tool="arc"]');
  for (const point of [[0, 0], [2, 0], [0, 2]]) await page.pointerClick(await page.modelClient(...point));
  await page.click('[data-tool="select"]');
  assert.ok(await page.configurationHandle('Arc 1 trim start'));
  assert.ok(await page.configurationHandle('Arc 1 trim end'));
  await page.dragConfigurationHandle('Arc 1 trim end', -2, 0);
  const circular = JSON.parse(await page.exportJson());
  const circularArc = circular.curves[0].definition;
  near(circular.scalars.find((item) => item.id === circularArc.end_angle).value, Math.PI, 1e-9);
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.historyCursor`), '2');
  await page.assertAccepted();
  page.assertNoErrors();
  console.log('desktop: conic creation plus circular/conic trim and homogeneous-handle editing passed');
}

async function mobileConicSuite(page) {
  assert.equal(page.touch, true);
  await page.click('[data-action="new"]');
  await page.click('[data-tool="ellipse"]');
  await page.setInput('conic-ratio', 0.45);
  const narrowLayout = await page.evaluate(`(() => { const panel = document.querySelector('#conic-options').getBoundingClientRect(); const input = document.querySelector('#conic-ratio').getBoundingClientRect(); return { panelWidth: panel.width, viewportWidth: innerWidth, inputHeight: input.height, visible: getComputedStyle(document.querySelector('#conic-options')).display !== 'none' }; })()`);
  assert.equal(narrowLayout.visible, true);
  assert.ok(narrowLayout.panelWidth <= narrowLayout.viewportWidth);
  assert.ok(narrowLayout.inputHeight >= 35);
  await page.pointerClick(await page.modelClient(-2, 2));
  assert.match(await page.evaluate(`document.querySelector('#draft-status').textContent`), /major-axis endpoint/i);
  await page.pointerClick(await page.modelClient(1, 2));
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.historyCursor`), '1');

  await page.click('[data-tool="hyperbola"]');
  await page.setInput('conic-trim-start', -0.75);
  await page.setInput('conic-trim-end', 1.25);
  await page.setInput('conic-semi-conjugate', 0.8);
  await page.setSelect('conic-hyperbola-branch', 'Positive branch');
  await page.pointerClick(await page.modelClient(-2, -2));
  assert.equal(await page.evaluate(`document.querySelectorAll('.draft-control').length`), 1);
  assert.match(await page.evaluate(`document.querySelector('#draft-status').textContent`), /transverse-axis endpoint/i);
  await page.pointerClick(await page.modelClient(0, -2));
  const json = await page.exportJson();
  const document = JSON.parse(json);
  assert.equal(document.curves.length, 2);
  assert.equal(document.curves[0].definition.kind, 'ellipse');
  assert.equal(document.curves[1].definition.kind, 'hyperbola_segment');
  assert.equal(document.curves[1].definition.branch, 'positive');
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.historyCursor`), '2');
  assert.equal(await page.evaluate(`localStorage.getItem('geosolve.sketch-playground.accepted.v1')`), json);
  await page.click('[data-tool="select"]');
  const trimBefore = await page.configurationHandle('Hyperbola 2 trim start');
  assert.ok(trimBefore);
  await page.dragConfigurationHandle('Hyperbola 2 trim start', -1, 0);
  const editedJson = await page.exportJson();
  const edited = JSON.parse(editedJson);
  const editedHyperbola = edited.curves[1].definition;
  assert.notEqual(edited.scalars.find((item) => item.id === editedHyperbola.trim_start).value, -0.75);
  assert.equal(await page.evaluate(`document.querySelector('#playground-root').dataset.historyCursor`), '3');
  assert.equal(await page.evaluate(`localStorage.getItem('geosolve.sketch-playground.accepted.v1')`), editedJson);
  await page.assertAccepted();
  page.assertNoErrors();
  console.log('mobile: ellipse/hyperbola creation and touch trim-handle editing passed');
}

async function historySuite(page) {
  await page.click('[data-action="new"]');
  await page.click('[data-tool="rectangle"]');
  await page.pointerClick(await page.modelClient(0, 0));
  await page.pointerClick(await page.modelClient(4, 3));
  const visualProfile = await page.evaluate(`(() => {
    const overlay = document.querySelector('.visual-profile-overlay');
    return overlay ? {
      count: document.querySelectorAll('.visual-profile-overlay').length,
      pointerEvents: getComputedStyle(overlay).pointerEvents,
      dataAttributes: [...overlay.attributes].filter((attribute) => attribute.name.startsWith('data-')).length,
    } : null;
  })()`);
  assert.deepEqual(visualProfile, { count: 1, pointerEvents: 'none', dataAttributes: 0 });
  await page.click('[data-tool="select"]');
  await page.pointerClick(await page.modelClient(-4, -3));
  const profileClickBefore = {
    json: await page.exportJson(),
    state: await page.evaluate(`(() => { const root = document.querySelector('#playground-root'); return { history: root.dataset.historyCursor, selection: document.querySelector('#selection-summary').textContent }; })()`),
  };
  const profileCenter = await page.modelClient(2, 1.5);
  assert.equal(await page.evaluate(`document.elementFromPoint(${profileCenter.x}, ${profileCenter.y}).classList.contains('visual-profile-overlay')`), false);
  await page.pointerClick(profileCenter);
  assert.deepEqual({
    json: await page.exportJson(),
    state: await page.evaluate(`(() => { const root = document.querySelector('#playground-root'); return { history: root.dataset.historyCursor, selection: document.querySelector('#selection-summary').textContent }; })()`),
  }, profileClickBefore);
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
  cases.push(JSON.stringify({ ...acceptedData, version: 5 }));
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

async function layoutPrioritySuite(page) {
  const layout = await page.evaluate(`(() => {
    const viewport = document.querySelector('#sketch-viewport').getBoundingClientRect();
    const inspector = document.querySelector('.inspector-panel').getBoundingClientRect();
    const diagnostics = document.querySelector('.diagnostics-panel').getBoundingClientRect();
    const header = document.querySelector('.playground-header').getBoundingClientRect();
    const canvas = document.querySelector('.canvas-panel').getBoundingClientRect();
    const badge = document.querySelector('#solve-badge');
    return {
      viewport: { width: viewport.width, height: viewport.height, bottom: viewport.bottom },
      inspector: { width: inspector.width, top: inspector.top, bottom: inspector.bottom },
      diagnosticsTop: diagnostics.top,
      canvasTop: canvas.top,
      headerHeight: header.height,
      badgeInCanvas: badge.closest('.canvas-panel') !== null,
      scrollWidth: document.documentElement.scrollWidth,
      innerWidth,
    };
  })()`);
  assert.equal(layout.badgeInCanvas, true);
  assert.ok(layout.scrollWidth <= layout.innerWidth + 1, JSON.stringify(layout));
  if (page.touch) {
    assert.ok(layout.viewport.width >= layout.innerWidth - 16, JSON.stringify(layout));
    assert.ok(layout.viewport.height >= 240, JSON.stringify(layout));
    assert.ok(layout.diagnosticsTop >= layout.inspector.bottom, JSON.stringify(layout));
    console.log(`mobile/layout: canvas=${layout.viewport.width.toFixed(0)}x${layout.viewport.height.toFixed(0)} no horizontal overflow`);
  } else {
    assert.ok(layout.viewport.width >= layout.inspector.width * 2.4, JSON.stringify(layout));
    assert.ok(layout.viewport.height >= 700, JSON.stringify(layout));
    assert.ok(layout.headerHeight <= 60, JSON.stringify(layout));
    assert.ok(Math.abs(layout.inspector.top - layout.canvasTop) <= 1, JSON.stringify(layout));
    assert.ok(layout.diagnosticsTop >= layout.viewport.bottom, JSON.stringify(layout));
    console.log(`desktop/layout: canvas=${layout.viewport.width.toFixed(0)}x${layout.viewport.height.toFixed(0)} inspector=${layout.inspector.width.toFixed(0)} header=${layout.headerHeight.toFixed(0)}`);
  }
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
  await layoutPrioritySuite(desktop);
  let mobile;
  if (process.env.GEOSOLVE_E2E_M28_ONLY === '1') {
    await m28VisibleTrimSuite(desktop);
    mobile = await openPage(chrome.cdp.socket.url, serving.url, { width: 390, height: 844 }, true);
    await m28VisibleTrimSuite(mobile, true);
  } else if (process.env.GEOSOLVE_E2E_M30_ONLY === '1') {
    await m30DesktopSuite(desktop);
    mobile = await openPage(chrome.cdp.socket.url, serving.url, { width: 390, height: 844 }, true);
    await layoutPrioritySuite(mobile);
    await m30MobileSmokeSuite(mobile);
  } else if (process.env.GEOSOLVE_E2E_M31_ONLY === '1') {
    await m31DesktopSuite(desktop);
    mobile = await openPage(chrome.cdp.socket.url, serving.url, { width: 390, height: 844 }, true);
    await layoutPrioritySuite(mobile);
    await m31MobileSmokeSuite(mobile);
  } else if (process.env.GEOSOLVE_E2E_M32_ONLY === '1') {
    await m32DesktopSuite(desktop);
    await m32BrowserPerformanceSuite(desktop);
  } else if (process.env.GEOSOLVE_E2E_CONICS_ONLY === '1') {
    await conicCreationSuite(desktop);
    mobile = await openPage(chrome.cdp.socket.url, serving.url, { width: 390, height: 844 }, true);
    await layoutPrioritySuite(mobile);
    await mobileConicSuite(mobile);
  } else {
    await scenarioSuite(desktop, 'desktop');
    await conicCreationSuite(desktop);
    await newDomainExampleSuite(desktop);
    await m28VisibleTrimSuite(desktop);
    await m30DesktopSuite(desktop);
    await m31DesktopSuite(desktop);
    await m32DesktopSuite(desktop);
    await m32BrowserPerformanceSuite(desktop);
    await fileSuite(desktop, chrome.cdp, 'desktop');
    await recoverySuite(desktop, 'desktop');
    await branchHistoryRecoverySuite(desktop, 'desktop');
    await renderBudgets(desktop);
    mobile = await openPage(chrome.cdp.socket.url, serving.url, { width: 390, height: 844 }, true);
    await layoutPrioritySuite(mobile);
    await scenarioSuite(mobile, 'mobile');
    await mobileConicSuite(mobile);
    await m28VisibleTrimSuite(mobile, true);
    await m30MobileSmokeSuite(mobile);
    await m31MobileSmokeSuite(mobile);
    await fileSuite(mobile, chrome.cdp, 'mobile');
    await recoverySuite(mobile, 'mobile');
    await branchHistoryRecoverySuite(mobile, 'mobile');
  }
  desktop.cdp.close();
  mobile?.cdp.close();
  const focusedSuite = process.env.GEOSOLVE_E2E_M28_ONLY === '1'
    ? 'M28 focused browser E2E passed'
    : process.env.GEOSOLVE_E2E_M30_ONLY === '1'
      ? 'M30 focused browser E2E passed'
      : process.env.GEOSOLVE_E2E_M31_ONLY === '1'
        ? 'M31 focused browser E2E passed'
        : process.env.GEOSOLVE_E2E_M32_ONLY === '1'
          ? 'M32 focused browser E2E passed'
          : 'M19 focused browser E2E passed';
  console.log(process.env.GEOSOLVE_E2E_M28_ONLY === '1' || process.env.GEOSOLVE_E2E_M30_ONLY === '1' || process.env.GEOSOLVE_E2E_M31_ONLY === '1' || process.env.GEOSOLVE_E2E_M32_ONLY === '1' || process.env.GEOSOLVE_E2E_CONICS_ONLY === '1' ? focusedSuite : 'M14 browser E2E passed');
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
