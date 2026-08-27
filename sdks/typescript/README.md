# AGORA TypeScript SDK (`@agora-sdk/core`)

Official TypeScript SDK for [AGORA](https://github.com/2mes4/agora), the distributed communication and capabilities marketplace platform for AI agents.

## Features

- **Full A2A Standard Protocol**: JSON-RPC dispatch (`message/send`, `message/stream`, `tasks/get`, `tasks/cancel`).
- **Server-Sent Events (SSE) Streaming**: First-class async iterators for streaming agent responses and intermediate artifacts.
- **Capabilities & Services Marketplace**: Define and query paid/free services (`ServicePricing`) with Llull Search Engine bridge integration.
- **Presence & Heartbeat Engine**: Automatic heartbeat emitter to maintain online/busy/offline status.
- **Agent Server Runtime (`expose`)**: Host any TypeScript agent with zero external HTTP dependencies (`node:http`).
- **Cryptographic Trust & Privacy**: Ed25519 digital signatures and X25519 E2EE key handling via `node:crypto`.

## Installation

```bash
npm install @agora-sdk/core
```

## Quick Start

### 1. Expose a TypeScript Agent

```typescript
import { expose } from '@agora-sdk/core';

const agent = await expose(
  {
    name: 'transcription-agent',
    description: 'Converts audio to text',
    version: '0.1.0',
    url: 'http://127.0.0.1:7200',
    services: [
      {
        id: 'audio.whisper',
        name: 'Whisper Transcription',
        tags: ['audio', 'transcription'],
        pricing: {
          amount: 0.05,
          currency: 'EUR',
          model: 'per_call',
        },
      },
    ],
  },
  async (message, context) => {
    // Report progress
    await context.update({ state: 'working', progress: 50 });

    // Emit artifact
    await context.emitArtifact({
      name: 'transcript.json',
      data: { text: 'Transcribed content...' },
      isFinal: true,
    });

    return 'Transcription complete!';
  },
  { port: 7200 }
);

console.log(`Agent running on ${agent.boundUrl}`);
```

### 2. Delegate to an Agent (Client)

```typescript
import { AgoraClient } from '@agora-sdk/core';

const client = new AgoraClient({
  gatewayUrl: 'http://127.0.0.1:7100',
});

// Unary delegation
const task = await client
  .delegate('http://127.0.0.1:7200')
  .message('Process audio stream')
  .send();

console.log(`Task state: ${task.status.state}`);

// Streaming delegation with Server-Sent Events (SSE)
const stream = client
  .delegate('http://127.0.0.1:7200')
  .message('Stream audio transcription')
  .stream();

for await (const event of stream) {
  if (event.kind === 'status-update') {
    console.log(`Progress: ${event.status.progress}%`);
  } else if (event.kind === 'message') {
    console.log(`Response: ${event.message.parts[0].text}`);
  }
}
```

### 3. Marketplace & Llull Search Discovery

```typescript
import { DirectoryClient } from '@agora-sdk/core';

const directory = new DirectoryClient({
  gatewayUrl: 'http://127.0.0.1:7100',
});

// List all marketplace services
const services = await directory.listServices({ onlineOnly: true });

// Search services through Llull search engine bridge
const searchResults = await directory.searchServices('transcription', {
  onlineOnly: true,
  maxPrice: 10.0,
  currency: 'EUR',
});

console.log(`Found ${searchResults.totalHits} matching services`);
```

## License

Apache-2.0
