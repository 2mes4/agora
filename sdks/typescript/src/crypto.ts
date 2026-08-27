import * as crypto from 'node:crypto';

export interface AgentKeypair {
  signingPublicKey: string;    // Hex-encoded 32-byte Ed25519 public key
  signingPrivateKey: string;   // Hex-encoded Ed25519 private key
  encryptionPublicKey: string; // Hex-encoded 32-byte X25519 public key
  encryptionPrivateKey: string;// Hex-encoded X25519 private key
}

/**
 * Generate a new Ed25519 + X25519 keypair for an agent.
 */
export function generateKeypair(): AgentKeypair {
  const edKeyPair = crypto.generateKeyPairSync('ed25519', {
    publicKeyEncoding: { type: 'spki', format: 'der' },
    privateKeyEncoding: { type: 'pkcs8', format: 'der' },
  });

  const xKeyPair = crypto.generateKeyPairSync('x25519', {
    publicKeyEncoding: { type: 'spki', format: 'der' },
    privateKeyEncoding: { type: 'pkcs8', format: 'der' },
  });

  // Extract raw 32-byte keys from standard DER SPKI/PKCS8 headers
  // Ed25519 public key DER is 44 bytes, raw key is last 32 bytes
  // Ed25519 private key DER is 48 bytes, raw seed is last 32 bytes
  const edPubRaw = edKeyPair.publicKey.subarray(-32);
  const edPrivRaw = edKeyPair.privateKey.subarray(-32);

  const xPubRaw = xKeyPair.publicKey.subarray(-32);
  const xPrivRaw = xKeyPair.privateKey.subarray(-32);

  return {
    signingPublicKey: edPubRaw.toString('hex'),
    signingPrivateKey: edPrivRaw.toString('hex'),
    encryptionPublicKey: xPubRaw.toString('hex'),
    encryptionPrivateKey: xPrivRaw.toString('hex'),
  };
}

/**
 * Sign arbitrary data with an Ed25519 private key (hex or raw).
 */
export function sign(data: Uint8Array | string, privateKeyHex: string): string {
  const dataBuffer = typeof data === 'string' ? Buffer.from(data, 'utf-8') : Buffer.from(data);
  const privKeyBuffer = Buffer.from(privateKeyHex, 'hex');

  // Reconstruct PKCS8 DER for Ed25519
  const prefix = Buffer.from('302e020100300506032b657004220420', 'hex');
  const pkcs8Der = Buffer.concat([prefix, privKeyBuffer]);

  const privateKey = crypto.createPrivateKey({
    key: pkcs8Der,
    format: 'der',
    type: 'pkcs8',
  });

  const signature = crypto.sign(null, dataBuffer, privateKey);
  return signature.toString('hex');
}

/**
 * Verify an Ed25519 signature against data and public key (hex).
 */
export function verifySignature(
  data: Uint8Array | string,
  signatureHex: string,
  publicKeyHex: string
): boolean {
  try {
    const dataBuffer = typeof data === 'string' ? Buffer.from(data, 'utf-8') : Buffer.from(data);
    const signatureBuffer = Buffer.from(signatureHex, 'hex');
    const pubKeyBuffer = Buffer.from(publicKeyHex, 'hex');

    // Reconstruct SPKI DER for Ed25519
    const prefix = Buffer.from('302a300506032b6570032100', 'hex');
    const spkiDer = Buffer.concat([prefix, pubKeyBuffer]);

    const publicKey = crypto.createPublicKey({
      key: spkiDer,
      format: 'der',
      type: 'spki',
    });

    return crypto.verify(null, dataBuffer, publicKey, signatureBuffer);
  } catch {
    return false;
  }
}
