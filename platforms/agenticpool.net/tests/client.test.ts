import { test, describe, before, after } from 'node:test';
import assert from 'node:assert/strict';
import * as http from 'node:http';
import { AgenticPoolClient } from '../src/client.js';

describe('AgenticPool Client', () => {
  let mockServer: http.Server;
  let gatewayUrl: string;
  let client: AgenticPoolClient;

  before(async () => {
    mockServer = http.createServer((req, res) => {
      res.setHeader('Content-Type', 'application/json');
      const url = new URL(req.url || '/', `http://${req.headers.host}`);

      if (url.pathname === '/v1/agents' && req.method === 'POST') {
        res.writeHead(201);
        res.end(JSON.stringify({ status: 'created' }));
        return;
      }

      if (url.pathname === '/v1/services' && req.method === 'GET') {
        res.writeHead(200);
        res.end(
          JSON.stringify({
            services: [
              {
                agentName: 'translator-bot',
                service: {
                  id: 'translate.text',
                  name: 'Text Translator',
                  pricing: { amount: 5.0, currency: 'DUCKIES', model: 'per_call' },
                },
              },
            ],
          })
        );
        return;
      }

      if (url.pathname === '/a2a/translator-bot' && req.method === 'POST') {
        res.writeHead(200);
        res.end(
          JSON.stringify({
            jsonrpc: '2.0',
            id: 1,
            result: {
              id: 'task-100',
              status: {
                state: 'completed',
                message: { role: 'agent', parts: [{ kind: 'text', text: 'Hello World Translated' }] },
              },
            },
          })
        );
        return;
      }

      res.writeHead(404);
      res.end('Not Found');
    });

    await new Promise<void>((resolve) => mockServer.listen(0, '127.0.0.1', () => resolve()));
    const addr = mockServer.address() as { port: number };
    gatewayUrl = `http://127.0.0.1:${addr.port}`;

    client = new AgenticPoolClient({
      gatewayUrl,
      credentials: {
        agentId: 'test_id',
        agentName: 'client-agent',
        apiKey: 'test_key',
        signingPublicKey: 'abc',
        signingPrivateKey: 'def',
        encryptionPublicKey: '123',
        encryptionPrivateKey: '456',
        gatewayUrl,
        registeredAt: new Date().toISOString(),
      },
    });
  });

  after(async () => {
    await new Promise<void>((resolve) => mockServer.close(() => resolve()));
  });

  test('registers agent with services in Duckies', async () => {
    const res = await client.registerAgent('client-agent', 'Test Agent', [
      {
        id: 'code.review',
        name: 'Code Reviewer',
        priceDuckies: 10.0,
        pricingModel: 'per_call',
        tags: ['code', 'review'],
      },
    ]);
    assert.equal(res.status, 'created');
  });

  test('lists marketplace services', async () => {
    const services = await client.listServices();
    assert.equal(services.length, 1);
    assert.equal(services[0].agentName, 'translator-bot');
    assert.equal(services[0].service.pricing.amount, 5.0);
  });

  test('requests favor from target agent', async () => {
    const resp = await client.requestFavor('translator-bot', 'Translate this text');
    assert.equal(resp.result.id, 'task-100');
    assert.equal(resp.result.status.state, 'completed');
    assert.equal(resp.result.status.message.parts[0].text, 'Hello World Translated');
  });
});
