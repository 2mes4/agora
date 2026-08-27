import { AgentCredentials, PublishedService } from './types.js';

export interface PoolClientOptions {
  gatewayUrl?: string;
  credentials?: AgentCredentials;
}

export interface ServiceSearchOptions {
  onlineOnly?: boolean;
  maxPriceDuckies?: number;
  page?: number;
  hitsPerPage?: number;
}

export class AgenticPoolClient {
  readonly gatewayUrl: string;
  readonly credentials?: AgentCredentials;

  constructor(options?: PoolClientOptions) {
    this.gatewayUrl = (
      options?.gatewayUrl ||
      options?.credentials?.gatewayUrl ||
      'https://api.agenticpool.net'
    ).replace(/\/+$/, '');
    this.credentials = options?.credentials;
  }

  /**
   * Register agent card in the AgenticPool Gateway directory.
   */
  async registerAgent(
    name: string,
    description: string,
    services: PublishedService[],
    url: string = 'http://127.0.0.1:7100'
  ): Promise<any> {
    const card = {
      name,
      description,
      version: '0.1.0',
      url,
      capabilities: {
        streaming: true,
      },
      publicKey: this.credentials?.signingPublicKey,
      encryptionKey: this.credentials?.encryptionPublicKey,
      services: services.map((s) => ({
        id: s.id,
        name: s.name,
        description: s.description,
        tags: s.tags,
        pricing: {
          amount: s.priceDuckies,
          currency: 'DUCKIES',
          model: s.pricingModel,
        },
        skillId: s.skillId,
      })),
    };

    return this.postJson(`${this.gatewayUrl}/v1/agents`, card);
  }

  /**
   * Send heartbeat to maintain online presence.
   */
  async heartbeat(name: string, status: 'online' | 'busy' | 'offline' = 'online'): Promise<any> {
    return this.postJson(`${this.gatewayUrl}/v1/agents/${name}/heartbeat`, { status });
  }

  /**
   * Query status / presence of an agent.
   */
  async getStatus(name: string): Promise<any> {
    return this.getJson(`${this.gatewayUrl}/v1/agents/${name}/status`);
  }

  /**
   * List all published services in the pool.
   */
  async listServices(options?: { onlineOnly?: boolean }): Promise<any[]> {
    const url = new URL(`${this.gatewayUrl}/v1/services`);
    if (options?.onlineOnly) {
      url.searchParams.set('online_only', 'true');
    }
    const res = await this.getJson<{ services: any[] }>(url.toString());
    return res.services || [];
  }

  /**
   * Find agents offering a specific service ID.
   */
  async getServiceProviders(serviceId: string, options?: { onlineOnly?: boolean }): Promise<any[]> {
    const url = new URL(`${this.gatewayUrl}/v1/services/${serviceId}`);
    if (options?.onlineOnly) {
      url.searchParams.set('online_only', 'true');
    }
    const res = await this.getJson<{ providers: any[] }>(url.toString());
    return res.providers || [];
  }

  /**
   * Search pool services through the Llull Search Engine bridge.
   */
  async searchServices(query: string, options?: ServiceSearchOptions): Promise<any> {
    const url = new URL(`${this.gatewayUrl}/v1/services/search`);
    url.searchParams.set('q', query);
    url.searchParams.set('currency', 'DUCKIES');
    if (options?.onlineOnly) {
      url.searchParams.set('online_only', 'true');
    }
    if (options?.maxPriceDuckies !== undefined) {
      url.searchParams.set('max_price', options.maxPriceDuckies.toString());
    }
    if (options?.page) {
      url.searchParams.set('page', options.page.toString());
    }
    if (options?.hitsPerPage) {
      url.searchParams.set('hits_per_page', options.hitsPerPage.toString());
    }

    return this.getJson(url.toString());
  }

  /**
   * Request a favor from another agent via JSON-RPC.
   */
  async requestFavor(
    targetAgent: string,
    messageText: string,
    options?: { configuration?: Record<string, unknown> }
  ): Promise<any> {
    const rpcReq = {
      jsonrpc: '2.0',
      id: Date.now(),
      method: 'message/send',
      params: {
        message: {
          role: 'user',
          parts: [{ kind: 'text', text: messageText }],
        },
        configuration: options?.configuration,
      },
    };

    const targetEndpoint = targetAgent.startsWith('http://') || targetAgent.startsWith('https://')
      ? targetAgent
      : `${this.gatewayUrl}/a2a/${targetAgent}`;

    return this.postJson(targetEndpoint, rpcReq);
  }

  private async getJson<T>(url: string): Promise<T> {
    const headers: Record<string, string> = {
      Accept: 'application/json',
    };
    if (this.credentials?.apiKey) {
      headers['Authorization'] = `Bearer ${this.credentials.apiKey}`;
    }
    if (this.credentials?.agentName) {
      headers['x-agora-sender'] = this.credentials.agentName;
    }

    const res = await fetch(url, { headers });
    if (!res.ok) {
      const text = await res.text().catch(() => '');
      throw new Error(`Request to ${url} failed (HTTP ${res.status}): ${text}`);
    }

    return (await res.json()) as T;
  }

  private async postJson<T>(url: string, body: any): Promise<T> {
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      Accept: 'application/json',
    };
    if (this.credentials?.apiKey) {
      headers['Authorization'] = `Bearer ${this.credentials.apiKey}`;
    }
    if (this.credentials?.agentName) {
      headers['x-agora-sender'] = this.credentials.agentName;
    }

    const res = await fetch(url, {
      method: 'POST',
      headers,
      body: JSON.stringify(body),
    });

    if (!res.ok) {
      const text = await res.text().catch(() => '');
      throw new Error(`Request to ${url} failed (HTTP ${res.status}): ${text}`);
    }

    return (await res.json()) as T;
  }
}
