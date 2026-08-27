import { test, describe, before, after } from 'node:test';
import assert from 'node:assert/strict';
import * as http from 'node:http';
import { DirectoryClient } from '../src/directory.js';

describe('Directory & Marketplace Client', () => {
  let mockServer: http.Server;
  let gatewayUrl: string;
  let directory: DirectoryClient;

  before(async () => {
    mockServer = http.createServer((req, res) => {
      res.setHeader('Content-Type', 'application/json');
      const url = new URL(req.url || '/', `http://${req.headers.host}`);

      if (url.pathname === '/v1/agents' && req.method === 'GET') {
        res.writeHead(200);
        res.end(JSON.stringify({ agents: [{ name: 'test-agent', url: 'http://test' }] }));
        return;
      }

      if (url.pathname === '/v1/agents/test-agent/heartbeat' && req.method === 'POST') {
        res.writeHead(200);
        res.end(
          JSON.stringify({
            agentName: 'test-agent',
            status: 'online',
            lastSeen: new Date().toISOString(),
            isOnline: true,
          })
        );
        return;
      }

      if (url.pathname === '/v1/services' && req.method === 'GET') {
        res.writeHead(200);
        res.end(
          JSON.stringify({
            services: [
              {
                agentName: 'test-agent',
                agentUrl: 'http://test',
                service: {
                  id: 'test.svc',
                  name: 'Test Service',
                  pricing: { amount: 5.0, currency: 'EUR', model: 'per_call' },
                },
                presence: {
                  agentName: 'test-agent',
                  status: 'online',
                  lastSeen: new Date().toISOString(),
                  isOnline: true,
                },
              },
            ],
          })
        );
        return;
      }

      if (url.pathname === '/v1/services/search' && req.method === 'GET') {
        const q = url.searchParams.get('q');
        res.writeHead(200);
        res.end(
          JSON.stringify({
            engine: 'llull',
            query: q || '',
            page: 1,
            totalHits: 1,
            hits: [
              {
                id: 'test-agent:test.svc',
                score: 1.0,
                agentName: 'test-agent',
                presence: { isOnline: true },
              },
            ],
          })
        );
        return;
      }

      res.writeHead(404);
      res.end('Not Found');
    });

    await new Promise<void>((resolve) => {
      mockServer.listen(0, '127.0.0.1', () => resolve());
    });

    const addr = mockServer.address() as { port: number };
    gatewayUrl = `http://127.0.0.1:${addr.port}`;
    directory = new DirectoryClient({ gatewayUrl });
  });

  after(async () => {
    await new Promise<void>((resolve) => mockServer.close(() => resolve()));
  });

  test('lists registered agents from directory', async () => {
    const agents = await directory.listAgents();
    assert.equal(agents.length, 1);
    assert.equal(agents[0].name, 'test-agent');
  });

  test('sends heartbeat and receives presence', async () => {
    const presence = await directory.heartbeat('test-agent', 'online');
    assert.equal(presence.agentName, 'test-agent');
    assert.equal(presence.isOnline, true);
  });

  test('lists marketplace services and queries Llull search bridge', async () => {
    const services = await directory.listServices();
    assert.equal(services.length, 1);
    assert.equal(services[0].service.id, 'test.svc');

    const searchResp = await directory.searchServices('test', { onlineOnly: true });
    assert.equal(searchResp.engine, 'llull');
    assert.equal(searchResp.totalHits, 1);
    assert.equal(searchResp.hits[0].agentName, 'test-agent');
  });
});
