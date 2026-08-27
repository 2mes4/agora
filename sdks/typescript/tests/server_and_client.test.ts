import { test, describe, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { expose, ExposedAgent } from '../src/server.js';
import { AgoraClient } from '../src/client.js';
import { A2aEvent, Message, Task } from '../src/types.js';

describe('Server and Client End-to-End', () => {
  let agent: ExposedAgent;
  let serverUrl: string;
  let client: AgoraClient;

  before(async () => {
    // Expose an echo/calculator agent
    agent = await expose(
      {
        name: 'calc-agent',
        description: 'Calculates math formulas',
        version: '0.1.0',
        url: 'http://127.0.0.1:0', // dynamic port
        skills: [
          {
            id: 'math.eval',
            name: 'Math Evaluator',
          },
        ],
        services: [
          {
            id: 'math.add',
            name: 'Addition Service',
            tags: ['math', 'addition'],
            pricing: {
              amount: 0.01,
              currency: 'EUR',
              model: 'per_call',
            },
          },
        ],
      },
      async (message, context) => {
        const text = message.parts[0]?.kind === 'text' ? message.parts[0].text : '';

        if (text === 'stream-me') {
          await context.update({ state: 'working', progress: 50 });
          await context.emitArtifact({
            name: 'calc_output.txt',
            data: 'intermediate calculations',
          });
          return 'streaming calculation finished';
        }

        if (text === 'fail-me') {
          throw new Error('intentional calculation failure');
        }

        return `Processed: ${text}`;
      }
    );

    serverUrl = agent.boundUrl;
    client = new AgoraClient({ gatewayUrl: serverUrl });
  });

  after(async () => {
    await agent.close();
  });

  test('fetches agent card manifest', async () => {
    const card = await client.agentCard(serverUrl);
    assert.equal(card.name, 'calc-agent');
    assert.equal(card.skills?.length, 1);
    assert.equal(card.services?.length, 1);
    assert.equal(card.services[0].id, 'math.add');
    assert.equal(card.services[0].pricing.amount, 0.01);
  });

  test('sends unary delegation request and gets completed task', async () => {
    const task: Task = await client
      .delegate(serverUrl)
      .message('2 + 2 = 4')
      .send();

    assert.equal(task.status.state, 'completed');
    assert.equal(task.status.message?.role, 'agent');
    assert.equal(task.status.message?.parts[0]?.kind, 'text');
    if (task.status.message?.parts[0]?.kind === 'text') {
      assert.equal(task.status.message.parts[0].text, 'Processed: 2 + 2 = 4');
    }
  });

  test('streams lifecycle events via SSE', async () => {
    const stream = client
      .delegate(serverUrl)
      .message('stream-me')
      .stream();

    const events: A2aEvent[] = [];
    for await (const event of stream) {
      events.push(event);
    }

    assert.ok(events.length >= 3);
    assert.equal(events[0].kind, 'task');
    assert.ok(events.some((e) => e.kind === 'artifact-update'));
    assert.ok(events.some((e) => e.kind === 'message' && e.isFinal));
  });

  test('handles task execution errors gracefully', async () => {
    const task = await client
      .delegate(serverUrl)
      .message('fail-me')
      .send();

    assert.equal(task.status.state, 'failed');
    if (task.status.message?.parts[0]?.kind === 'text') {
      assert.ok(task.status.message.parts[0].text.includes('intentional calculation failure'));
    }
  });
});
