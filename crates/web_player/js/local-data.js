import { LEARNING_STORAGE_KEY } from "./learning-client.js";

export const LOCAL_EXPORT_FORMAT = "ensub-local-export";

function encodeBase64(bytes) {
  let binary = "";
  const chunkSize = 0x8000;
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize));
  }
  return btoa(binary);
}

export function createLocalDataManager({ store, local = globalThis.localStorage, settings }) {
  return {
    async exportValue() {
      const playerBytes = await store.load();
      return {
        format: LOCAL_EXPORT_FORMAT,
        schemaVersion: 1,
        learningSnapshot: local.getItem(LEARNING_STORAGE_KEY),
        playerCacheBase64: encodeBase64(playerBytes),
      };
    },
    async reset() {
      await store.clear();
      local.removeItem(LEARNING_STORAGE_KEY);
      settings.clear();
    },
  };
}

export async function downloadLocalExport(manager, documentRef = document) {
  const value = await manager.exportValue();
  const blob = new Blob([`${JSON.stringify(value, null, 2)}\n`], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const anchor = documentRef.createElement("a");
  anchor.href = url;
  anchor.download = "ensub-local-export-v1.json";
  anchor.click();
  URL.revokeObjectURL(url);
}

function focusable(dialog) {
  return [...dialog.querySelectorAll("button:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex='-1'])")]
    .filter((element) => !element.hidden);
}

export function bindLocalDataControls({ manager, documentRef = document, reload = () => location.reload() }) {
  const button = documentRef.getElementById("local-data-button");
  const dialog = documentRef.getElementById("local-data-dialog");
  const close = documentRef.getElementById("local-data-close");
  const exportButton = documentRef.getElementById("local-data-export");
  const reset = documentRef.getElementById("local-data-reset");
  const confirmReset = documentRef.getElementById("local-data-confirm-reset");
  const confirmation = documentRef.getElementById("local-data-confirmation");
  const status = documentRef.getElementById("local-data-status");
  let returnFocus;

  function hide() {
    dialog.hidden = true;
    confirmation.hidden = true;
    status.textContent = "";
    returnFocus?.focus();
  }
  button.addEventListener("click", () => {
    returnFocus = documentRef.activeElement;
    dialog.hidden = false;
    close.focus();
  });
  close.addEventListener("click", hide);
  exportButton.addEventListener("click", async () => {
    status.textContent = "Preparing export…";
    try {
      await downloadLocalExport(manager, documentRef);
      status.textContent = "Export ready.";
    } catch (error) {
      status.textContent = error?.message ?? "Local data could not be exported.";
    }
  });
  reset.addEventListener("click", () => {
    confirmation.hidden = false;
    confirmReset.focus();
  });
  confirmReset.addEventListener("click", async () => {
    status.textContent = "Resetting local data…";
    try {
      await manager.reset();
      reload();
    } catch (error) {
      status.textContent = error?.message ?? "Local data could not be reset.";
    }
  });
  dialog.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      event.preventDefault();
      hide();
    } else if (event.key === "Tab") {
      const controls = focusable(dialog);
      if (!controls.length) return;
      const first = controls[0];
      const last = controls.at(-1);
      if (event.shiftKey && documentRef.activeElement === first) { last.focus(); event.preventDefault(); }
      else if (!event.shiftKey && documentRef.activeElement === last) { first.focus(); event.preventDefault(); }
    }
  });
}
