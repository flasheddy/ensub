import init, { EnsubPlayerWorkspace } from "../pkg/ensub_wasm.js";

let initialized;
export async function initializeWasm() {
  initialized ??= init();
  await initialized;
}

export function createWorkspace(snapshot) {
  return new EnsubPlayerWorkspace(snapshot);
}
