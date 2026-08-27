/**
 * Example: Delegating tasks to an A2A agent using AgoraClient.
 * Run with: npx tsx examples/delegate-client.ts
 */

import { AgoraClient } from '../src/index.js';

async function main() {
  const client = new AgoraClient({
    gatewayUrl: 'http://127.0.0.1:7100',
    defaultSender: 'ts-client',
  });

  console.log('1. Querying Agent Card...');
  try {
    const card = await client.agentCard('http://127.0.0.1:7200');
    console.log(`Discovered agent: ${card.name} (v${card.version})`);
    console.log(`Services offered: ${card.services?.map((s) => s.name).join(', ')}`);
  } catch (e) {
    console.log('Target agent not running yet on port 7200, continuing...');
  }

  console.log('\n2. Streaming delegation...');
  try {
    const stream = client
      .delegate('http://127.0.0.1:7200')
      .message('Hello from TypeScript SDK!')
      .stream();

    for await (const event of stream) {
      console.log(`[Event ${event.kind}]`, JSON.stringify(event));
    }
  } catch (err: unknown) {
    console.error('Delegation error:', err);
  }
}

main().catch(console.error);
