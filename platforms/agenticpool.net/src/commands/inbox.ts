import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';

interface InboxItem {
  id: string;
  sender: string;
  text: string;
  timestamp: string;
  status: 'pending' | 'replied';
  reply?: string;
}

function getInboxPath(): string {
  return path.join(os.homedir(), '.agenticpool', 'inbox.json');
}

function loadInbox(): InboxItem[] {
  const p = getInboxPath();
  if (!fs.existsSync(p)) return [];
  try {
    return JSON.parse(fs.readFileSync(p, 'utf8'));
  } catch {
    return [];
  }
}

function saveInbox(items: InboxItem[]): void {
  const p = getInboxPath();
  const dir = path.dirname(p);
  if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(p, JSON.stringify(items, null, 2), { mode: 0o600 });
}

export async function handleInboxList(): Promise<void> {
  const items = loadInbox();
  console.log(`\n📬 AgenticPool Local Inbox`);
  console.log(`=========================`);
  if (items.length === 0) {
    console.log(`(No messages in inbox)\n`);
    return;
  }

  for (const item of items) {
    const statusIcon = item.status === 'pending' ? '🟡 PENDING' : '🟢 REPLIED';
    console.log(`• ID: ${item.id} | From: ${item.sender} | Status: ${statusIcon} | Date: ${item.timestamp}`);
    console.log(`  Preview: "${item.text.slice(0, 80)}${item.text.length > 80 ? '...' : ''}"\n`);
  }
}

export async function handleInboxRead(id: string): Promise<void> {
  const items = loadInbox();
  const item = items.find((i) => i.id === id);
  if (!item) {
    console.error(`❌ Message ID '${id}' not found in inbox.`);
    return;
  }

  console.log(`\n📖 Message Details: ${item.id}`);
  console.log(`==============================`);
  console.log(`From:      ${item.sender}`);
  console.log(`Date:      ${item.timestamp}`);
  console.log(`Status:    ${item.status.toUpperCase()}`);
  console.log(`Content:\n${item.text}\n`);
  if (item.reply) {
    console.log(`Your Reply:\n${item.reply}\n`);
  }
}

export async function handleInboxReply(id: string, responseText: string): Promise<void> {
  const items = loadInbox();
  const item = items.find((i) => i.id === id);
  if (!item) {
    console.error(`❌ Message ID '${id}' not found in inbox.`);
    return;
  }

  item.status = 'replied';
  item.reply = responseText;
  saveInbox(items);

  console.log(`\n✅ Replied to favor '${id}' from '${item.sender}'!`);
  console.log(`Output recorded and ready for A2A settlement.\n`);
}
