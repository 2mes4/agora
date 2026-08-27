import { test, describe } from 'node:test';
import assert from 'node:assert/strict';
import { generateKeypair, sign, verifySignature } from '../src/crypto.js';

describe('Crypto Module', () => {
  test('generates valid 32-byte Ed25519 & X25519 keypairs', () => {
    const keys = generateKeypair();
    assert.equal(keys.signingPublicKey.length, 64); // 32 bytes hex = 64 chars
    assert.equal(keys.signingPrivateKey.length, 64);
    assert.equal(keys.encryptionPublicKey.length, 64);
    assert.equal(keys.encryptionPrivateKey.length, 64);
  });

  test('signs and verifies Ed25519 signature correctly', () => {
    const keys = generateKeypair();
    const data = 'canonical envelope payload string to sign';

    const signature = sign(data, keys.signingPrivateKey);
    assert.ok(signature.length > 0);

    const valid = verifySignature(data, signature, keys.signingPublicKey);
    assert.equal(valid, true);

    const invalid = verifySignature('tampered data', signature, keys.signingPublicKey);
    assert.equal(invalid, false);
  });
});
