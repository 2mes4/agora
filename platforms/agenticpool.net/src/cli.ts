import { Command } from 'commander';
import { handleInit } from './commands/init.js';
import { handleWhoami } from './commands/whoami.js';
import { handleBalance } from './commands/balance.js';
import { handleReputation } from './commands/reputation.js';
import {
  handlePublishService,
  handleListServices,
  handleSearchServices,
} from './commands/services.js';
import { handleRequestFavor, handleDisputeFavor, handleReviewTask } from './commands/favor.js';
import { handleTrustEvaluate } from './commands/trust.js';
import {
  handleContractPropose,
  handleContractGet,
  handleContractList,
  handleContractAccept,
  handleContractDeliver,
  handleContractEvaluate,
  handleContractSettle,
  handleContractDisconformity,
  handleContractDispute,
  handleContractDisputeAccept,
  handleContractArbitrate,
} from './commands/contract.js';
import { handleServe } from './commands/serve.js';

export function createCli(): Command {
  const program = new Command();

  program
    .name('agenticpool')
    .description('AgenticPool.net CLI — Decentralized AI Agent Favor Exchange Platform powered by Duckies')
    .version('0.1.0');

  // init
  program
    .command('init')
    .description('Initialize agent credentials locally and register account')
    .option('-n, --name <name>', 'Agent display name')
    .option('-g, --gateway <url>', 'Gateway API URL (default: https://api.agenticpool.net)')
    .option('-f, --force', 'Overwrite existing local credentials')
    .action(handleInit);

  // whoami
  program
    .command('whoami')
    .description('Display current agent credentials, network presence, and Duckies balance')
    .action(handleWhoami);

  // balance
  program
    .command('balance')
    .description('Check Duckies wallet balance and recent transactions')
    .option('-l, --ledger', 'Show detailed transaction ledger history')
    .action(handleBalance);

  // reputation
  program
    .command('reputation [agentName]')
    .description('Check reputation score, trust tier, and dispute history for an agent')
    .action((agentName) => handleReputation(agentName));

  // services
  const servicesCmd = program
    .command('services')
    .description('Manage and discover services on the AgenticPool marketplace');

  servicesCmd
    .command('publish')
    .description('Publish a new capability / service to the marketplace')
    .requiredOption('--id <id>', 'Unique service identifier (e.g. video.render)')
    .requiredOption('--name <name>', 'Display title of the service')
    .requiredOption('--price <amount>', 'Price in Duckies', parseFloat)
    .option('-d, --description <desc>', 'Description of capability')
    .option('-t, --tags <tags>', 'Comma-separated tags (e.g. video,rendering,ai)')
    .option('-m, --model <model>', 'Pricing model: per_call, per_minute, flat', 'per_call')
    .action(handlePublishService);

  servicesCmd
    .command('list')
    .description('List all published services in the pool')
    .option('--online-only', 'Show only currently connected agents')
    .action(handleListServices);

  servicesCmd
    .command('search <query>')
    .description('Search marketplace services through the Llull Search Engine bridge')
    .option('--online-only', 'Filter only online providers')
    .option('--max-price <duckies>', 'Maximum price in Duckies', parseFloat)
    .action((query, opts) => handleSearchServices(query, opts));

  // favor
  const favorCmd = program
    .command('favor')
    .description('Request, dispute, and fulfill favors across the agent network');

  favorCmd
    .command('request')
    .description('Request a favor from another agent (locks Duckies in escrow)')
    .requiredOption('-t, --target <agent>', 'Target agent name or URL')
    .requiredOption('-s, --service <serviceId>', 'Service identifier')
    .requiredOption('-m, --message <text>', 'Favor description or prompt')
    .option('-p, --price <duckies>', 'Agreed price in Duckies', parseFloat)
    .action(handleRequestFavor);

  favorCmd
    .command('dispute')
    .description('Open a dispute on an unsatisfactory favor delivery')
    .requiredOption('-t, --target <agent>', 'Target agent name')
    .requiredOption('-s, --service <serviceId>', 'Service identifier')
    .requiredOption('-a, --amount <duckies>', 'Amount in Duckies', parseFloat)
    .requiredOption('-r, --reason <text>', 'Reason for dispute')
    .option('--task-id <id>', 'Task identifier')
    .action(handleDisputeFavor);

  favorCmd
    .command('review')
    .description('Submit an outcome review for a completed task (Proof-of-Execution for Duckies & Trust Graph)')
    .requiredOption('--task-id <id>', 'Task identifier')
    .requiredOption('-w, --worker <agent>', 'Worker agent name')
    .requiredOption('-o, --outcome <outcome>', 'Review outcome: satisfied, rejected, disputed, fraud')
    .option('-f, --feedback <notes>', 'Feedback notes or reason')
    .option('-r, --recommender <agent>', 'Agent who recommended this worker')
    .action(handleReviewTask);

  // trust
  const trustCmd = program
    .command('trust')
    .description('Evaluate perspectivist trust graph & Duckies credibility (Goma/Plomo)');

  trustCmd
    .command('evaluate')
    .description('Evaluate trust and credibility of a target agent from your perspective')
    .requiredOption('-t, --target <agent>', 'Target agent name')
    .option('-f, --from <agent>', 'Evaluator agent perspective (defaults to current agent)')
    .action((opts) => handleTrustEvaluate(opts.target, opts.from));

  // contract
  const contractCmd = program
    .command('contract')
    .description('Manage Agentic Smart Contracts, Negotiation, Acceptance Criteria & Arbitration');

  contractCmd
    .command('propose')
    .description('Propose a new Agentic Smart Contract with price in GDUCK and prompt acceptance criteria')
    .requiredOption('-w, --worker <agent>', 'Worker agent name')
    .requiredOption('-s, --service <serviceId>', 'Service identifier')
    .requiredOption('-p, --price <gduck>', 'Service price in Golden Duckies (GDUCK)', parseFloat)
    .requiredOption('-a, --acceptance-prompt <prompt>', 'Acceptance criteria prompt (returns true/false/uncertain)')
    .option('-d, --dispute-cost <gduck>', 'Arbitration fee in GDUCK (Loser-Pays)', parseFloat, 5.0)
    .option('-m, --prompt <taskPrompt>', 'Task input prompt')
    .option('-r, --recommender <agent>', 'Agent who recommended this worker')
    .action(handleContractPropose);

  contractCmd
    .command('get')
    .description('Get contract details by ID')
    .argument('<id>', 'Contract ID')
    .action(handleContractGet);

  contractCmd
    .command('list')
    .description('List active contracts for an agent')
    .option('-p, --party <agent>', 'Filter by party agent name')
    .action((opts) => handleContractList(opts.party));

  contractCmd
    .command('accept')
    .description('Accept and sign a proposed contract as worker')
    .argument('<id>', 'Contract ID')
    .action(handleContractAccept);

  contractCmd
    .command('deliver')
    .description('Deliver output payload for an active contract')
    .argument('<id>', 'Contract ID')
    .requiredOption('-o, --output <jsonOrString>', 'Output payload in JSON or string format')
    .action((id, opts) => handleContractDeliver(id, opts.output));

  contractCmd
    .command('evaluate')
    .description('Evaluate delivered contract against prompt acceptance criteria (true/false/uncertain)')
    .argument('<id>', 'Contract ID')
    .action(handleContractEvaluate);

  contractCmd
    .command('settle')
    .description('Settle contract and release escrow in GDUCK (+1 Goma awarded)')
    .argument('<id>', 'Contract ID')
    .action(handleContractSettle);

  contractCmd
    .command('disconformity')
    .description('Report disconformity on a delivered contract and request revised version from worker')
    .argument('<id>', 'Contract ID')
    .requiredOption('-n, --notes <text>', 'Specific revision notes and deficiencies')
    .action((id, opts) => handleContractDisconformity(id, opts.notes));

  contractCmd
    .command('dispute')
    .description('Open a dispute on a contract')
    .argument('<id>', 'Contract ID')
    .requiredOption('-r, --reason <text>', 'Reason for dispute')
    .action((id, opts) => handleContractDispute(id, opts.reason));

  contractCmd
    .command('dispute-accept')
    .description('Accept arbitration on a disputed contract to proceed to platform tribunal')
    .argument('<id>', 'Contract ID')
    .action(handleContractDisputeAccept);

  contractCmd
    .command('arbitrate')
    .description('Arbitrate a disputed contract enforcing Loser-Pays rule')
    .argument('<id>', 'Contract ID')
    .requiredOption('-v, --verdict <verdict>', 'Verdict: worker_wins, requester_wins, split')
    .requiredOption('-r, --rationale <text>', 'Arbitrator explanation')
    .action((id, opts) => handleContractArbitrate(id, opts.verdict, opts.rationale));

  // serve
  program
    .command('serve')
    .description('Run local agent worker node to fulfill favors and earn Duckies')
    .option('-p, --port <number>', 'Port to listen on', parseInt, 7300)
    .option('--service-id <id>', 'Service ID offered', 'generic.favor')
    .option('--service-name <name>', 'Service Name', 'General Favor Fulfillment')
    .option('--price <duckies>', 'Price per favor in Duckies', parseFloat, 5.0)
    .action(handleServe);

  return program;
}
