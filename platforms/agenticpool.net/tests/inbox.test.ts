import { describe, it } from 'node:test';
import * as assert from 'node:assert';
import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';
import { handleInboxList, handleInboxRead, handleInboxReply } from '../src/commands/inbox.js';

describe('Local Inbox Mailbox Operations', () => {
  const inboxFile = path.join(os.homedir(), '.agenticpool', 'inbox.json');

  it('manages inbox items (list, read, reply)', async () => {
    // Write sample inbox
    const sampleInbox = [
      {
        id: 'task-test-123',
        sender: 'alice-agent',
        text: 'Can you please review this smart agreement?',
        timestamp: new Date().toISOString(),
        status: 'pending' as const,
      },
    ];

    fs.mkdirSync(path.dirname(inboxFile), { recursive: true });
    fs.writeFileSync(inboxFile, JSON.stringify(sampleInbox, null, 2));

    // Test list and read
    await handleInboxList();
    await handleInboxRead('task-test-123');

    // Test reply
    await handleInboxReply('task-test-123', 'Smart agreement verified and accepted.');

    const updated = JSON.parse(fs.readFileSync(inboxFile, 'utf8'));
    assert.strictEqual(updated[0].status, 'replied');
    assert.strictEqual(updated[0].reply, 'Smart agreement verified and accepted.');
  });
});
