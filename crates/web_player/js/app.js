import { createWorkspaceCoordinator } from "./cache.js";
import { loadLexicon } from "./assets.js";
import { createController } from "./controller.js";
import { createLearningClient } from "./learning-client.js";
import { createView } from "./view.js";
import { createWorkspace, EnsubPlayerLearning, initializeWasm } from "./wasm-client.js";

async function boot() {
  const view = createView();
  try {
    await initializeWasm();
    const lexiconBytes = await loadLexicon({
      manifestUrl: new URL("../assets/lexicon-v1.manifest.json", import.meta.url),
      assetUrl: new URL("../assets/lexicon-v1.postcard.gz", import.meta.url),
    });
    const learning = createLearningClient({ LearningClass: EnsubPlayerLearning, lexiconBytes });
    const coordinator = createWorkspaceCoordinator({ workspaceFactory: createWorkspace });
    await coordinator.open();
    const controller = createController({ coordinator, learning, view });
    await controller.start();
    coordinator.listen(async () => {
      await coordinator.reload();
      controller.reloadWorkspace(coordinator.workspace.view());
    });
    addEventListener("online", () => controller.setOnline(true));
    addEventListener("offline", () => controller.setOnline(false));
    if ("serviceWorker" in navigator) navigator.serviceWorker.register("./service-worker.js").catch(() => {});
  } catch (error) {
    document.getElementById("network-state").textContent = "Workspace unavailable";
    document.getElementById("empty-state").querySelector("p").textContent = error?.message ?? "The local player could not start.";
    document.getElementById("demo-button").disabled = true;
  }
}

boot();
