export class SandboxClient {
  static open({
    SandboxClass,
    lexiconBytes,
    storageKey = "ensub.sandbox.v1",
    coordinator,
  }) {
    return new SandboxClient({
      sandbox: new SandboxClass(lexiconBytes, storageKey, !coordinator.writable),
      coordinator,
    });
  }

  constructor({ sandbox, coordinator }) {
    this.sandbox = sandbox;
    this.coordinator = coordinator;
  }

  get writable() {
    return this.coordinator.writable && !(this.sandbox.isReadOnly?.() ?? false);
  }

  get recovery() {
    return this.storageRecovery ?? null;
  }

  async initialize() {
    try {
      const status = this.coordinator.writable
        ? await this.coordinator.run(() => this.sandbox.initializeStorage())
        : this.sandbox.initializeStorage();
      return { status };
    } catch (error) {
      this.storageRecovery = {
        status: "recovery_required",
        message: error?.message ?? "Browser storage migration failed.",
      };
      return this.storageRecovery;
    }
  }

  rawSnapshot() {
    return this.sandbox.rawSnapshot();
  }

  parse(input) {
    return this.sandbox.parse(input);
  }

  dueReviews(input) {
    return this.sandbox.dueReviews(input);
  }

  stats(input) {
    return this.sandbox.stats(input);
  }

  captureParsed(input) {
    return this.coordinator.run(() => this.sandbox.captureParsed(input));
  }

  review(input) {
    return this.coordinator.run(() => this.sandbox.review(input));
  }

  reset() {
    return this.coordinator.run(() => this.sandbox.reset());
  }
}
