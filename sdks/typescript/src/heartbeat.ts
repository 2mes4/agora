import { DirectoryClient } from './directory.js';
import { AgentStatus } from './types.js';

export class HeartbeatEmitter {
  private timer?: NodeJS.Timeout;
  private running = false;

  constructor(
    private directory: DirectoryClient,
    private agentName: string,
    private intervalMs: number = 30000,
    private status: AgentStatus = 'online'
  ) {}

  /**
   * Start sending periodic heartbeats.
   */
  start(): void {
    if (this.running) return;
    this.running = true;

    // Send initial heartbeat immediately
    this.sendBeat();

    this.timer = setInterval(() => {
      this.sendBeat();
    }, this.intervalMs);

    if (this.timer.unref) {
      this.timer.unref();
    }
  }

  /**
   * Stop sending heartbeats and mark agent offline.
   */
  async stop(): Promise<void> {
    this.running = false;
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = undefined;
    }
    try {
      await this.directory.heartbeat(this.agentName, 'offline');
    } catch {
      // Ignore network errors on shutdown
    }
  }

  /**
   * Update the status reported in heartbeats.
   */
  setStatus(status: AgentStatus): void {
    this.status = status;
    if (this.running) {
      this.sendBeat();
    }
  }

  private async sendBeat(): Promise<void> {
    try {
      await this.directory.heartbeat(this.agentName, this.status);
    } catch (err) {
      // Log or silently ignore heartbeat network errors
    }
  }
}
