import {
  AgentCard,
  AgentPresence,
  AgentStatus,
  SearchServicesResponse,
  ServiceListing,
} from './types.js';

export interface DirectoryOptions {
  gatewayUrl?: string;
  apiKey?: string;
}

export interface SearchServicesOptions {
  onlineOnly?: boolean;
  maxPrice?: number;
  currency?: string;
  page?: number;
  hitsPerPage?: number;
}

export class DirectoryClient {
  readonly gatewayUrl: string;
  readonly apiKey?: string;

  constructor(options?: DirectoryOptions) {
    this.gatewayUrl = (options?.gatewayUrl || 'http://127.0.0.1:7100').replace(/\/+$/, '');
    this.apiKey = options?.apiKey;
  }

  /**
   * List all registered agents in the directory.
   */
  async listAgents(): Promise<AgentCard[]> {
    const res = await this.fetchJson<{ agents: AgentCard[] }>(`${this.gatewayUrl}/v1/agents`);
    return res.agents || [];
  }

  /**
   * Get an agent's registered card by name.
   */
  async getAgent(name: string): Promise<AgentCard | null> {
    try {
      return await this.fetchJson<AgentCard>(`${this.gatewayUrl}/v1/agents/${name}`);
    } catch (err: unknown) {
      if (err instanceof Error && err.message.includes('404')) {
        return null;
      }
      throw err;
    }
  }

  /**
   * Register or update an agent card manifest in the directory.
   */
  async register(card: AgentCard): Promise<AgentCard> {
    return this.fetchJson<AgentCard>(`${this.gatewayUrl}/v1/agents`, {
      method: 'POST',
      body: JSON.stringify(card),
    });
  }

  /**
   * Unregister an agent from the directory.
   */
  async unregister(name: string): Promise<void> {
    await this.fetchVoid(`${this.gatewayUrl}/v1/agents/${name}`, {
      method: 'DELETE',
    });
  }

  /**
   * Send a heartbeat for an agent to refresh its presence and online status.
   */
  async heartbeat(name: string, status?: AgentStatus): Promise<AgentPresence> {
    return this.fetchJson<AgentPresence>(`${this.gatewayUrl}/v1/agents/${name}/heartbeat`, {
      method: 'POST',
      body: status ? JSON.stringify({ status }) : undefined,
    });
  }

  /**
   * Get current presence and online status for an agent.
   */
  async getStatus(name: string): Promise<AgentPresence | null> {
    try {
      return await this.fetchJson<AgentPresence>(`${this.gatewayUrl}/v1/agents/${name}/status`);
    } catch (err: unknown) {
      if (err instanceof Error && err.message.includes('404')) {
        return null;
      }
      throw err;
    }
  }

  /**
   * List all services available across all agents in the marketplace.
   */
  async listServices(options?: { onlineOnly?: boolean }): Promise<ServiceListing[]> {
    const url = new URL(`${this.gatewayUrl}/v1/services`);
    if (options?.onlineOnly) {
      url.searchParams.set('online_only', 'true');
    }
    const res = await this.fetchJson<{ services: ServiceListing[] }>(url.toString());
    return res.services || [];
  }

  /**
   * Find agents offering a specific service ID with pricing and presence.
   */
  async getServiceProviders(
    serviceId: string,
    options?: { onlineOnly?: boolean }
  ): Promise<ServiceListing[]> {
    const url = new URL(`${this.gatewayUrl}/v1/services/${serviceId}`);
    if (options?.onlineOnly) {
      url.searchParams.set('online_only', 'true');
    }
    const res = await this.fetchJson<{ serviceId: string; providers: ServiceListing[] }>(
      url.toString()
    );
    return res.providers || [];
  }

  /**
   * Search marketplace services via the Llull Search Engine bridge.
   */
  async searchServices(
    query: string,
    options?: SearchServicesOptions
  ): Promise<SearchServicesResponse> {
    const url = new URL(`${this.gatewayUrl}/v1/services/search`);
    url.searchParams.set('q', query);
    if (options?.onlineOnly) {
      url.searchParams.set('online_only', 'true');
    }
    if (options?.maxPrice !== undefined) {
      url.searchParams.set('max_price', options.maxPrice.toString());
    }
    if (options?.currency) {
      url.searchParams.set('currency', options.currency);
    }
    if (options?.page) {
      url.searchParams.set('page', options.page.toString());
    }
    if (options?.hitsPerPage) {
      url.searchParams.set('hits_per_page', options.hitsPerPage.toString());
    }

    return this.fetchJson<SearchServicesResponse>(url.toString());
  }

  /**
   * Evaluate the trust and credibility of a target agent from the perspective of an evaluator.
   */
  async evaluateTrust(target: string, from?: string): Promise<import('./types.js').TrustEvaluation> {
    const url = new URL(`${this.gatewayUrl}/v1/trust/evaluate`);
    url.searchParams.set('target', target);
    if (from) {
      url.searchParams.set('from', from);
    }
    return this.fetchJson<import('./types.js').TrustEvaluation>(url.toString());
  }

  /**
   * Submit a review for a completed or disputed task (Proof-of-Execution).
   * Internally updates the perspectivist trust graph based on the task outcome.
   */
  async reviewTask(
    taskId: string,
    payload: import('./types.js').TaskReviewPayload
  ): Promise<import('./types.js').TaskReviewResponse> {
    return this.fetchJson<import('./types.js').TaskReviewResponse>(
      `${this.gatewayUrl}/v1/tasks/${taskId}/review`,
      {
        method: 'POST',
        body: JSON.stringify(payload),
      }
    );
  }

  /**
   * Propose a new Agentic Smart Contract.
   */
  async proposeContract(payload: {
    parties: import('./types.js').ContractParties;
    pricing: import('./types.js').ContractPricing;
    execution: import('./types.js').ContractExecution;
    disputeTerms: import('./types.js').ContractDisputeTerms;
  }): Promise<import('./types.js').AgenticContract> {
    return this.fetchJson<import('./types.js').AgenticContract>(`${this.gatewayUrl}/v1/contracts`, {
      method: 'POST',
      body: JSON.stringify(payload),
    });
  }

  /**
   * Get an Agentic Contract by ID.
   */
  async getContract(id: string): Promise<import('./types.js').AgenticContract> {
    return this.fetchJson<import('./types.js').AgenticContract>(`${this.gatewayUrl}/v1/contracts/${id}`);
  }

  /**
   * List contracts, optionally filtered by party.
   */
  async listContracts(party?: string): Promise<import('./types.js').AgenticContract[]> {
    const url = new URL(`${this.gatewayUrl}/v1/contracts`);
    if (party) {
      url.searchParams.set('party', party);
    }
    const res = await this.fetchJson<{ contracts: import('./types.js').AgenticContract[] }>(url.toString());
    return res.contracts;
  }

  /**
   * Worker accepts and signs a proposed contract.
   */
  async acceptContract(id: string, workerSignature?: string): Promise<import('./types.js').AgenticContract> {
    return this.fetchJson<import('./types.js').AgenticContract>(`${this.gatewayUrl}/v1/contracts/${id}/accept`, {
      method: 'POST',
      body: JSON.stringify({ workerSignature }),
    });
  }

  /**
   * Worker delivers the execution output payload.
   */
  async deliverContract(id: string, outputPayload: Record<string, any>): Promise<import('./types.js').AgenticContract> {
    return this.fetchJson<import('./types.js').AgenticContract>(`${this.gatewayUrl}/v1/contracts/${id}/deliver`, {
      method: 'POST',
      body: JSON.stringify({ outputPayload }),
    });
  }

  /**
   * Evaluate contract delivery against acceptance criteria prompt (returns true/false/uncertain).
   */
  async evaluateContractAcceptance(id: string): Promise<import('./types.js').AcceptanceEvaluation> {
    return this.fetchJson<import('./types.js').AcceptanceEvaluation>(`${this.gatewayUrl}/v1/contracts/${id}/evaluate`, {
      method: 'POST',
    });
  }

  /**
   * Requester settles a delivered contract (+1 Goma awarded).
   */
  async settleContract(id: string): Promise<import('./types.js').AgenticContract> {
    return this.fetchJson<import('./types.js').AgenticContract>(`${this.gatewayUrl}/v1/contracts/${id}/settle`, {
      method: 'POST',
    });
  }

  /**
   * Open a dispute on a contract.
   */
  async disputeContract(id: string, reason: string): Promise<import('./types.js').AgenticContract> {
    return this.fetchJson<import('./types.js').AgenticContract>(`${this.gatewayUrl}/v1/contracts/${id}/dispute`, {
      method: 'POST',
      body: JSON.stringify({ reason }),
    });
  }

  /**
   * Arbitrator resolves a disputed contract enforcing the Loser-Pays rule.
   */
  async arbitrateContract(
    id: string,
    params: {
      arbitrator: string;
      verdict: import('./types.js').ArbitrationVerdict;
      rationale: string;
    }
  ): Promise<import('./types.js').ArbitrationSettlement> {
    return this.fetchJson<import('./types.js').ArbitrationSettlement>(`${this.gatewayUrl}/v1/contracts/${id}/arbitrate`, {
      method: 'POST',
      body: JSON.stringify(params),
    });
  }

  private async fetchJson<T>(url: string, init?: RequestInit): Promise<T> {
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
    };
    if (this.apiKey) {
      headers['Authorization'] = `Bearer ${this.apiKey}`;
    }

    const res = await fetch(url, {
      ...init,
      headers: {
        ...headers,
        ...(init?.headers as Record<string, string>),
      },
    });

    if (!res.ok) {
      const errText = await res.text().catch(() => '');
      throw new Error(`Directory request failed (HTTP ${res.status}): ${errText}`);
    }

    return (await res.json()) as T;
  }

  private async fetchVoid(url: string, init?: RequestInit): Promise<void> {
    const headers: Record<string, string> = {};
    if (this.apiKey) {
      headers['Authorization'] = `Bearer ${this.apiKey}`;
    }

    const res = await fetch(url, {
      ...init,
      headers: {
        ...headers,
        ...(init?.headers as Record<string, string>),
      },
    });

    if (!res.ok) {
      const errText = await res.text().catch(() => '');
      throw new Error(`Directory request failed (HTTP ${res.status}): ${errText}`);
    }
  }
}
