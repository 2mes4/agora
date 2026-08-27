import * as http from 'node:http';
import { loadCredentials } from '../config.js';
import { AgenticPoolClient } from '../client.js';
import { DuckiesLedger } from '../economy/duckies.js';

export async function handleServe(options: {
  port?: number;
  serviceId?: string;
  serviceName?: string;
  price?: number;
}): Promise<void> {
  const credentials = loadCredentials();
  const client = new AgenticPoolClient({ credentials });
  const ledger = new DuckiesLedger();

  const port = options.port || 7300;
  const serviceId = options.serviceId || 'generic.favor';
  const serviceName = options.serviceName || 'General Agentic Favor Fulfillment';
  const priceDuckies = options.price || 5.0;

  // 1. Start Local HTTP server for A2A favor requests
  const server = http.createServer(async (req, res) => {
    res.setHeader('Access-Control-Allow-Origin', '*');
    res.setHeader('Content-Type', 'application/json');

    if (req.method === 'GET' && req.url === '/.well-known/agent-card.json') {
      res.writeHead(200);
      res.end(
        JSON.stringify({
          name: credentials.agentName,
          description: `AgenticPool node fulfilling ${serviceName}`,
          version: '0.1.0',
          url: `http://127.0.0.1:${port}`,
          services: [
            {
              id: serviceId,
              name: serviceName,
              pricing: {
                amount: priceDuckies,
                currency: 'DUCKIES',
                model: 'per_call',
              },
            },
          ],
        })
      );
      return;
    }

    if (req.method === 'POST') {
      let body = '';
      req.on('data', (c) => (body += c));
      req.on('end', async () => {
        try {
          const rpc = JSON.parse(body);
          const fromSender = (req.headers['x-agora-sender'] as string) || 'unknown-client';
          const messageText = rpc.params?.message?.parts?.[0]?.text || 'No text';

          console.log(`\n📥 Received favor task from '${fromSender}': "${messageText}"`);

          // Process the favor
          const output = `[${credentials.agentName}] Fulfill favor for '${messageText}'. Completed successfully.`;

          // Record Duckies payout in local ledger
          ledger.settleEscrow(fromSender, credentials.agentName, priceDuckies, serviceId);
          console.log(`💰 Earned ${priceDuckies} DUCKIES from '${fromSender}'!`);

          res.writeHead(200);
          res.end(
            JSON.stringify({
              jsonrpc: '2.0',
              id: rpc.id,
              result: {
                id: `task-${Date.now()}`,
                status: {
                  state: 'completed',
                  message: {
                    role: 'agent',
                    parts: [{ kind: 'text', text: output }],
                  },
                },
              },
            })
          );
        } catch (err: unknown) {
          const msg = err instanceof Error ? err.message : String(err);
          res.writeHead(500);
          res.end(JSON.stringify({ error: msg }));
        }
      });
      return;
    }

    res.writeHead(404);
    res.end(JSON.stringify({ error: 'Not found' }));
  });

  server.listen(port, '0.0.0.0', async () => {
    console.log(`\n🚀 AgenticPool Worker Node Started!`);
    console.log(`=========================================`);
    console.log(`🤖 Agent:         ${credentials.agentName}`);
    console.log(`🌐 Local URL:     http://127.0.0.1:${port}`);
    console.log(`🏷️ Service:       ${serviceName} (${serviceId})`);
    console.log(`💰 Price/Favor:   ${priceDuckies} DUCKIES`);
    console.log(`🌐 Gateway:       ${credentials.gatewayUrl}`);

    // Register with Gateway and start heartbeats
    try {
      await client.registerAgent(
        credentials.agentName,
        `AgenticPool provider for ${serviceName}`,
        [
          {
            id: serviceId,
            name: serviceName,
            priceDuckies,
            pricingModel: 'per_call',
            tags: ['favor', 'agenticpool'],
          },
        ],
        `http://127.0.0.1:${port}`
      );
      console.log(`✅ Registered on AgenticPool directory & marketplace.`);

      // Heartbeat pulse every 30s
      setInterval(() => {
        client.heartbeat(credentials.agentName, 'online').catch(() => {});
      }, 30000);
    } catch (err) {
      console.warn(`⚠️  Gateway connection notice: ${err}`);
    }

    console.log(`\n👂 Listening for incoming favor requests... (Press Ctrl+C to stop)\n`);
  });
}
