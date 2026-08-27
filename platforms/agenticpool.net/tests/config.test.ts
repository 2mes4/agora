import { test, describe, before, after } from 'node:test';
import assert from 'node:assert/strict';
import * as path from 'node:path';
import * as os from 'node:os';
import * as fs from 'node:fs';
import {
  generateAgentKeys,
  saveCredentials,
  loadCredentials,
  isInitialized,
  clearCredentials,
} from '../src/config.js';

describe('Config & Local Credentials', () => {
  const testFile = path.join(os.tmpdir(), `agenticpool_test_creds_${Date.now()}.json`);

  after(() => {
    clearCredentials(testFile);
  });

  test('generates Ed25519 and X25519 keypairs', () => {
    const keys = generateAgentKeys();
    assert.equal(keys.signingPublicKey.length, 64);
    assert.equal(keys.signingPrivateKey.length, 64);
    assert.equal(keys.encryptionPublicKey.length, 64);
    assert.equal(keys.encryptionPrivateKey.length, 64);
  });

  test('saves and loads credentials with strict permissions', () => {
    assert.equal(isInitialized(testFile), false);

    const keys = generateAgentKeys();
    const creds = {
      agentId: 'ap_test_123',
      agentName: 'unit-test-agent',
      apiKey: 'agp_secret_key',
      signingPublicKey: keys.signingPublicKey,
      signingPrivateKey: keys.signingPrivateKey,
      encryptionPublicKey: keys.encryptionPublicKey,
      encryptionPrivateKey: keys.encryptionPrivateKey,
      gatewayUrl: 'https://api.agenticpool.net',
      registeredAt: new Date().toISOString(),
    };

    saveCredentials(creds, testFile);
    assert.equal(isInitialized(testFile), true);

    const loaded = loadCredentials(testFile);
    assert.equal(loaded.agentName, 'unit-test-agent');
    assert.equal(loaded.agentId, 'ap_test_123');
    assert.equal(loaded.signingPublicKey, keys.signingPublicKey);
  });
});
