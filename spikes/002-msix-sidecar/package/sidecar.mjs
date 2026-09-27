// SPIKE CODE - throwaway.
//
// Bundled inside the MSIX package. Writes a proof file so we can confirm it ran
// from WindowsApps, under full trust, with a real filesystem and cwd.

import { writeFileSync } from 'node:fs';

const proof = process.argv.find((a) => a.startsWith('--proof='))?.split('=')[1];

const info = {
  ran: true,
  nodeVersion: process.version,
  execPath: process.execPath,
  cwd: process.cwd(),
  platform: process.platform,
  scriptDir: import.meta.url,
  localAppData: process.env.LOCALAPPDATA ?? null,
  userProfile: process.env.USERPROFILE ?? null,
  argv: process.argv,
  time: new Date().toISOString(),
};

if (proof) writeFileSync(proof, JSON.stringify(info, null, 2));

console.log(`sidecar ran under full-trust MSIX: ${process.version}`);
console.log(`  execPath: ${process.execPath}`);
console.log(`  cwd     : ${process.cwd()}`);
