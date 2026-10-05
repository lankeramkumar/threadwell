// Release gate: no plaintext secrets in files git tracks. Scans for common key formats and
// assignments of long literal values. Exits non-zero on any match. Heuristic, not exhaustive.
import { execFileSync } from 'node:child_process';
import { readFileSync, statSync } from 'node:fs';

const PATTERNS = [
  { name: 'OpenAI-style key', re: /\bsk-[A-Za-z0-9_-]{20,}/g },
  { name: 'Anthropic-style key', re: /\bsk-ant-[A-Za-z0-9_-]{20,}/g },
  { name: 'AWS access key id', re: /\bAKIA[0-9A-Z]{16}\b/g },
  { name: 'private key block', re: /-----BEGIN (RSA |EC |OPENSSH |)PRIVATE KEY-----/g },
  { name: 'assigned secret', re: /\b(api[_-]?key|secret|token|password)\b\s*[:=]\s*["'][^"'\s]{16,}["']/gi },
];

// Files whose job is to describe patterns or hold placeholders.
const ALLOWED = new Set(['scripts/check-secrets.mjs', '.env.example']);

const files = execFileSync('git', ['ls-files'], { encoding: 'utf8' })
  .split('\n')
  .filter(Boolean)
  .filter((f) => !ALLOWED.has(f));

const findings = [];
for (const file of files) {
  let stat;
  try {
    stat = statSync(file);
  } catch {
    continue;
  }
  if (!stat.isFile() || stat.size > 5 * 1024 * 1024) continue;
  const text = readFileSync(file, 'utf8');
  for (const { name, re } of PATTERNS) {
    re.lastIndex = 0;
    const match = re.exec(text);
    if (match) {
      const line = text.slice(0, match.index).split('\n').length;
      findings.push(`${file}:${line} ${name}`);
    }
  }
}

if (findings.length > 0) {
  console.error('Possible plaintext secrets found:');
  for (const f of findings) console.error(`  ${f}`);
  process.exit(1);
}
console.log(`No plaintext secrets matched in ${files.length} tracked files.`);
