import { parseSseStream } from './events.js';
import { DelegateBuilder } from './delegate.js';
import {
  A2aEvent,
  AgentCard,
  CancelTaskParams,
  GetTaskParams,
  JsonRpcRequest,
  JsonRpcResponse,
  Message,
  SendParams,
  Task,
} from './types.js';

export interface ClientOptions {
  gatewayUrl?: string;
  apiKey?: string;
  defaultSender?: string;
}

export interface RequestOptions {
  configuration?: Record<string, unknown>;
  pushNotificationConfig?: { url: string; token?: string };
  bearerToken?: string;
  sender?: string;
}

export class AgoraClient {
  readonly gatewayUrl: string;
  readonly apiKey?: string;
  readonly defaultSender?: string;

  constructor(options?: ClientOptions) {
    this.gatewayUrl = (options?.gatewayUrl || 'http://127.0.0.1:7100').replace(/\/+$/, '');
    this.apiKey = options?.apiKey;
    this.defaultSender = options?.defaultSender;
  }

  /**
   * Start building a delegation request to a target agent URL or name.
   */
  delegate(targetUrlOrName: string): DelegateBuilder {
    const targetUrl = this.resolveTargetUrl(targetUrlOrName);
    return new DelegateBuilder(this, targetUrl);
  }

  /**
   * Fetch the agent card from target endpoint or gateway directory.
   */
  async agentCard(targetUrlOrName?: string): Promise<AgentCard> {
    const url = targetUrlOrName
      ? this.resolveTargetUrl(targetUrlOrName)
      : this.gatewayUrl;
    
    // Check if target is a hosted endpoint or direct agent
    const cardUrl = url.endsWith('/.well-known/agent-card.json')
      ? url
      : `${url}/.well-known/agent-card.json`;

    const res = await fetch(cardUrl, {
      headers: this.buildHeaders(),
    });

    if (!res.ok) {
      throw new Error(`Failed to fetch agent card from ${cardUrl}: HTTP ${res.status}`);
    }

    return (await res.json()) as AgentCard;
  }

  /**
   * Send a task request synchronously via JSON-RPC `message/send`.
   */
  async send(
    targetUrlOrName: string,
    message: Message,
    options?: RequestOptions
  ): Promise<Task> {
    const targetUrl = this.resolveTargetUrl(targetUrlOrName);
    const params: SendParams = {
      message,
      configuration: options?.configuration,
      pushNotificationConfig: options?.pushNotificationConfig,
    };

    const req: JsonRpcRequest<SendParams> = {
      jsonrpc: '2.0',
      id: Date.now(),
      method: 'message/send',
      params,
    };

    const res = await this.postJsonRpc<Task>(targetUrl, req, options);
    return res;
  }

  /**
   * Send a task request and stream progress events via SSE `message/stream`.
   */
  async *stream(
    targetUrlOrName: string,
    message: Message,
    options?: RequestOptions
  ): AsyncGenerator<A2aEvent, void, unknown> {
    const targetUrl = this.resolveTargetUrl(targetUrlOrName);
    const params: SendParams = {
      message,
      configuration: options?.configuration,
      pushNotificationConfig: options?.pushNotificationConfig,
    };

    const req: JsonRpcRequest<SendParams> = {
      jsonrpc: '2.0',
      id: Date.now(),
      method: 'message/stream',
      params,
    };

    const headers = this.buildHeaders(options);
    headers['Content-Type'] = 'application/json';
    headers['Accept'] = 'text/event-stream';

    const res = await fetch(targetUrl, {
      method: 'POST',
      headers,
      body: JSON.stringify(req),
    });

    if (!res.ok) {
      const errText = await res.text().catch(() => '');
      throw new Error(`Streaming failed (HTTP ${res.status}): ${errText}`);
    }

    if (!res.body) {
      throw new Error('Streaming response missing body');
    }

    yield* parseSseStream(res.body);
  }

  /**
   * Get the current state of a task by ID.
   */
  async getTask(
    targetUrlOrName: string,
    taskId: string,
    contextId?: string,
    options?: RequestOptions
  ): Promise<Task> {
    const targetUrl = this.resolveTargetUrl(targetUrlOrName);
    const params: GetTaskParams = { taskId, contextId };
    const req: JsonRpcRequest<GetTaskParams> = {
      jsonrpc: '2.0',
      id: Date.now(),
      method: 'tasks/get',
      params,
    };
    return this.postJsonRpc<Task>(targetUrl, req, options);
  }

  /**
   * Cancel an ongoing task by ID.
   */
  async cancelTask(
    targetUrlOrName: string,
    taskId: string,
    reason?: string,
    options?: RequestOptions
  ): Promise<Task> {
    const targetUrl = this.resolveTargetUrl(targetUrlOrName);
    const params: CancelTaskParams = { taskId, reason };
    const req: JsonRpcRequest<CancelTaskParams> = {
      jsonrpc: '2.0',
      id: Date.now(),
      method: 'tasks/cancel',
      params,
    };
    return this.postJsonRpc<Task>(targetUrl, req, options);
  }

  private resolveTargetUrl(target: string): string {
    if (target.startsWith('http://') || target.startsWith('https://')) {
      return target;
    }
    // Target is an agent name hosted on the gateway
    return `${this.gatewayUrl}/a2a/${target}`;
  }

  private buildHeaders(options?: RequestOptions): Record<string, string> {
    const headers: Record<string, string> = {};
    const token = options?.bearerToken || this.apiKey;
    if (token) {
      headers['Authorization'] = `Bearer ${token}`;
    }
    const sender = options?.sender || this.defaultSender;
    if (sender) {
      headers['x-agora-sender'] = sender;
    }
    return headers;
  }

  private async postJsonRpc<T>(
    url: string,
    request: JsonRpcRequest,
    options?: RequestOptions
  ): Promise<T> {
    const headers = this.buildHeaders(options);
    headers['Content-Type'] = 'application/json';

    const res = await fetch(url, {
      method: 'POST',
      headers,
      body: JSON.stringify(request),
    });

    if (!res.ok) {
      const errText = await res.text().catch(() => '');
      throw new Error(`JSON-RPC request to ${url} failed (HTTP ${res.status}): ${errText}`);
    }

    const json = (await res.json()) as JsonRpcResponse<T>;
    if (json.error) {
      throw new Error(`JSON-RPC error (${json.error.code}): ${json.error.message}`);
    }

    if (json.result === undefined) {
      throw new Error('JSON-RPC response missing result');
    }

    return json.result;
  }
}
