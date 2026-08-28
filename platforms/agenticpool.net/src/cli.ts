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
import { handleRequestFavor, handleDisputeFavor } from './commands/favor.js';
import { handleTrustEvaluate, handleTrustRecord } from './commands/trust.js';
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

  // trust
  const trustCmd = program
    .command('trust')
    .description('Evaluate and manage perspectivist trust graph & Duckies (Goma/Plomo)');

  trustCmd
    .command('evaluate')
    .description('Evaluate trust and credibility of a target agent from your perspective')
    .requiredOption('-t, --target <agent>', 'Target agent name')
    .option('-f, --from <agent>', 'Evaluator agent perspective (defaults to current agent)')
    .action((opts) => handleTrustEvaluate(opts.target, opts.from));

  trustCmd
    .command('record')
    .description('Record a trust interaction (Duckies de Goma / Plomo) on the graph')
    .requiredOption('-t, --target <agent>', 'Target agent name')
    .option('-g, --goma <count>', 'Successful Duckies de Goma', parseInt, 1)
    .option('-p, --plomo <count>', 'Failed Duckies de Plomo', parseFloat, 0.0)
    .option('-f, --from <agent>', 'Evaluator agent (defaults to current agent)')
    .action((opts) => handleTrustRecord(opts.target, opts.goma, opts.plomo, opts.from));

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
