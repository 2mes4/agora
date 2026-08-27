import { A2aEvent } from './types.js';

/**
 * Parses an SSE (Server-Sent Events) stream chunk by chunk and yields A2aEvent objects.
 */
export async function* parseSseStream(
  stream: ReadableStream<Uint8Array> | AsyncIterable<Uint8Array>
): AsyncGenerator<A2aEvent, void, unknown> {
  const decoder = new TextDecoder('utf-8');
  let buffer = '';

  const iterable = isAsyncIterable(stream) ? stream : readableStreamToAsyncIterable(stream);

  for await (const chunk of iterable) {
    buffer += typeof chunk === 'string' ? chunk : decoder.decode(chunk, { stream: true });
    
    // Normalize newlines
    buffer = buffer.replace(/\r\n/g, '\n').replace(/\r/g, '\n');

    let delimiterIndex: number;
    while ((delimiterIndex = buffer.indexOf('\n\n')) !== -1) {
      const block = buffer.slice(0, delimiterIndex);
      buffer = buffer.slice(delimiterIndex + 2);

      const event = parseEventBlock(block);
      if (event) {
        yield event;
      }
    }
  }

  // Flush remaining buffer if any
  if (buffer.trim().length > 0) {
    const event = parseEventBlock(buffer);
    if (event) {
      yield event;
    }
  }
}

function parseEventBlock(block: string): A2aEvent | null {
  const lines = block.split('\n');
  let dataStr = '';
  let eventType = '';

  for (const line of lines) {
    if (line.startsWith(':')) {
      // SSE comment
      continue;
    }
    if (line.startsWith('event:')) {
      eventType = line.slice(6).trim();
    } else if (line.startsWith('data:')) {
      const dataContent = line.slice(5).trim();
      dataStr += (dataStr ? '\n' : '') + dataContent;
    }
  }

  if (!dataStr) {
    return null;
  }

  try {
    const parsed = JSON.parse(dataStr);
    // If parsed object doesn't have kind, set it from eventType if present
    if (eventType && !parsed.kind) {
      parsed.kind = eventType;
    }
    return parsed as A2aEvent;
  } catch {
    return null;
  }
}

function isAsyncIterable(obj: unknown): obj is AsyncIterable<Uint8Array> {
  return Symbol.asyncIterator in Object(obj);
}

async function* readableStreamToAsyncIterable(
  stream: ReadableStream<Uint8Array>
): AsyncGenerator<Uint8Array, void, unknown> {
  const reader = stream.getReader();
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      if (value) yield value;
    }
  } finally {
    reader.releaseLock();
  }
}

/**
 * Format an A2aEvent into an SSE string payload.
 */
export function formatSseEvent(event: A2aEvent): string {
  const kind = event.kind;
  const json = JSON.stringify(event);
  return `event: ${kind}\ndata: ${json}\n\n`;
}
