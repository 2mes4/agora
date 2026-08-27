/**
 * Example: Exposing a TypeScript Agent over A2A.
 * Run with: npx tsx examples/echo-agent.ts
 */

import { expose, ServicePricing } from '../src/index.js';

async function main() {
  const agent = await expose(
    {
      name: 'ts-echo-agent',
      description: 'An example TypeScript agent exposing echo & streaming services',
      version: '0.1.0',
      url: 'http://127.0.0.1:7200',
      skills: [
        {
          id: 'echo.text',
          name: 'Text Echo',
          description: 'Repeats back messages',
        },
      ],
      services: [
        {
          id: 'echo.stream',
          name: 'Streaming Echo Service',
          description: 'Streams back echo chunks with progress',
          tags: ['echo', 'streaming'],
          pricing: {
            amount: 0.0, // Free service
            currency: 'EUR',
            model: 'per_call',
          },
        },
      ],
    },
    async (message, context) => {
      const text = message.parts[0]?.kind === 'text' ? message.parts[0].text : '';
      console.log(`[ts-echo-agent] Received task: ${text}`);

      // Emit intermediate progress
      await context.update({ state: 'working', progress: 50 });

      // Emit an artifact
      await context.emitArtifact({
        name: 'response.txt',
        data: `Echo: ${text}`,
        isFinal: true,
      });

      return `Echo from TS: ${text}`;
    },
    {
      port: 7200,
    }
  );

  console.log(`🚀 Agent listening on ${agent.boundUrl}`);
  console.log(`📋 Agent Card available at ${agent.boundUrl}/.well-known/agent-card.json`);
}

main().catch(console.error);
