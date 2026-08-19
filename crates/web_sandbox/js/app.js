import initWasm, { EnsubSandbox } from "../pkg/ensub_wasm.js";

import { loadLexicon } from "./assets.js";
import { createWriterCoordinator } from "./coordinator.js";
import { createController } from "./controller.js";
import { SandboxClient } from "./sandbox-client.js";
import { bindStorageRecovery } from "./storage-recovery.js";
import { createView } from "./view.js";

const STORAGE_KEY = "ensub.sandbox.v1";
const view = createView(document);

async function start() {
  await initWasm(new URL("../pkg/ensub_wasm_bg.wasm", import.meta.url));
  const lexiconBytes = await loadLexicon({
    manifestUrl: new URL("../assets/lexicon-v1.manifest.json", import.meta.url),
    assetUrl: new URL("../assets/lexicon-v1.postcard.gz", import.meta.url),
  });
  const coordinator = createWriterCoordinator({ locks: navigator.locks });
  const client = SandboxClient.open({
    SandboxClass: EnsubSandbox,
    lexiconBytes,
    storageKey: STORAGE_KEY,
    coordinator,
  });
  const storageInitialization = await client.initialize();
  bindStorageRecovery({ client, initialization: storageInitialization });
  const controller = createController({ client, view });
  addEventListener("storage", (event) => {
    if (event.key === STORAGE_KEY) controller.refresh().catch(view.report);
  });
  await controller.refresh();

  if ("serviceWorker" in navigator) {
    await navigator.serviceWorker.register("./service-worker.js", { scope: "./" });
  }
  globalThis.__ensubSandbox = { client, controller };
}

start().catch(view.report);
