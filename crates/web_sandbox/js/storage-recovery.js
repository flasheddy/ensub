export function createRawSnapshotBlob(client) {
  const raw = client.rawSnapshot();
  if (typeof raw !== "string") {
    throw new Error("No browser learning snapshot is available.");
  }
  return new Blob([raw], { type: "application/json" });
}

export function downloadRawSnapshot(client, documentRef = document) {
  const blob = createRawSnapshotBlob(client);
  const url = URL.createObjectURL(blob);
  const anchor = documentRef.createElement("a");
  anchor.href = url;
  anchor.download = "ensub-browser-learning-raw.json";
  anchor.click();
  URL.revokeObjectURL(url);
}

export function bindStorageRecovery({ client, initialization, documentRef = document }) {
  if (initialization.status !== "recovery_required") return;
  const banner = documentRef.querySelector("[data-testid=storage-recovery]");
  const message = documentRef.querySelector("[data-testid=storage-recovery-message]");
  const status = documentRef.querySelector("[data-testid=storage-recovery-status]");
  const exportButton = documentRef.querySelector("[data-testid=storage-recovery-export]");
  banner.hidden = false;
  message.textContent = initialization.message;
  documentRef.querySelector("[data-testid=reset]").disabled = true;
  exportButton.addEventListener("click", () => {
    try {
      downloadRawSnapshot(client, documentRef);
      status.textContent = "Raw snapshot exported.";
    } catch (error) {
      status.textContent = error?.message ?? "Raw snapshot export failed.";
    }
  });
}
