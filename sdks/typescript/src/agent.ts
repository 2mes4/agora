import { AgoraClient } from './client.js';
import { DelegateBuilder } from './delegate.js';
import { DirectoryClient, SearchServicesOptions } from './directory.js';
import { AgentHandler, ExposedAgent } from './server.js';
import {
  AgentCard,
  AgentService,
  AgentSkill,
  SearchServicesResponse,
  ServiceListing,
} from './types.js';

export interface AgentOptions {
  /**
   * Unique name of the agent (also registered in the AGORA directory).
   */
  name: string;
  /**
   * Human-readable description of the agent.
   */
  description?: string;
  /**
   * Version of the agent.
   */
  version?: string;
  /**
   * Skills declared by this agent.
   */
  skills?: AgentSkill[];
  /**
   * Marketplace services offered by this agent (with pricing).
   */
  services?: AgentService[];
  /**
   * Public URL advertised to the AGORA Gateway.
   * If omitted, defaults to the local bound address.
   */
  url?: string;
  /**
   * Port to listen on locally (defaults to 0 for random free port).
   */
  port?: number;
  /**
   * Host to listen on locally (defaults to '0.0.0.0').
   */
  host?: string;
  /**
   * AGORA Gateway URL (running in Rust, e.g. http://127.0.0.1:7100).
   */
  gatewayUrl?: string;
  /**
   * Optional API key for authenticating with the Gateway.
   */
  apiKey?: string;
  /**
   * Automatically send periodic heartbeats to the Gateway (default: true).
   */
  autoHeartbeat?: boolean;
  /**
   * Heartbeat interval in milliseconds (default: 30000).
   */
  heartbeatIntervalMs?: number;
}

/**
 * High-level AGORA Agent class for building, running, and connecting agents in TypeScript
 * against an AGORA Gateway.
 */
export class Agent {
  readonly name: string;
  readonly description?: string;
  readonly version: string;
  readonly skills: AgentSkill[];
  readonly services: AgentService[];
  readonly gatewayUrl: string;
  readonly apiKey?: string;

  private port?: number;
  private host: string;
  private advertisedUrl?: string;
  private autoHeartbeat: boolean;
  private heartbeatIntervalMs: number;

  private handler?: AgentHandler;
  private exposedAgent?: ExposedAgent;
  private client: AgoraClient;
  private directory: DirectoryClient;

  constructor(options: AgentOptions) {
    this.name = options.name;
    this.description = options.description;
    this.version = options.version || '0.1.0';
    this.skills = options.skills || [];
    this.services = options.services || [];
    this.port = options.port;
    this.host = options.host || '0.0.0.0';
    this.advertisedUrl = options.url;
    this.gatewayUrl = (options.gatewayUrl || 'http://127.0.0.1:7100').replace(/\/+$/, '');
    this.apiKey = options.apiKey;
    this.autoHeartbeat = options.autoHeartbeat ?? true;
    this.heartbeatIntervalMs = options.heartbeatIntervalMs || 30000;

    this.client = new AgoraClient({
      gatewayUrl: this.gatewayUrl,
      apiKey: this.apiKey,
      defaultSender: this.name,
    });

    this.directory = this.client.directory;
  }

  /**
   * Register the task handler function for incoming tasks.
   */
  onTask(handler: AgentHandler): this {
    this.handler = handler;
    return this;
  }

  /**
   * Start the agent:
   * 1. Starts the local HTTP & SSE listener.
   * 2. Registers the Agent Card with the AGORA Gateway (`POST /v1/agents`).
   * 3. Starts periodic heartbeats (`POST /v1/agents/{name}/heartbeat`).
   */
  async start(): Promise<string> {
    if (!this.handler) {
      throw new Error(`Agent '${this.name}' cannot start without an onTask handler.`);
    }

    const placeholderUrl = this.advertisedUrl || `http://127.0.0.1:${this.port || 0}`;

    this.exposedAgent = new ExposedAgent(
      {
        name: this.name,
        description: this.description,
        version: this.version,
        url: placeholderUrl,
        skills: this.skills,
        services: this.services,
      },
      this.handler,
      {
        gatewayUrl: this.gatewayUrl,
        apiKey: this.apiKey,
        autoHeartbeat: this.autoHeartbeat,
        heartbeatIntervalMs: this.heartbeatIntervalMs,
      }
    );

    const boundUrl = await this.exposedAgent.listen(this.port, this.host);
    const finalUrl = this.advertisedUrl || boundUrl;

    // Register card in Gateway
    const card: AgentCard = {
      name: this.name,
      description: this.description,
      version: this.version,
      url: finalUrl,
      capabilities: {
        streaming: true,
      },
      skills: this.skills,
      services: this.services,
    };

    try {
      await this.directory.register(card);
    } catch (err) {
      // Gateway may be offline during standalone testing; log warning
      console.warn(`[agora-agent:${this.name}] Gateway registration warning:`, err);
    }

    return boundUrl;
  }

  /**
   * Stop the agent and close all listeners.
   */
  async stop(): Promise<void> {
    if (this.exposedAgent) {
      await this.exposedAgent.close();
      this.exposedAgent = undefined;
    }
  }

  /**
   * The local URL where the agent is listening.
   */
  get boundUrl(): string {
    return this.exposedAgent?.boundUrl || '';
  }

  /**
   * Delegate a task to another agent via the AGORA Gateway.
   */
  delegate(targetAgentNameOrUrl: string): DelegateBuilder {
    return this.client.delegate(targetAgentNameOrUrl);
  }

  /**
   * Search for services in the marketplace (via Gateway Llull bridge).
   */
  async searchServices(
    query: string,
    options?: SearchServicesOptions
  ): Promise<SearchServicesResponse> {
    return this.directory.searchServices(query, options);
  }

  /**
   * List all services in the marketplace.
   */
  async listServices(options?: { onlineOnly?: boolean }): Promise<ServiceListing[]> {
    return this.directory.listServices(options);
  }

  /**
   * Evaluate the trust and credibility of a target agent from this agent's perspective.
   */
  async evaluateTrust(target: string): Promise<import('./types.js').TrustEvaluation> {
    return this.directory.evaluateTrust(target, this.name);
  }

  /**
   * Submit a review for a completed task to update the trust graph with Goma or Plomo.
   */
  async reviewTask(
    taskId: string,
    worker: string,
    outcome: import('./types.js').TaskReviewOutcome,
    options?: { feedback?: string; recommender?: string }
  ): Promise<import('./types.js').TaskReviewResponse> {
    return this.directory.reviewTask(taskId, {
      outcome,
      requester: this.name,
      worker,
      feedback: options?.feedback,
      recommender: options?.recommender,
    });
  }

  /**
   * Propose a smart contract to another agent from this agent.
   */
  async proposeContract(params: {
    worker: string;
    recommender?: string;
    pricing: import('./types.js').ContractPricing;
    execution: import('./types.js').ContractExecution;
    disputeTerms: import('./types.js').ContractDisputeTerms;
  }): Promise<import('./types.js').AgenticContract> {
    return this.directory.proposeContract({
      parties: {
        requester: this.name,
        worker: params.worker,
        recommender: params.recommender,
      },
      pricing: params.pricing,
      execution: params.execution,
      disputeTerms: params.disputeTerms,
    });
  }

  /**
   * Accept an incoming contract proposed to this agent.
   */
  async acceptContract(id: string): Promise<import('./types.js').AgenticContract> {
    return this.directory.acceptContract(id);
  }

  /**
   * Deliver output for an active contract executed by this agent.
   */
  async deliverContract(id: string, outputPayload: Record<string, any>): Promise<import('./types.js').AgenticContract> {
    return this.directory.deliverContract(id, outputPayload);
  }

  /**
   * Settle a delivered contract after acceptance criteria pass.
   */
  async settleContract(id: string): Promise<import('./types.js').AgenticContract> {
    return this.directory.settleContract(id);
  }

  /**
   * Dispute a contract delivery.
   */
  async disputeContract(id: string, reason: string): Promise<import('./types.js').AgenticContract> {
    return this.directory.disputeContract(id, reason);
  }
}
