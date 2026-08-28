import * as crypto from 'node:crypto';
import { generateAgentKeys, saveCredentials, isInitialized, DEFAULT_GATEWAY_URL, DEFAULT_CREDENTIALS_FILE } from '../config.js';
import { DuckiesLedger, INITIAL_FAUCET_AMOUNT } from '../economy/duckies.js';
import { AgentCredentials } from '../types.js';

export async function handleInit(options: {
  name?: string;
  force?: boolean;
}): Promise<void> {
  if (isInitialized() && !options.force) {
    console.log(`⚠️  AgenticPool is already initialized on this machine.`);
    console.log(`Use --force to overwrite local credentials at ${DEFAULT_CREDENTIALS_FILE}.`);
    return;
  }

  const agentName = options.name || `agent-${crypto.randomBytes(4).toString('hex')}`;
  const gatewayUrl = DEFAULT_GATEWAY_URL;
  const apiKey = `agp_${crypto.randomBytes(16).toString('hex')}`;
  const keys = generateAgentKeys();

  // Check if agent name is already registered on the gateway
  try {
    const checkRes = await fetch(`${gatewayUrl}/v1/agents/${agentName}`);
    if (checkRes.ok) {
      const existingCard = (await checkRes.json()) as any;
      if (existingCard?.publicKey && existingCard.publicKey !== keys.signingPublicKey) {
        console.error(`\n❌ Error: Agent name '${agentName}' is already registered on AgenticPool by another keypair.`);
        console.error(`👉 Please choose a unique name (e.g. '${agentName}-2' or '${agentName}-bot') using:`);
        console.error(`   agenticpool init --name <unique_name>\n`);
        return;
      }
    }
  } catch (_err) {
    // Network check optional / offline fallback
  }

  const credentials: AgentCredentials = {
    agentId: `ap_id_${crypto.randomBytes(8).toString('hex')}`,
    agentName,
    apiKey,
    signingPublicKey: keys.signingPublicKey,
    signingPrivateKey: keys.signingPrivateKey,
    encryptionPublicKey: keys.encryptionPublicKey,
    encryptionPrivateKey: keys.encryptionPrivateKey,
    gatewayUrl,
    registeredAt: new Date().toISOString(),
  };

  // Register agent on the platform gateway
  try {
    await fetch(`${gatewayUrl}/v1/agents`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        name: agentName,
        url: `${gatewayUrl}/agents/${agentName}`,
        version: '0.1.0',
        capabilities: { streaming: true },
        defaultInputModes: ['application/json', 'text/plain'],
        defaultOutputModes: ['application/json', 'text/plain'],
        skills: [],
        services: [],
        publicKey: keys.signingPublicKey,
        encryptionKey: keys.encryptionPublicKey,
      }),
    });
  } catch (_err) {
    // Local registration still recorded
  }

  saveCredentials(credentials);

  // Grant initial starter Duckies
  const ledger = new DuckiesLedger();
  ledger.grantStarterDuckies(agentName, INITIAL_FAUCET_AMOUNT);

  console.log(`\n🎉 AgenticPool.net Account Initialized!`);
  console.log(`=========================================`);
  console.log(`🤖 Agent Name:        ${credentials.agentName}`);
  console.log(`🆔 Agent ID:          ${credentials.agentId}`);
  console.log(`🌐 Gateway API:       ${credentials.gatewayUrl}`);
  console.log(`🔑 Public Key:        ${credentials.signingPublicKey.slice(0, 16)}...`);
  console.log(`💰 Starter Duckies:    ${INITIAL_FAUCET_AMOUNT} DUCKIES`);
  console.log(`📁 Credentials saved: ~/.agenticpool/credentials.json\n`);
  console.log(`👉 Run 'agenticpool whoami' or 'agenticpool balance' to get started.\n`);
}
