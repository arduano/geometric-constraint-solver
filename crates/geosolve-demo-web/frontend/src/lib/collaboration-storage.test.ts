// SPDX-License-Identifier: GPL-3.0-or-later
import { afterAll, beforeAll, expect, test } from "vitest";
import { storageBrowserFixture } from "./collaboration-storage.browser-fixture";

let fixture: Awaited<ReturnType<typeof storageBrowserFixture>>;
beforeAll(async () => { fixture = await storageBrowserFixture(); });
afterAll(async () => { await fixture?.close(); });

test("simultaneous claims from different tabs cannot share a writable outbox", async () => {
  const pages = await Promise.all([fixture.page(), fixture.page()]);
  try {
    const claims = await Promise.all(pages.map((page, index) => page.evaluate(async ownerId => {
      const { CollaborationPendingStore: Store } = Reflect.get(window,"storageModule") as typeof import("./collaboration-storage");
      const claim = await Store.claim({scope:"concurrent",clientId:"client",ownerId,leaseMs:1000});
      if ("store" in claim) Reflect.set(window,"store",claim.store);
      return "store" in claim;
    }, `page-${index}`)));
    expect(claims.filter(Boolean)).toHaveLength(1);
    const winner = pages[claims.indexOf(true)];
    const result = await winner.evaluate(async () => {
      const store = Reflect.get(window,"store") as import("./collaboration-storage").CollaborationPendingStore;
      const checkpoint = {format:"geosolve-client-pending-v1" as const,documentId:"doc",documentEpoch:"epoch",userId:"user",clientId:"client",
        requests:[{route:"commands" as const,requestId:"original-exact-id",body:{requestId:"original-exact-id",command:{kind:"undo"}}}]};
      await store.write(checkpoint);
      return { pending:await store.read(),checkpoint };
    });
    expect(result.pending).toEqual(result.checkpoint);
  } finally { await Promise.all(pages.map(page => page.close())); }
});

test("expired takeover fences stale read, ACK write and release without losing original request bytes", async () => {
  const page = await fixture.page();
  try {
    const result = await page.evaluate(async () => {
      const { CollaborationPendingStore: Store } = Reflect.get(window,"storageModule") as typeof import("./collaboration-storage");
      let lost = 0;
      const first = await Store.claim({scope:"fencing",clientId:"client",ownerId:"a",leaseMs:80,onLost:()=>{lost++;}});
      if (!("store" in first)) throw Error("First claim blocked");
      const original = {format:"geosolve-client-pending-v1" as const,documentId:"doc",documentEpoch:"epoch",userId:"user",clientId:"client",
        requests:[{route:"text" as const,requestId:"exact-lost-ack",body:{requestId:"exact-lost-ack",changes:[[0,255,1,42]],label:"寸法😀"}}]};
      await first.store.write(original);
      await new Promise(resolve=>setTimeout(resolve,100));
      const second = await Store.claim({scope:"fencing",clientId:"client",ownerId:"b",leaseMs:1000});
      if (!("store" in second)) throw Error("Recovery blocked");
      const recovered = await second.store.read();
      const next = {...original,requests:[...original.requests,{route:"text" as const,requestId:"later",body:{requestId:"later",changes:[[7]],label:"later"}}]};
      await second.store.write(next);
      let writeRejected = false, readRejected = false;
      try { await first.store.write({...original,requests:[]}); } catch { writeRejected = true; }
      try { await first.store.read(); } catch { readRejected = true; }
      await first.store.release();
      await second.store.assertOwned();
      return { recovered, original, after:await second.store.read(), next, lost, writeRejected, readRejected };
    });
    expect(result.recovered).toEqual(result.original);
    expect(result.after).toEqual(result.next);
    expect(result).toMatchObject({lost:1,writeRejected:true,readRejected:true});
  } finally { await page.close(); }
});

test("release drains already admitted saves and retains exact work for the next page lifetime", async () => {
  const page = await fixture.page();
  try {
    const result = await page.evaluate(async () => {
      const { CollaborationPendingStore: Store } = Reflect.get(window,"storageModule") as typeof import("./collaboration-storage");
      const first = await Store.claim({scope:"drain",clientId:"client",ownerId:"a",leaseMs:1000});
      if (!("store" in first)) throw Error("Claim blocked");
      const original = {format:"geosolve-client-pending-v1" as const,documentId:"doc",documentEpoch:"epoch",userId:"user",clientId:"client",
        requests:[{route:"commands" as const,requestId:"saved-before-release",body:{requestId:"saved-before-release",command:{kind:"undo"}}}]};
      const save = first.store.write(original), release = first.store.release(), again = first.store.release();
      await Promise.all([save, release, again]);
      const second = await Store.claim({scope:"drain",clientId:"client",ownerId:"b",leaseMs:1000});
      if (!("store" in second)) throw Error("Released claim blocked");
      let refused = false;
      try { await first.store.write({...original,requests:[]}); } catch { refused = true; }
      return { original, recovered:await second.store.read(), refused };
    });
    expect(result.recovered).toEqual(result.original);
    expect(result.refused).toBe(true);
  } finally { await page.close(); }
});

test("foreign checkpoint and aborted native transaction preserve the prior outbox", async () => {
  const page = await fixture.page();
  try {
    const result = await page.evaluate(async () => {
      const { CollaborationPendingStore: Store } = Reflect.get(window,"storageModule") as typeof import("./collaboration-storage");
      const checkpoint = {format:"geosolve-client-pending-v1" as const,documentId:"doc",documentEpoch:"epoch",userId:"user",clientId:"client",
        requests:[{route:"commands" as const,requestId:"retained",body:{requestId:"retained",command:{kind:"undo"}}}]};
      const foreign = await Store.claim({scope:"foreign",clientId:"client",ownerId:"a",leaseMs:1000});
      const abort = await Store.claim({scope:"abort",clientId:"client",ownerId:"a",leaseMs:1000});
      if (!("store" in foreign) || !("store" in abort)) throw Error("Claims blocked");
      await foreign.store.write(checkpoint); await abort.store.write(checkpoint);
      let foreignRefused = false, abortRefused = false;
      try { await foreign.store.write({...checkpoint,userId:"another-user",requests:[]}); } catch { foreignRefused = true; }
      const originalPut = IDBObjectStore.prototype.put;
      IDBObjectStore.prototype.put = function(...args: Parameters<IDBObjectStore["put"]>) {
        if (this.name === "outboxes") throw new DOMException("Injected persistence failure", "QuotaExceededError");
        return originalPut.apply(this,args);
      };
      try { await abort.store.write({...checkpoint,requests:[]}); } catch { abortRefused = true; }
      finally { IDBObjectStore.prototype.put = originalPut; }
      return { foreignRefused, abortRefused, checkpoint, foreign:(await Store.inspect("foreign","client")).pending, aborted:(await Store.inspect("abort","client")).pending };
    });
    expect(result).toMatchObject({foreignRefused:true,abortRefused:true});
    expect(result.foreign).toEqual(result.checkpoint);
    expect(result.aborted).toEqual(result.checkpoint);
  } finally { await page.close(); }
});

test("v1 migration preserves legacy outboxes and refuses malformed work without replacing its bytes", async () => {
  const context = await fixture.browser.newContext(), page = await context.newPage();
  try {
    await page.goto(fixture.url);
    const result = await page.evaluate(async () => {
      const original = {format:"geosolve-client-pending-v1" as const,documentId:"doc",documentEpoch:"epoch",userId:"user",clientId:"legacy",
        requests:[{route:"commands" as const,requestId:"legacy-exact-id",body:{requestId:"legacy-exact-id",command:{kind:"undo"}}}]};
      await new Promise<void>((resolve,reject)=>{
        const request = indexedDB.open("geosolve.collaboration.v1",1);
        request.onupgradeneeded=()=>request.result.createObjectStore("outboxes");
        request.onerror=()=>reject(request.error);
        request.onsuccess=()=>{
          const database=request.result,transaction=database.transaction("outboxes","readwrite"),store=transaction.objectStore("outboxes");
          store.put(original,"legacy-scope.legacy"); store.put({format:"broken",clientId:"broken",requests:[]},"legacy-scope.broken");
          transaction.oncomplete=()=>{database.close();resolve();};transaction.onerror=()=>reject(transaction.error);
        };
      });
      const { CollaborationPendingStore: Store } = Reflect.get(window,"storageModule") as typeof import("./collaboration-storage");
      const claimed=await Store.claim({scope:"legacy-scope",clientId:"legacy",ownerId:"new-page",leaseMs:1000});
      if (!("store" in claimed)) throw Error("Legacy recovery blocked");
      let malformedRefused=false;
      try { await Store.claim({scope:"legacy-scope",clientId:"broken",ownerId:"new-page",leaseMs:1000}); } catch { malformedRefused=true; }
      const raw=await new Promise<unknown>((resolve,reject)=>{
        const request=indexedDB.open("geosolve.collaboration.v1",2);
        request.onsuccess=()=>{const database=request.result,transaction=database.transaction("outboxes"),read=transaction.objectStore("outboxes").get("legacy-scope.broken");transaction.oncomplete=()=>{resolve(read.result);database.close();};};request.onerror=()=>reject(request.error);
      });
      return {original,recovered:await claimed.store.read(),malformedRefused,raw};
    });
    expect(result.recovered).toEqual(result.original);
    expect(result.malformedRefused).toBe(true);
    expect(result.raw).toEqual({format:"broken",clientId:"broken",requests:[]});
  } finally { await context.close(); }
});
