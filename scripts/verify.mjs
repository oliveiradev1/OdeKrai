import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const TAURI_DIR = path.join(ROOT, 'src-tauri');
const npm = process.platform === 'win32' ? 'npm.cmd' : 'npm';

export const verificationSteps = [
  { name: 'build', command: npm, args: ['run', 'build'], cwd: ROOT },
  { name: 'testes de interface', command: npm, args: ['test'], cwd: ROOT },
  { name: 'format:check', command: npm, args: ['run', 'format:check'], cwd: ROOT },
  { name: 'check:encoding', command: npm, args: ['run', 'check:encoding'], cwd: ROOT },
  { name: 'check:version', command: npm, args: ['run', 'check:version'], cwd: ROOT },
  { name: 'check:debug-command', command: npm, args: ['run', 'check:debug-command'], cwd: ROOT },
  {
    name: 'check:blocking-commands',
    command: npm,
    args: ['run', 'check:blocking-commands'],
    cwd: ROOT,
  },
  {
    name: 'check:tauri-window-commands',
    command: npm,
    args: ['run', 'check:tauri-window-commands'],
    cwd: ROOT,
  },
  { name: 'check:window-api-acl', command: npm, args: ['run', 'check:window-api-acl'], cwd: ROOT },
  {
    name: 'check:release-features',
    command: npm,
    args: ['run', 'check:release-features'],
    cwd: ROOT,
  },
  { name: 'check:mcp-catalog', command: npm, args: ['run', 'check:mcp-catalog'], cwd: ROOT },
  { name: 'cargo fmt --check', command: 'cargo', args: ['fmt', '--check'], cwd: TAURI_DIR },
  {
    name: 'cargo clippy --all-targets --features mock-window-tests -- -D warnings',
    command: 'cargo',
    args: ['clippy', '--all-targets', '--features', 'mock-window-tests', '--', '-D', 'warnings'],
    cwd: TAURI_DIR,
  },
  { name: 'cargo test --lib', command: 'cargo', args: ['test', '--lib'], cwd: TAURI_DIR },
  {
    name: 'cargo test --test mock_window --features mock-window-tests',
    command: 'cargo',
    args: ['test', '--test', 'mock_window', '--features', 'mock-window-tests'],
    cwd: TAURI_DIR,
  },
];

function executeStep(step) {
  return spawnSync(step.command, step.args, {
    cwd: step.cwd,
    stdio: 'inherit',
    shell: process.platform === 'win32' && step.command.endsWith('.cmd'),
  });
}

export function runVerification(execute = executeStep, report = console.error) {
  for (const step of verificationSteps) {
    const commandText = [step.command, ...step.args].join(' ');
    report(`[verify] Iniciando: ${step.name}`);
    const result = execute(step);
    if (result.error || result.status !== 0) {
      report(`[verify] FALHOU: ${step.name} (${commandText})`);
      if (result.error) report(`[verify] Erro ao iniciar: ${result.error.message}`);
      else if (result.signal) report(`[verify] Processo encerrado pelo sinal ${result.signal}.`);
      else report(`[verify] Código de saída: ${result.status}`);
      return 1;
    }
  }
  report('[verify] Todas as etapas passaram.');
  return 0;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = runVerification();
}
