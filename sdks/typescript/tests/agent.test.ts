import { test, describe, before, after } from 'node:test';
import assert from 'node:assert/strict';
import * as http from 'node:http';
import { Agent } from '../src/agent.js';

describe('Agent Class & Lifecycle', () => {
  let mockGateway: http.Server;
  let gatewayUrl: string;
  let registeredCards: any[] = [];
  let heartbeatsReceived: string[] = [];

  before(async () => {
    mockGateway = http.createServer((req, res) => {
      res.setHeader('Content-Type', 'application/json');
      const url = new URL(req.url || '/', `http://${req.headers.host}`);

      if (url.pathname === '/v1/agents' && req.method === 'POST') {
        let body = '';
        req.on('data', (c) => (body += c));
        req.on('end', () => {
          const card = JSON.parse(body);
          registeredCards.push(card);
          res.writeHead(201);
          res.end(JSON.stringify(card));
        });
        return;
      }

      if (url.pathname.includes('/heartbeat') && req.method === 'POST') {
        const agentName = url.pathname.split('/')[3];
        heartbeatsReceived.push(agentName);
        res.writeHead(200);
        res.end(
          JSON.stringify({
            agentName,
            status: 'online',
            lastSeen: new Date().toISOString(),
            isOnline: true,
          })
        );
        return;
      }

      res.writeHead(404);
      res.end('Not Found');
    });

    await new Promise<void>((resolve) => {
      mockGateway.listen(0, '127.0.0.1', () => resolve());
    });

    const addr = mockGateway.address() as { port: number };
    gatewayUrl = `http://127.0.0.1:${addr.port}`;
  });

  after(async () => {
    await new Promise<void>((resolve) => mockGateway.close(() => resolve()));
  });

  test('creates, registers, runs, and handles tasks on Agent', async () => {
    const agent = new Agent({
      name: 'translator-agent',
      description: 'Translates languages',
      gatewayUrl,
      port: 0,
      skills: [{ id: 'translate', name: 'Translator' }],
      services: [
        {
          id: 'translate.es_to_en',
          name: 'Spanish to English',
          tags: ['translation', 'spanish'],
          pricing: {
            amount: 0.05,
            currency: 'EUR',
            model: 'per_call',
          },
        },
      ],
    });

    agent.onTask(async (message, context) => {
      const text = message.parts[0]?.kind === 'text' ? message.parts[0].text : '';
      return `Translated: ${text}`;
    });

    const boundUrl = await agent.start();
    assert.ok(boundUrl.startsWith('http://127.0.0.1:'));

    // Verify registration on Gateway
    assert.equal(registeredCards.length, 1);
    assert.equal(registeredCards[0].name, 'translator-agent');
    assert.equal(registeredCards[0].services[0].id, 'translate.es_to_en');

    // Verify heartbeat sent
    assert.ok(heartbeatsReceived.includes('translator-agent'));

    // Delegate task directly to the agent's boundUrl
    const task = await agent.delegate(boundUrl).message('Hola mundo').send();
    assert.equal(task.status.state, 'completed');
    if (task.status.message?.parts[0]?.kind === 'text') {
      assert.equal(task.status.message.parts[0].text, 'Translated: Hola mundo');
    }

    await agent.stop();
  });
});
