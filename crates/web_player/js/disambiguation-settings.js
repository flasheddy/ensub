export const DISCLOSURE_VERSION = 1;
export const DISAMBIGUATION_CONFIG_KEY = "ensub.disambiguation.config.v1";
const SESSION_CREDENTIAL_KEY = "ensub.disambiguation.credential.session.v1";
const PERSISTENT_CREDENTIAL_KEY = "ensub.disambiguation.credential.local.v1";
const CONSENT_PREFIX = "ensub.disambiguation.consent.v1.";

export function normalizeProviderEndpoint(value) {
  const endpoint = new URL(value);
  endpoint.hash = "";
  return endpoint.href;
}

function consentKey(adapterId, endpointUrl) {
  return `${CONSENT_PREFIX}${encodeURIComponent(adapterId)}.${encodeURIComponent(normalizeProviderEndpoint(endpointUrl))}`;
}

export function createDisambiguationSettings({
  local = globalThis.localStorage,
  session = globalThis.sessionStorage,
} = {}) {
  return {
    load() {
      try {
        const value = JSON.parse(local.getItem(DISAMBIGUATION_CONFIG_KEY));
        return value?.version === 1 ? value : null;
      } catch {
        return null;
      }
    },
    save({ adapterId, endpointUrl, model, credential, rememberCredential }) {
      const config = {
        version: 1,
        adapterId,
        endpointUrl: normalizeProviderEndpoint(endpointUrl),
        model: model.trim(),
        rememberCredential: Boolean(rememberCredential),
      };
      local.setItem(DISAMBIGUATION_CONFIG_KEY, JSON.stringify(config));
      if (config.rememberCredential) {
        local.setItem(PERSISTENT_CREDENTIAL_KEY, credential);
        session.removeItem(SESSION_CREDENTIAL_KEY);
      } else {
        session.setItem(SESSION_CREDENTIAL_KEY, credential);
        local.removeItem(PERSISTENT_CREDENTIAL_KEY);
      }
      return config;
    },
    credential() {
      const config = this.load();
      return config?.rememberCredential
        ? local.getItem(PERSISTENT_CREDENTIAL_KEY) ?? ""
        : session.getItem(SESSION_CREDENTIAL_KEY) ?? "";
    },
    consentRecord(adapterId, endpointUrl) {
      try {
        return JSON.parse(local.getItem(consentKey(adapterId, endpointUrl)));
      } catch {
        return null;
      }
    },
    hasConsent(adapterId, endpointUrl) {
      return this.consentRecord(adapterId, endpointUrl)?.disclosureVersion === DISCLOSURE_VERSION;
    },
    grantConsent(adapterId, endpointUrl) {
      const record = { disclosureVersion: DISCLOSURE_VERSION };
      local.setItem(consentKey(adapterId, endpointUrl), JSON.stringify(record));
      return record;
    },
  };
}
