import { AgoraClient } from './client.js';
import { A2aEvent, Message, Part, SendParams, Task } from './types.js';

export class DelegateBuilder {
  private _message?: Message;
  private _configuration?: Record<string, unknown>;
  private _pushConfig?: { url: string; token?: string };
  private _bearerToken?: string;
  private _sender?: string;

  constructor(
    private client: AgoraClient,
    private targetUrl: string
  ) {}

  /**
   * Set text message.
   */
  message(textOrMessage: string | Message): this {
    if (typeof textOrMessage === 'string') {
      this._message = {
        role: 'user',
        parts: [{ kind: 'text', text: textOrMessage }],
      };
    } else {
      this._message = textOrMessage;
    }
    return this;
  }

  /**
   * Set structured message parts.
   */
  parts(...parts: Part[]): this {
    this._message = {
      role: 'user',
      parts,
    };
    return this;
  }

  /**
   * Attach a context URI.
   */
  contextUri(uri: string): this {
    if (!this._message) {
      this._message = { role: 'user', parts: [] };
    }
    this._message.contextUri = uri;
    return this;
  }

  /**
   * Set execution configuration.
   */
  configuration(config: Record<string, unknown>): this {
    this._configuration = config;
    return this;
  }

  /**
   * Configure push notifications for task updates.
   */
  pushNotifications(url: string, token?: string): this {
    this._pushConfig = { url, token };
    return this;
  }

  /**
   * Set bearer token for authentication.
   */
  auth(token: string): this {
    this._bearerToken = token;
    return this;
  }

  /**
   * Set the declared sender agent name.
   */
  sender(name: string): this {
    this._sender = name;
    return this;
  }

  private buildParams(): SendParams {
    if (!this._message) {
      throw new Error('Message is required before sending delegation request');
    }
    return {
      message: this._message,
      configuration: this._configuration,
      pushNotificationConfig: this._pushConfig,
    };
  }

  /**
   * Execute unary request (message/send).
   */
  async send(): Promise<Task> {
    const params = this.buildParams();
    return this.client.send(this.targetUrl, params.message, {
      configuration: params.configuration,
      pushNotificationConfig: params.pushNotificationConfig,
      bearerToken: this._bearerToken,
      sender: this._sender,
    });
  }

  /**
   * Execute streaming request (message/stream).
   */
  stream(): AsyncIterable<A2aEvent> {
    const params = this.buildParams();
    return this.client.stream(this.targetUrl, params.message, {
      configuration: params.configuration,
      pushNotificationConfig: params.pushNotificationConfig,
      bearerToken: this._bearerToken,
      sender: this._sender,
    });
  }
}
