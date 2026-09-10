// SPDX-License-Identifier: GPL-3.0-or-later
import { afterAll, beforeAll, expect, test } from "vitest";
import { storageBrowserFixture } from "./collaboration-storage.browser-fixture";

let fixture: Awaited<ReturnType<typeof storageBrowserFixture>>;
beforeAll(async () => { fixture = await storageBrowserFixture(); });
afterAll(async () => { await fixture?.close(); });

test("real opened tabs copy sessionStorage but acquire independent fenced identities on insecure HTTP", async () => {
  const first = await fixture.page();
  let second: Awaited<ReturnType<typeof fixture.page>> | undefined;
  try {
    const original = await first.evaluate(async () => {
      const {acquireCollaborationTabIdentity:acquire}=Reflect.get(window,"identityModule") as typeof import("./collaboration-tab-identity");
      const identity=await acquire({scope:"open-tabs",leaseMs:1000,heartbeatMs:200});
      Reflect.set(window,"identity",identity);
      await identity.storage.write({format:"geosolve-client-pending-v1",documentId:"doc",documentEpoch:"epoch",userId:"user",clientId:identity.clientId,
        requests:[{route:"commands",requestId:"original-pending-id",body:{requestId:"original-pending-id",command:{kind:"undo"}}}]});
      return {clientId:identity.clientId,secure:isSecureContext,locks:!!navigator.locks,randomUUID:typeof crypto.randomUUID,indexedDB:!!indexedDB};
    });
    const popup=first.waitForEvent("popup");
    await first.evaluate(()=>{window.open("/","_blank");});
    second=await popup;await second.waitForLoadState("domcontentloaded");
    const duplicate=await second.evaluate(async()=>{
      const copied=sessionStorage.getItem("open-tabs.client");
      const {acquireCollaborationTabIdentity:acquire}=Reflect.get(window,"identityModule") as typeof import("./collaboration-tab-identity");
      const identity=await acquire({scope:"open-tabs",leaseMs:1000,heartbeatMs:200});
      Reflect.set(window,"identity",identity);
      return {copied,clientId:identity.clientId,resumed:identity.resumed,retained:identity.retained,pending:await identity.storage.read()};
    });
    expect(original).toMatchObject({secure:false,locks:false,randomUUID:"undefined",indexedDB:true});
    expect(duplicate.copied).toBe(original.clientId);
    expect(duplicate.clientId).not.toBe(original.clientId);
    expect(duplicate.resumed).toBe(false);
    expect(duplicate.pending).toBeUndefined();
    expect(duplicate.retained).toContainEqual(expect.objectContaining({clientId:original.clientId,pendingRequests:1}));
    const unchanged=await first.evaluate(async()=>{const identity=Reflect.get(window,"identity") as import("./collaboration-tab-identity").CollaborationTabIdentity;await identity.assertOwned();return (await identity.storage.read())?.requests[0].requestId;});
    expect(unchanged).toBe("original-pending-id");
  } finally { await second?.close();await first.close(); }
});

test("genuine reload resumes the exact pending command and text IDs after clean page release", async () => {
  const page=await fixture.page();
  try {
    const original=await page.evaluate(async()=>{
      const {acquireCollaborationTabIdentity:acquire}=Reflect.get(window,"identityModule") as typeof import("./collaboration-tab-identity");
      const identity=await acquire({scope:"reload",leaseMs:1000,heartbeatMs:200});
      const pending={format:"geosolve-client-pending-v1" as const,documentId:"doc",documentEpoch:"epoch",userId:"user",clientId:identity.clientId,
        requests:[{route:"commands" as const,requestId:"same-command-id",body:{requestId:"same-command-id",command:{kind:"undo"}}},
          {route:"text" as const,requestId:"same-text-id",body:{requestId:"same-text-id",changes:[[1,0,255,72]]}}]};
      await identity.storage.write(pending);
      window.addEventListener("pagehide",()=>{void identity.release();});
      return pending;
    });
    await page.reload();
    const recovered=await page.evaluate(async()=>{
      const {acquireCollaborationTabIdentity:acquire}=Reflect.get(window,"identityModule") as typeof import("./collaboration-tab-identity");
      const identity=await acquire({scope:"reload",leaseMs:1000,heartbeatMs:200});
      const result={resumed:identity.resumed,pending:await identity.storage.read()};await identity.release();return result;
    });
    expect(recovered.resumed).toBe(true);
    expect(recovered.pending).toEqual(original);
  } finally { await page.close(); }
});

test("crashed-page recovery preserves exact requests while new tabs never adopt expired sibling work", async () => {
  const first=await fixture.page(),second=await fixture.page();
  try {
    const original=await first.evaluate(async()=>{
      const {acquireCollaborationTabIdentity:acquire}=Reflect.get(window,"identityModule") as typeof import("./collaboration-tab-identity");
      const identity=await acquire({scope:"crash",leaseMs:300,heartbeatMs:80});
      const pending={format:"geosolve-client-pending-v1" as const,documentId:"doc",documentEpoch:"epoch",userId:"user",clientId:identity.clientId,
        requests:[{route:"text" as const,requestId:"crash-lost-ack",body:{requestId:"crash-lost-ack",changes:[[255,0,17]]}}]};
      await identity.storage.write(pending);return pending;
    });
    // No unload handler was installed: this simulates an unfinished release.
    await first.close();
    await second.evaluate(clientId=>{sessionStorage.setItem("crash.client",clientId);},original.clientId);
    const fresh=await second.evaluate(async()=>{
      const {acquireCollaborationTabIdentity:acquire}=Reflect.get(window,"identityModule") as typeof import("./collaboration-tab-identity");
      const identity=await acquire({scope:"crash",leaseMs:300,heartbeatMs:80});
      const result={clientId:identity.clientId,retained:identity.retained};await identity.release();return result;
    });
    expect(fresh.clientId).not.toBe(original.clientId);
    expect(fresh.retained).toContainEqual(expect.objectContaining({clientId:original.clientId,pendingRequests:1}));
    await second.reload();
    const afterReload=await second.evaluate(async originalId=>{
      const {acquireCollaborationTabIdentity:acquire}=Reflect.get(window,"identityModule") as typeof import("./collaboration-tab-identity");
      const current=await acquire({scope:"crash",leaseMs:300,heartbeatMs:80});
      const retained=current.retained;await current.release();
      sessionStorage.setItem("crash.client",originalId);
      const recovered=await acquire({scope:"crash",mode:"resume",leaseMs:300,heartbeatMs:80,resumeTimeoutMs:1500});
      const pending=await recovered.storage.read();await recovered.release();return {retained,pending};
    },original.clientId);
    expect(afterReload.retained).toContainEqual(expect.objectContaining({clientId:original.clientId,pendingRequests:1}));
    expect(afterReload.pending).toEqual(original);
  } finally { if(!first.isClosed())await first.close();await second.close(); }
});

test("active editor cannot be stolen by a copied reload hint and timeout preserves recovery identity", async () => {
  const first=await fixture.page(),second=await fixture.page();
  try {
    const clientId=await first.evaluate(async()=>{
      const {acquireCollaborationTabIdentity:acquire}=Reflect.get(window,"identityModule") as typeof import("./collaboration-tab-identity");
      const identity=await acquire({scope:"busy",leaseMs:600,heartbeatMs:100});Reflect.set(window,"identity",identity);return identity.clientId;
    });
    const result=await second.evaluate(async clientId=>{
      sessionStorage.setItem("busy.client",clientId);
      const {acquireCollaborationTabIdentity:acquire}=Reflect.get(window,"identityModule") as typeof import("./collaboration-tab-identity");
      let refused=false;
      try { await acquire({scope:"busy",mode:"resume",leaseMs:600,heartbeatMs:100,resumeTimeoutMs:200}); } catch(error) { refused=(error as Error).message.includes("previous editor still owns"); }
      return {refused,hint:sessionStorage.getItem("busy.client")};
    },clientId);
    expect(result).toEqual({refused:true,hint:clientId});
    await first.evaluate(async()=>{await (Reflect.get(window,"identity") as import("./collaboration-tab-identity").CollaborationTabIdentity).assertOwned();});
  } finally { await first.close();await second.close(); }
});

test("failed session hint persistence releases the claim and prevents an unrecoverable writable editor", async () => {
  const page=await fixture.page();
  try {
    const result=await page.evaluate(async()=>{
      const {acquireCollaborationTabIdentity:acquire}=Reflect.get(window,"identityModule") as typeof import("./collaboration-tab-identity");
      let refused=false;
      try { await acquire({scope:"hint-failure",mode:"resume",sessionStorage:{getItem:()=>"fixed-client",setItem:()=>{throw Error("Session storage denied");}}}); } catch(error) { refused=(error as Error).message==="Session storage denied"; }
      const next=await acquire({scope:"hint-failure",mode:"resume",sessionStorage:{getItem:()=>"fixed-client",setItem:()=>{}}});
      await next.release();return {refused,recovered:next.clientId};
    });
    expect(result).toEqual({refused:true,recovered:"fixed-client"});
  } finally { await page.close(); }
});
