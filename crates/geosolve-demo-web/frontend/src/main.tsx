// SPDX-License-Identifier: GPL-3.0-or-later
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import * as Tooltip from "@radix-ui/react-tooltip";
import App from "./App";
import { WorkerWorkbenchAdapter } from "./lib/worker-workbench-adapter";
import { FolderWorkbenchAdapter } from "./lib/folder-adapter";
import { createBrowserWorkbenchSession } from "./lib/browser-workbench-session";
import { createFolderWorkbenchSession } from "./lib/folder-workbench-session";
import type { WorkbenchSession } from "./lib/workbench-session";
import { MockWorkbenchAdapter } from "./lib/mock-adapter";
import "./styles.css";

async function mount() {
  let session: WorkbenchSession;
  if (new URLSearchParams(location.search).get("collaboration") === "1") {
    const [{CollaborativeWorkbenchAdapter},{acquireCollaborationTabIdentity},{CollaborationPendingStore},{CollaborationRawRecovery},{createCollaborativeWorkbenchSession}]=await Promise.all([
      import("./lib/collaboration-adapter"),import("./lib/collaboration-tab-identity"),import("./lib/collaboration-storage"),import("./lib/collaboration-raw-recovery"),import("./lib/collaborative-workbench-session"),
    ]);
    const baseUrl=new URL("./api/collaboration/",location.href).href;
    const key=`geosolve.collaboration.${baseUrl}`;
    const token=new URLSearchParams(location.hash.slice(1)).get("invite")??sessionStorage.getItem(`${key}.invite`);
    if(!token)throw Error("Open the shared document invitation provided by its host.");
    sessionStorage.setItem(`${key}.invite`,token);
    const params=new URLSearchParams(location.search),resume=params.get("recoverClient");
    const authoringPreview=params.get("authoringPreview")??undefined;
    if(authoringPreview!==undefined&&authoringPreview!=="client"&&authoringPreview!=="server")throw Error("Authoring preview mode must be client or server");
    if(resume){sessionStorage.setItem(`${key}.client`,resume);params.delete("recoverClient");}
    history.replaceState(null,"",location.pathname+"?"+params);
    const identity=await acquireCollaborationTabIdentity({scope:key,...(resume?{mode:"resume" as const}:{})});
    try {
    const pending=await identity.storage.read(),raw=new CollaborationRawRecovery(`${key}.${identity.clientId}.raw`),recovered=raw.read();
    // Keep recovered raw bytes distinct from new typing until explicit export;
    // interpreting old offsets against current shared text would be unsafe.
    const rawOwnerKey=`${key}.${identity.clientId}.raw-owner`,rawOwner=Array.from(crypto.getRandomValues(new Uint8Array(16)),byte=>byte.toString(16).padStart(2,"0")).join("");
    sessionStorage.setItem(rawOwnerKey,rawOwner);
    const activeRaw=new CollaborationRawRecovery(`${key}.${identity.clientId}.raw-active`);
    const previousActive=activeRaw.read();
    const recovery=[...recovered,...previousActive];if(recovery.length)raw.write(recovery);
    activeRaw.write([]);
    const collaboration=new CollaborativeWorkbenchAdapter({baseUrl,inviteToken:token,clientId:identity.clientId,pending,authoringPreview,
      assertOwned:()=>identity.assertOwned(),savePending:checkpoint=>identity.storage.write(checkpoint),saveSourceIntents:intents=>{if(sessionStorage.getItem(rawOwnerKey)!==rawOwner)throw Error("This page no longer owns its source recovery buffer");activeRaw.write(intents);}});
    const shared=collaboration;
    const download=(name:string,value:unknown)=>{
      const url=URL.createObjectURL(new Blob([JSON.stringify(value,null,2)],{type:"application/json"}));
      const anchor=document.createElement("a");anchor.href=url;anchor.download=name;anchor.click();setTimeout(()=>URL.revokeObjectURL(url),1000);
    };
    shared.recoveryActions=[
      ...(recovery.length?[{label:`Export ${recovery.length} recovered source edit${recovery.length===1?"":"s"}`,run:()=>download("geosolve-recovered-source.json",recovery)}]:[]),
      ...identity.retained.flatMap(item=>[
        {label:`Export saved work (${item.clientId.slice(0,8)})`,run:()=>{void CollaborationPendingStore.inspect(key,item.clientId).then(value=>download("geosolve-pending-work.json",value));}},
        {label:`Resume saved editor (${item.clientId.slice(0,8)})`,run:()=>{void(async()=>{shared.dispose();await identity.release();const next=new URL(location.href);next.searchParams.set("recoverClient",item.clientId);location.assign(next);})();}},
      ]),
    ];
    void identity.lost.then(error=>{shared.notice=error.message;shared.dispose();});
    window.addEventListener("pagehide",()=>{shared.dispose();void identity.release();});
    window.addEventListener("pageshow",event=>{if(event.persisted)location.reload();});
    window.addEventListener("beforeunload",event=>{if(shared.pending.length||shared.pendingSourceEdits.length){event.preventDefault();}});
    session=createCollaborativeWorkbenchSession(shared);
    } catch(error) { await identity.release().catch(()=>{}); throw error; }
  } else if (new URLSearchParams(location.search).get("folder") === "1") {
    const token = new URLSearchParams(location.hash.slice(1)).get("token") ?? sessionStorage.getItem("geosolve.folder.token");
    if (!token) throw Error("Open the folder URL printed by the local bridge; its session token is required.");
    sessionStorage.setItem("geosolve.folder.token", token);
    history.replaceState(null, "", location.pathname + location.search);
    const folderSession = new FolderWorkbenchAdapter(token);
    window.addEventListener("pagehide", (event) => { if (!event.persisted) folderSession.dispose(); });
    session = createFolderWorkbenchSession(folderSession);
  } else if (import.meta.env.PROD && import.meta.env.VITE_GEOSOLVE_MOCK !== "1") {
    const worker = new WorkerWorkbenchAdapter();
    session = createBrowserWorkbenchSession(worker);
    window.addEventListener("pagehide", (event) => { if (!event.persisted) worker.dispose(); });
  } else {
    session = createBrowserWorkbenchSession(new MockWorkbenchAdapter());
  }
  createRoot(document.getElementById("root")!).render(<StrictMode><Tooltip.Provider delayDuration={450}><App session={session} /></Tooltip.Provider></StrictMode>);
}

void mount().catch((error: unknown) => {
  const root = document.getElementById("root")!;
  root.innerHTML = `<main class="bootstrap-error"><h1>Workbench unavailable</h1><p></p></main>`;
  root.querySelector("p")!.textContent = error instanceof Error ? error.message : String(error);
});
