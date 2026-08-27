import * as http from 'node:http';
import { formatSseEvent } from './events.js';
import { HeartbeatEmitter } from './heartbeat.js';
import { DirectoryClient } from './directory.js';
import {
  A2aEvent,
  AgentCard,
  AgentService,
  AgentSkill,
  Artifact,
  CancelTaskParams,
  GetTaskParams,
  JsonRpcRequest,
  JsonRpcResponse,
  Message,
  SendParams,
  Task,
  TaskState,
} from './types.js';

export class TaskContext {
  readonly taskId: string;
  readonly message: Message;
  private task: Task;
  private sseRes?: http.ServerResponse;
  private canceled = false;

  constructor(taskId: string, message: Message, sseRes?: http.ServerResponse) {
    this.taskId = taskId;
    this.message = message;
    this.sseRes = sseRes;
    this.task = {
      id: taskId,
      status: {
        state: 'submitted',
        timestamp: new Date().toISOString(),
      },
      history: [message],
      artifacts: [],
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    };
  }

  /**
   * Check if the task has been requested to cancel.
   */
  isCanceled(): boolean {
    return this.canceled;
  }

  /**
   * Mark the task as canceled internally.
   */
  markCanceled(): void {
    this.canceled = true;
    this.task.status.state = 'canceled';
    this.task.updatedAt = new Date().toISOString();
    this.emitEvent({
      kind: 'status-update',
      status: this.task.status,
      isFinal: true,
    });
  }

  /**
   * Update task state, progress, or emit a response message.
   */
  async update(params: {
    state?: TaskState;
    message?: Message | string;
    progress?: number;
  }): Promise<void> {
    if (this.canceled) return;

    if (params.state) {
      this.task.status.state = params.state;
    }
    if (params.progress !== undefined) {
      this.task.status.progress = params.progress;
    }

    if (params.message) {
      const msg: Message =
        typeof params.message === 'string'
          ? {
              role: 'agent',
              parts: [{ kind: 'text', text: params.message }],
              createdAt: new Date().toISOString(),
            }
          : params.message;
      this.task.status.message = msg;
      this.task.history?.push(msg);

      this.emitEvent({
        kind: 'message',
        message: msg,
        isFinal: params.state === 'completed' || params.state === 'failed',
      });
    } else {
      this.emitEvent({
        kind: 'status-update',
        status: this.task.status,
        isFinal: params.state === 'completed' || params.state === 'failed',
      });
    }

    this.task.updatedAt = new Date().toISOString();
  }

  /**
   * Emit an artifact generated during task execution.
   */
  async emitArtifact(artifact: Artifact): Promise<void> {
    if (this.canceled) return;

    this.task.artifacts = this.task.artifacts || [];
    this.task.artifacts.push(artifact);

    this.emitEvent({
      kind: 'artifact-update',
      artifact,
      isFinal: artifact.isFinal,
    });
  }

  /**
   * Return the current in-memory task snapshot.
   */
  getTask(): Task {
    return { ...this.task };
  }

  private emitEvent(event: A2aEvent): void {
    if (this.sseRes && !this.sseRes.writableEnded) {
      this.sseRes.write(formatSseEvent(event));
    }
  }
}

export type AgentHandler = (
  message: Message,
  context: TaskContext
) => Promise<void | Message | string>;

export interface AgentDefinition {
  name: string;
  description?: string;
  version?: string;
  url: string;
  skills?: AgentSkill[];
  services?: AgentService[];
  streaming?: boolean;
  publicKey?: string;
  encryptionKey?: string;
}

export interface ExposeOptions {
  port?: number;
  host?: string;
  gatewayUrl?: string;
  apiKey?: string;
  autoHeartbeat?: boolean;
  heartbeatIntervalMs?: number;
}

export class ExposedAgent {
  readonly definition: AgentDefinition;
  readonly card: AgentCard;
  private server: http.Server;
  private tasks = new Map<string, TaskContext>();
  private heartbeatEmitter?: HeartbeatEmitter;
  private _boundUrl: string = '';

  constructor(
    definition: AgentDefinition,
    handler: AgentHandler,
    options?: ExposeOptions
  ) {
    this.definition = definition;
    this.card = {
      name: definition.name,
      description: definition.description,
      version: definition.version || '0.1.0',
      url: definition.url,
      capabilities: {
        streaming: definition.streaming ?? true,
      },
      skills: definition.skills || [],
      services: definition.services || [],
      publicKey: definition.publicKey,
      encryptionKey: definition.encryptionKey,
    };

    if (options?.autoHeartbeat && options.gatewayUrl) {
      const dirClient = new DirectoryClient({
        gatewayUrl: options.gatewayUrl,
        apiKey: options.apiKey,
      });
      this.heartbeatEmitter = new HeartbeatEmitter(
        dirClient,
        definition.name,
        options.heartbeatIntervalMs || 30000
      );
    }

    this.server = http.createServer(async (req, res) => {
      // CORS headers
      res.setHeader('Access-Control-Allow-Origin', '*');
      res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
      res.setHeader('Access-Control-Allow-Headers', 'Content-Type, Authorization, x-agora-sender');

      if (req.method === 'OPTIONS') {
        res.writeHead(204);
        res.end();
        return;
      }

      const url = new URL(req.url || '/', `http://${req.headers.host || 'localhost'}`);

      // 1. Serve Agent Card
      if (
        req.method === 'GET' &&
        (url.pathname === '/.well-known/agent-card.json' || url.pathname === '/card')
      ) {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify(this.card));
        return;
      }

      // 2. JSON-RPC endpoints
      if (req.method === 'POST') {
        let body = '';
        req.on('data', (chunk) => {
          body += chunk;
        });

        req.on('end', async () => {
          try {
            const rpcReq = JSON.parse(body) as JsonRpcRequest;
            await this.handleJsonRpc(rpcReq, handler, res);
          } catch (err: unknown) {
            const message = err instanceof Error ? err.message : String(err);
            res.writeHead(400, { 'Content-Type': 'application/json' });
            res.end(
              JSON.stringify({
                jsonrpc: '2.0',
                id: null,
                error: { code: -32700, message: `Parse error: ${message}` },
              } satisfies JsonRpcResponse)
            );
          }
        });
        return;
      }

      res.writeHead(404, { 'Content-Type': 'text/plain' });
      res.end('Not Found');
    });
  }

  get boundUrl(): string {
    return this._boundUrl || this.definition.url;
  }

  private async handleJsonRpc(
    req: JsonRpcRequest,
    handler: AgentHandler,
    res: http.ServerResponse
  ): Promise<void> {
    const { method, params, id } = req;

    switch (method) {
      case 'message/send': {
        const sendParams = params as SendParams;
        const taskId = `task-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
        const ctx = new TaskContext(taskId, sendParams.message);
        this.tasks.set(taskId, ctx);

        try {
          await ctx.update({ state: 'working' });
          const result = await handler(sendParams.message, ctx);
          if (result) {
            await ctx.update({ state: 'completed', message: result });
          } else if (ctx.getTask().status.state === 'working') {
            await ctx.update({ state: 'completed' });
          }
        } catch (err: unknown) {
          const errMsg = err instanceof Error ? err.message : String(err);
          await ctx.update({ state: 'failed', message: `Execution failed: ${errMsg}` });
        }

        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(
          JSON.stringify({
            jsonrpc: '2.0',
            id,
            result: ctx.getTask(),
          } satisfies JsonRpcResponse<Task>)
        );
        break;
      }

      case 'message/stream': {
        const sendParams = params as SendParams;
        const taskId = `task-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;

        res.writeHead(200, {
          'Content-Type': 'text/event-stream',
          'Cache-Control': 'no-cache',
          Connection: 'keep-alive',
        });

        const ctx = new TaskContext(taskId, sendParams.message, res);
        this.tasks.set(taskId, ctx);

        // Initial task event
        res.write(formatSseEvent({ kind: 'task', task: ctx.getTask() }));

        try {
          await ctx.update({ state: 'working' });
          const result = await handler(sendParams.message, ctx);
          if (result) {
            await ctx.update({ state: 'completed', message: result });
          } else if (ctx.getTask().status.state === 'working') {
            await ctx.update({ state: 'completed' });
          }
        } catch (err: unknown) {
          const errMsg = err instanceof Error ? err.message : String(err);
          await ctx.update({ state: 'failed', message: `Execution failed: ${errMsg}` });
        } finally {
          res.end();
        }
        break;
      }

      case 'tasks/get': {
        const getParams = params as GetTaskParams;
        const ctx = this.tasks.get(getParams.taskId);
        if (!ctx) {
          res.writeHead(200, { 'Content-Type': 'application/json' });
          res.end(
            JSON.stringify({
              jsonrpc: '2.0',
              id,
              error: { code: -32602, message: `Task '${getParams.taskId}' not found` },
            } satisfies JsonRpcResponse)
          );
          return;
        }

        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(
          JSON.stringify({
            jsonrpc: '2.0',
            id,
            result: ctx.getTask(),
          } satisfies JsonRpcResponse<Task>)
        );
        break;
      }

      case 'tasks/cancel': {
        const cancelParams = params as CancelTaskParams;
        const ctx = this.tasks.get(cancelParams.taskId);
        if (!ctx) {
          res.writeHead(200, { 'Content-Type': 'application/json' });
          res.end(
            JSON.stringify({
              jsonrpc: '2.0',
              id,
              error: { code: -32602, message: `Task '${cancelParams.taskId}' not found` },
            } satisfies JsonRpcResponse)
          );
          return;
        }

        ctx.markCanceled();
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(
          JSON.stringify({
            jsonrpc: '2.0',
            id,
            result: ctx.getTask(),
          } satisfies JsonRpcResponse<Task>)
        );
        break;
      }

      default: {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(
          JSON.stringify({
            jsonrpc: '2.0',
            id,
            error: { code: -32601, message: `Method '${method}' not found` },
          } satisfies JsonRpcResponse)
        );
        break;
      }
    }
  }

  /**
   * Start listening for connections.
   */
  async listen(port?: number, host: string = '0.0.0.0'): Promise<string> {
    let p = port;
    if (p === undefined) {
      try {
        const parsed = new URL(this.definition.url);
        p = parsed.port ? parseInt(parsed.port, 10) : 0;
      } catch {
        p = 0;
      }
    }

    return new Promise((resolve, reject) => {
      this.server.on('error', reject);
      this.server.listen(p, host, () => {
        const addr = this.server.address();
        if (typeof addr === 'object' && addr) {
          const boundPort = addr.port;
          this._boundUrl = `http://${host === '0.0.0.0' ? '127.0.0.1' : host}:${boundPort}`;
          if (this.heartbeatEmitter) {
            this.heartbeatEmitter.start();
          }
          resolve(this._boundUrl);
        } else {
          this._boundUrl = this.definition.url;
          resolve(this._boundUrl);
        }
      });
    });
  }

  /**
   * Close the server.
   */
  async close(): Promise<void> {
    if (this.heartbeatEmitter) {
      await this.heartbeatEmitter.stop();
    }
    return new Promise((resolve, reject) => {
      this.server.close((err) => {
        if (err) reject(err);
        else resolve();
      });
    });
  }
}

/**
 * Expose an agent definition with a task handler as an A2A HTTP server.
 */
export async function expose(
  definition: AgentDefinition,
  handler: AgentHandler,
  options?: ExposeOptions
): Promise<ExposedAgent> {
  const agent = new ExposedAgent(definition, handler, options);
  await agent.listen(options?.port, options?.host);
  return agent;
}
