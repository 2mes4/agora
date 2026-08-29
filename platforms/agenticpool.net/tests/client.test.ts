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

      if (url.pathname === '/v1/agents/translator-bot' && req.method === 'GET') {
        res.writeHead(200);
        res.end(
          JSON.stringify({
            name: 'translator-bot',
            description: 'Autonomous translation agent',
            services: [
              {
                id: 'translate.text',
                name: 'Text Translator',
                tags: ['translation', 'text'],
                pricing: { amount: 5.0, currency: 'DUCKIES', model: 'per_call' },
              },
            ],
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

      if (url.pathname === '/v1/trust/evaluate' && req.method === 'GET') {
        res.writeHead(200);
        res.end(
          JSON.stringify({
            target: 'translator-bot',
            perspectiveFrom: 'client-agent',
            globalMetrics: {
              score: 85.0,
              gomaTotal: 20,
              plomoTotal: 1.0,
              connections: 5,
              ratio: 0.95,
            },
            personalizedTrust: {
              directInteractions: {
                hasHistory: true,
                gomaLocal: 4,
                plomoLocal: 0.0,
                localScore: 4.0,
                killSwitchActive: false,
              },
              networkVouching: {
                trustedPeersCount: 2,
                samplePeers: ['peer1', 'peer2'],
                transitiveScore: 10.0,
              },
              credibilityPercent: 95.0,
              verdict: 'trusted',
              killSwitchActive: false,
            },
          })
        );
        return;
      }

      if (url.pathname === '/v1/tasks/task-100/review' && req.method === 'POST') {
        res.writeHead(200);
        res.end(
          JSON.stringify({
            taskId: 'task-100',
            outcome: 'satisfied',
            gomaAwarded: 1,
            plomoAssessed: 0.0,
            edgeUpdated: {
              fromAgent: 'client-agent',
              toAgent: 'translator-bot',
              goma: 1,
              plomo: 0.0,
              recomGoma: 0,
              recomPlomo: 0.0,
              lastInteraction: new Date().toISOString(),
            },
          })
        );
        return;
      }

      if (url.pathname === '/v1/contracts' && req.method === 'POST') {
        res.writeHead(201);
        res.end(
          JSON.stringify({
            id: 'ctr-100',
            version: '1.0',
            parties: { requester: 'client-agent', worker: 'translator-bot' },
            pricing: { servicePriceGduck: 20.0, platformFeeGduck: 0.6, disputeCostGduck: 3.6 },
            execution: {
              serviceId: 'text.translate',
              timeoutSeconds: 300,
              inputPayload: { text: 'Hello' },
              acceptanceCriteria: { prompt: 'Valid translation' },
            },
            disputeTerms: { validationPrompt: 'Verify translation', loserPays: true },
            status: 'proposed',
            createdAt: new Date().toISOString(),
            updatedAt: new Date().toISOString(),
          })
        );
        return;
      }

      if (url.pathname === '/v1/contracts/ctr-100/evaluate' && req.method === 'POST') {
        res.writeHead(200);
        res.end(
          JSON.stringify({
            contractId: 'ctr-100',
            result: 'true',
            rationale: 'Passes acceptance criteria',
            qualityScore: 95.0,
          })
        );
        return;
      }

      if (url.pathname === '/v1/contracts/ctr-100/arbitrate' && req.method === 'POST') {
        res.writeHead(200);
        res.end(
          JSON.stringify({
            contractId: 'ctr-100',
            verdict: 'worker_wins',
            arbitrator: 'referee-node',
            rationale: 'Worker satisfied prompt criteria',
            workerPayoutGduck: 20.0,
            requesterRefundGduck: 0.0,
            disputeFeePaidBy: 'client-agent',
            disputeFeeAmountGduck: 5.0,
            workerPlomoDelta: 0.0,
            requesterPlomoDelta: 1.0,
            recommenderPlomoDelta: 0.0,
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

  test('gets agent card from gateway directory', async () => {
    const agent = await client.getAgent('translator-bot');
    assert.ok(agent);
    assert.equal(agent.name, 'translator-bot');
    assert.equal(agent.services.length, 1);
    assert.equal(agent.services[0].id, 'translate.text');

    const notFound = await client.getAgent('nonexistent-agent');
    assert.equal(notFound, null);
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

  test('evaluates trust graph metrics from agent perspective', async () => {
    const evalRes = await client.evaluateTrust('translator-bot');
    assert.equal(evalRes.target, 'translator-bot');
    assert.equal(evalRes.globalMetrics.gomaTotal, 20);
    assert.equal(evalRes.personalizedTrust.verdict, 'trusted');
    assert.equal(evalRes.personalizedTrust.credibilityPercent, 95.0);
  });

  test('submits task review and updates trust graph via Proof-of-Execution', async () => {
    const reviewRes = await client.reviewTask('task-100', {
      outcome: 'satisfied',
      worker: 'translator-bot',
      feedback: 'Excellent translation',
    });
    assert.equal(reviewRes.taskId, 'task-100');
    assert.equal(reviewRes.outcome, 'satisfied');
    assert.equal(reviewRes.gomaAwarded, 1);
    assert.equal(reviewRes.edgeUpdated.fromAgent, 'client-agent');
    assert.equal(reviewRes.edgeUpdated.toAgent, 'translator-bot');
  });

  test('proposes and evaluates smart contract with prompt acceptance criteria', async () => {
    const contract = await client.proposeContract({
      parties: { requester: 'client-agent', worker: 'translator-bot' },
      pricing: { servicePriceGduck: 20.0, platformFeeGduck: 0.6, disputeCostGduck: 3.6 },
      execution: {
        serviceId: 'text.translate',
        timeoutSeconds: 300,
        inputPayload: { text: 'Hello' },
        acceptanceCriteria: { prompt: 'Valid translation' },
      },
      disputeTerms: { validationPrompt: 'Verify translation', loserPays: true },
    });
    assert.equal(contract.id, 'ctr-100');
    assert.equal(contract.status, 'proposed');

    const evalRes = await client.evaluateContractAcceptance('ctr-100');
    assert.equal(evalRes.contractId, 'ctr-100');
    assert.equal(evalRes.result, 'true');
    assert.equal(evalRes.qualityScore, 95.0);
  });

  test('arbitrates disputed contract enforcing Loser-Pays rule', async () => {
    const settlement = await client.arbitrateContract('ctr-100', {
      arbitrator: 'referee-node',
      verdict: 'worker_wins',
      rationale: 'Worker satisfied prompt criteria',
    });
    assert.equal(settlement.contractId, 'ctr-100');
    assert.equal(settlement.verdict, 'worker_wins');
    assert.equal(settlement.workerPayoutGduck, 20.0);
    assert.equal(settlement.disputeFeePaidBy, 'client-agent');
    assert.equal(settlement.disputeFeeAmountGduck, 5.0);
  });
});
