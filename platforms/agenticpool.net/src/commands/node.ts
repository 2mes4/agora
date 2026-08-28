import * as http from 'node:http';
import { spawn } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';
import { loadCredentials } from '../config.js';

export interface NodeOptions {
  mode?: 'hook' | 'inbox' | 'spawn';
  port?: number;
  webhook?: string;
  runner?: string;
}

export async function handleNode(options: NodeOptions): Promise<void> {
  const credentials = loadCredentials();

  const mode = options.mode || 'hook';
  const port = options.port || 7189;
  const webhookUrl = options.webhook;
  const runnerCmd = options.runner || 'opencode --prompt "{prompt}"';

  const inboxDir = path.join(os.homedir(), '.agenticpool');
  const inboxFile = path.join(inboxDir, 'inbox.json');

  if (!fs.existsSync(inboxDir)) {
    fs.mkdirSync(inboxDir, { recursive: true });
  }

  console.log(`\n🚀 AgenticPool Reactive Node Engine v2.0.0`);
  console.log(`=========================================`);
  console.log(`🤖 Agent:         ${credentials.agentName}`);
  console.log(`🔀 Mode:          ${mode.toUpperCase()}`);
  if (mode === 'hook') {
    if (webhookUrl) {
      console.log(`🔗 Webhook URL:   ${webhookUrl}`);
    } else {
      console.log(`🔌 Hook Port:     ${port}`);
    }
  } else if (mode === 'spawn') {
    console.log(`🏃 Runner Cmd:    ${runnerCmd}`);
  } else if (mode === 'inbox') {
    console.log(`📬 Inbox File:    ${inboxFile}`);
  }
  console.log(`🌐 Gateway:       ${credentials.gatewayUrl}`);

  // Create Local HTTP Receiver for A2A Messages
  const server = http.createServer(async (req, res) => {
    res.setHeader('Access-Control-Allow-Origin', '*');
    res.setHeader('Content-Type', 'application/json');

    if (req.method === 'POST') {
      let body = '';
      req.on('data', (c) => (body += c));
      req.on('end', async () => {
        try {
          const rpc = JSON.parse(body);
          const fromSender = (req.headers['x-agora-sender'] as string) || 'unknown-peer';
          const messageText = rpc.params?.message?.parts?.[0]?.text || '';
          const taskId = rpc.params?.id || `task-${Date.now()}`;

          console.log(`\n📥 [A2A Inbound] From '${fromSender}' (Task: ${taskId}): "${messageText}"`);

          let resultText = '';

          if (mode === 'hook') {
            if (webhookUrl) {
              // Forward to full Webhook URL
              const forwardRes = await fetch(webhookUrl, {
                method: 'POST',
                headers: {
                  'Content-Type': 'application/json',
                  'x-agenticpool-sender': fromSender,
                  'x-agenticpool-task-id': taskId,
                },
                body: JSON.stringify({
                  sender: fromSender,
                  taskId,
                  prompt: messageText,
                  params: rpc.params,
                }),
              });
              const forwardJson = await forwardRes.json() as { output?: string; result?: string; message?: string };
              resultText = forwardJson.output || forwardJson.result || forwardJson.message || JSON.stringify(forwardJson);
            } else {
              resultText = `[${credentials.agentName}] Hook received task '${taskId}'. Output processed via session listener.`;
            }
          } else if (mode === 'inbox') {
            // Save to local inbox for asynchronous pull
            let currentInbox: Array<{ id: string; sender: string; text: string; timestamp: string; status: string }> = [];
            if (fs.existsSync(inboxFile)) {
              try {
                currentInbox = JSON.parse(fs.readFileSync(inboxFile, 'utf8'));
              } catch {
                currentInbox = [];
              }
            }
            currentInbox.push({
              id: taskId,
              sender: fromSender,
              text: messageText,
              timestamp: new Date().toISOString(),
              status: 'pending',
            });
            fs.writeFileSync(inboxFile, JSON.stringify(currentInbox, null, 2), { mode: 0o600 });
            resultText = `[${credentials.agentName}] Favor queued in local inbox. Task ID: ${taskId}`;
            console.log(`📬 Stored in mailbox. View with 'npx agenticpool inbox list'`);
          } else if (mode === 'spawn') {
            // Spawn headless process
            console.log(`⚡ Spawning runner: ${runnerCmd}`);
            const cmdToExec = runnerCmd.replace('{prompt}', messageText.replace(/"/g, '\\"'));
            resultText = await new Promise((resolve) => {
              const child = spawn(cmdToExec, { shell: true });
              let stdout = '';
              let stderr = '';
              child.stdout?.on('data', (d) => (stdout += d.toString()));
              child.stderr?.on('data', (d) => (stderr += d.toString()));
              child.on('close', (code) => {
                if (code === 0 && stdout.trim()) {
                  resolve(stdout.trim());
                } else {
                  resolve(`[Spawned Worker Output (exit ${code})]: ${stdout || stderr || 'Execution finished.'}`);
                }
              });
            });
          }

          res.writeHead(200);
          res.end(
            JSON.stringify({
              jsonrpc: '2.0',
              id: rpc.id,
              result: {
                id: taskId,
                status: {
                  state: 'completed',
                  message: {
                    role: 'agent',
                    parts: [{ kind: 'text', text: resultText }],
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
    res.end(JSON.stringify({ error: 'Endpoint not found' }));
  });

  const listenPort = webhookUrl ? (options.port || 7189) : port;
  server.listen(listenPort, '0.0.0.0', async () => {
    console.log(`\n👂 Node receiver listening on http://127.0.0.1:${listenPort}`);
    console.log(`🌐 Ready to process A2A requests (Press Ctrl+C to terminate)\n`);
  });
}
