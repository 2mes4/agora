/**
 * AGORA / A2A Protocol Types and Marketplace Models.
 */

export type TaskState =
  | 'submitted'
  | 'working'
  | 'input-required'
  | 'completed'
  | 'failed'
  | 'canceled'
  | 'rejected'
  | 'auth-required'
  | 'unknown';

export type PricingModel = 'per_call' | 'per_minute' | 'flat' | 'subscription' | (string & {});

export interface ServicePricing {
  amount: number;
  currency: string;
  model: PricingModel;
}

export interface AgentService {
  id: string;
  name: string;
  description?: string;
  tags: string[];
  pricing: ServicePricing;
  skillId?: string;
  inputSchema?: Record<string, unknown>;
  outputSchema?: Record<string, unknown>;
  metadata?: Record<string, unknown>;
}

export interface AgentSkill {
  id: string;
  name: string;
  description?: string;
  tags?: string[];
  inputSchema?: Record<string, unknown>;
  outputSchema?: Record<string, unknown>;
}

export interface AgentCapabilities {
  streaming?: boolean;
  pushNotifications?: boolean;
  stateTransitionHistory?: boolean;
}

export interface AgentCard {
  name: string;
  description?: string;
  url: string;
  version: string;
  capabilities?: AgentCapabilities;
  skills?: AgentSkill[];
  services?: AgentService[];
  publicKey?: string;
  encryptionKey?: string;
  metadata?: Record<string, unknown>;
}

export type AgentStatus = 'online' | 'busy' | 'offline';

export interface AgentPresence {
  agentName: string;
  status: AgentStatus;
  lastSeen: string;
  isOnline: boolean;
}

export interface ServiceListing {
  agentName: string;
  agentUrl: string;
  service: AgentService;
  presence: AgentPresence;
}

export type MessageRole = 'user' | 'agent';

export interface TextPart {
  kind: 'text';
  text: string;
}

export interface FilePart {
  kind: 'file';
  file: {
    name?: string;
    mimeType: string;
    data?: string;
    uri?: string;
  };
}

export interface DataPart {
  kind: 'data';
  data: Record<string, unknown>;
}

export type Part = TextPart | FilePart | DataPart;

export interface Message {
  role: MessageRole;
  parts: Part[];
  contextUri?: string;
  messageId?: string;
  parentMessageId?: string;
  createdAt?: string;
}

export interface TaskStatus {
  state: TaskState;
  message?: Message;
  timestamp?: string;
  progress?: number;
}

export interface Artifact {
  name: string;
  data: Record<string, unknown> | string;
  isFinal?: boolean;
}

export interface Task {
  id: string;
  contextId?: string;
  status: TaskStatus;
  artifacts?: Artifact[];
  history?: Message[];
  createdAt?: string;
  updatedAt?: string;
}

export interface TaskEvent {
  kind: 'task';
  task: Task;
  isFinal?: boolean;
}

export interface MessageEvent {
  kind: 'message';
  message: Message;
  isFinal?: boolean;
}

export interface StatusUpdateEvent {
  kind: 'status-update';
  status: TaskStatus;
  isFinal?: boolean;
}

export interface ArtifactUpdateEvent {
  kind: 'artifact-update';
  artifact: Artifact;
  isFinal?: boolean;
}

export type A2aEvent = TaskEvent | MessageEvent | StatusUpdateEvent | ArtifactUpdateEvent;

export interface JsonRpcRequest<T = unknown> {
  jsonrpc: '2.0';
  id?: string | number | null;
  method: string;
  params?: T;
}

export interface JsonRpcError {
  code: number;
  message: string;
  data?: unknown;
}

export interface JsonRpcResponse<T = unknown> {
  jsonrpc: '2.0';
  id?: string | number | null;
  result?: T;
  error?: JsonRpcError;
}

export interface SendParams {
  message: Message;
  configuration?: Record<string, unknown>;
  pushNotificationConfig?: {
    url: string;
    token?: string;
  };
}

export interface GetTaskParams {
  taskId: string;
  contextId?: string;
}

export interface CancelTaskParams {
  taskId: string;
  reason?: string;
}

export interface SearchServicesResultHit {
  id: string;
  score: number;
  agentName: string;
  serviceId?: string;
  presence: AgentPresence;
  service?: AgentService;
  agentUrl?: string;
  fields?: Record<string, unknown>;
}

export interface SearchServicesResponse {
  engine: string;
  query: string;
  page: number;
  totalHits: number;
  hits: SearchServicesResultHit[];
}

export type TrustVerdict = 'trusted' | 'explore_recommended' | 'cautious' | 'vetoed_kill_switch';

export interface TrustEdge {
  fromAgent: string;
  toAgent: string;
  goma: number;
  plomo: number;
  recomGoma: number;
  recomPlomo: number;
  lastInteraction: string;
}

export interface GlobalTrustMetrics {
  score: number;
  gomaTotal: number;
  plomoTotal: number;
  connections: number;
  ratio: number;
}

export interface DirectTrustHistory {
  hasHistory: boolean;
  gomaLocal: number;
  plomoLocal: number;
  localScore?: number;
  killSwitchActive: boolean;
}

export interface NetworkVouching {
  trustedPeersCount: number;
  samplePeers: string[];
  transitiveScore: number;
}

export interface PersonalizedTrust {
  directInteractions: DirectTrustHistory;
  networkVouching: NetworkVouching;
  credibilityPercent: number;
  verdict: TrustVerdict;
  killSwitchActive: boolean;
}

export interface TrustEvaluation {
  target: string;
  perspectiveFrom?: string;
  globalMetrics: GlobalTrustMetrics;
  personalizedTrust?: PersonalizedTrust;
}
