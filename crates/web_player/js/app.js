import { createIndexedDbStore, createWorkspaceCoordinator } from "./cache.js";
import { loadLexicon } from "./assets.js";
import { createController } from "./controller.js";
import { createLearningClient, LEARNING_STORAGE_KEY } from "./learning-client.js";
import { createDisambiguationSettings } from "./disambiguation-settings.js";
import { bindLocalDataControls, createLocalDataManager } from "./local-data.js";
import { createView } from "./view.js";
import { createWorkspace, EnsubPlayerLearning, initializeWasm } from "./wasm-client.js";

if ("serviceWorker" in navigator) navigator.serviceWorker.register("./service-worker.js").catch(() => {});

async function boot() {
  const view = createView();
  const store = createIndexedDbStore();
  const disambiguationSettings = createDisambiguationSettings();
  bindLocalDataControls({ manager: createLocalDataManager({ store, settings: disambiguationSettings }) });
  try {
    await initializeWasm();
    const lexiconBytes = await loadLexicon({
      manifestUrl: new URL("../assets/lexicon-v1.manifest.json", import.meta.url),
      assetUrl: new URL("../assets/lexicon-v1.postcard.gz", import.meta.url),
    });
    const learning = createLearningClient({ LearningClass: EnsubPlayerLearning, lexiconBytes });
    const coordinator = createWorkspaceCoordinator({ store, workspaceFactory: createWorkspace });
    await coordinator.open();
    const controller = createController({ coordinator, learning, view, disambiguationSettings });
    await controller.start();
    coordinator.listen(async () => {
      await coordinator.reload();
      controller.reloadWorkspace(coordinator.workspace.view());
    });
    addEventListener("online", () => controller.setOnline(true));
    addEventListener("offline", () => controller.setOnline(false));
    addEventListener("storage", (event) => {
      if (event.key === LEARNING_STORAGE_KEY) controller.refreshDueCount();
    });
  } catch (error) {
    document.getElementById("network-state").textContent = "Workspace unavailable";
    document.getElementById("empty-state").querySelector("p").textContent = error?.message ?? "The local player could not start.";
    document.getElementById("demo-button").disabled = true;
  }
}

boot();
