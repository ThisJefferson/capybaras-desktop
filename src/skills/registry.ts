/**
 * The skill registry.
 *
 * ONE declarative source of truth for what the agent can do, replacing three
 * lists that could disagree: the headline table in the gate, the tool-owner
 * table in the sidecar, and the tier knowledge in the classifier.
 *
 * The governing rule (D20):
 *
 *   **Every skill must declare how it can hurt you, and a skill that forgets to
 *   will not build.**
 *
 * The reason is the failure mode this project keeps hitting: a lookup that
 * returns nothing and is silently skipped, producing something that looks almost
 * right. The same shape of bug in a skill table would produce a tool that is
 * *almost safe*. So the fields are required and `validateRegistry` refuses.
 */

import type { RiskTier } from '../risk-classifier';

/**
 * The plain-language headline. `many` is used when more than one thing is
 * affected and may contain `{n}`.
 *
 * Rule: never a tool identifier. The card is read by someone who does not know
 * what `db.delete` means, and the whole promise is that they do not have to.
 */
export interface SkillHeadline {
  one: string;
  many?: string;
}

export interface Skill {
  /** Dotted tool id, e.g. `fs.delete`. */
  tool: string;
  headline: SkillHeadline;
  /** Which capybara owns this work. Must be a real agent id. */
  owner: string;
  /** The input may be written by an adversary. Drives the taint discussion. */
  readsUntrusted: boolean;
  /** The action leaves this machine. */
  leavesMachine: boolean;
  /** Whether it can be undone. Irreversible actions are never rememberable. */
  reversible: boolean;
  /** A larger affectedCount should escalate the tier. */
  blastRadiusSensitive: boolean;
  /**
   * The MINIMUM tier. Escalation can only raise it, never lower it.
   *
   * Set conservatively. This is a safety net for a future classifier gap, not a
   * place to encode current behaviour — so every floor here is at or below what
   * the classifier already decides, which makes it inert today and load-bearing
   * the moment someone adds a skill the classifier does not know.
   */
  floor: RiskTier;
}

/**
 * Every tool the agent can use. Ordered for reviewability, not for behaviour.
 */
export const SKILLS: readonly Skill[] = Object.freeze([
  // ---- perception: reads and listings -----------------------------------
  { tool: 'fs.read',            headline: { one: 'Read a file', many: 'Read {n} files' },                 owner: 'bia',  readsUntrusted: true,  leavesMachine: false, reversible: true,  blastRadiusSensitive: true,  floor: 'silent' },
  { tool: 'fs.list',            headline: { one: 'Look at your files' },                                 owner: 'bia',  readsUntrusted: false, leavesMachine: false, reversible: true,  blastRadiusSensitive: false, floor: 'silent' },
  { tool: 'web.fetch',          headline: { one: 'Fetch a page' },                                       owner: 'bia',  readsUntrusted: true,  leavesMachine: false, reversible: true,  blastRadiusSensitive: false, floor: 'silent' },
  { tool: 'web.search',         headline: { one: 'Search the web' },                                     owner: 'bia',  readsUntrusted: true,  leavesMachine: false, reversible: true,  blastRadiusSensitive: false, floor: 'silent' },

  // ---- creation: the originals are untouched ----------------------------
  { tool: 'fs.create',          headline: { one: 'Create a file', many: 'Create {n} files' },            owner: 'nina', readsUntrusted: false, leavesMachine: false, reversible: true,  blastRadiusSensitive: true,  floor: 'notify' },
  { tool: 'fs.mkdir',           headline: { one: 'Create a folder' },                                    owner: 'nina', readsUntrusted: false, leavesMachine: false, reversible: true,  blastRadiusSensitive: false, floor: 'notify' },

  // ---- mutation: changes something that already existed -----------------
  { tool: 'fs.write',           headline: { one: 'Change a file', many: 'Change {n} files' },            owner: 'nina', readsUntrusted: false, leavesMachine: false, reversible: true,  blastRadiusSensitive: true,  floor: 'confirm' },
  { tool: 'fs.edit',            headline: { one: 'Change a file', many: 'Change {n} files' },            owner: 'nina', readsUntrusted: false, leavesMachine: false, reversible: true,  blastRadiusSensitive: true,  floor: 'confirm' },
  { tool: 'fs.move',            headline: { one: 'Move a file' },                                        owner: 'nina', readsUntrusted: false, leavesMachine: false, reversible: true,  blastRadiusSensitive: true,  floor: 'confirm' },
  { tool: 'fs.rename',          headline: { one: 'Rename a file' },                                      owner: 'nina', readsUntrusted: false, leavesMachine: false, reversible: true,  blastRadiusSensitive: false, floor: 'confirm' },
  { tool: 'fs.truncate',        headline: { one: 'Empty a file' },                                       owner: 'nina', readsUntrusted: false, leavesMachine: false, reversible: false, blastRadiusSensitive: true,  floor: 'confirm' },
  { tool: 'db.update',          headline: { one: 'Change a record', many: 'Change {n} records' },        owner: 'zeca', readsUntrusted: false, leavesMachine: false, reversible: true,  blastRadiusSensitive: true,  floor: 'confirm' },
  { tool: 'exec',               headline: { one: 'Run a command' },                                      owner: 'joca', readsUntrusted: false, leavesMachine: false, reversible: false, blastRadiusSensitive: false, floor: 'confirm' },

  // ---- the irreversible -------------------------------------------------
  { tool: 'fs.delete',          headline: { one: 'Delete a file', many: 'Delete {n} files' },            owner: 'zeca', readsUntrusted: false, leavesMachine: false, reversible: false, blastRadiusSensitive: true,  floor: 'confirm' },
  { tool: 'db.delete',          headline: { one: 'Delete a record', many: 'Delete {n} records' },        owner: 'zeca', readsUntrusted: false, leavesMachine: false, reversible: false, blastRadiusSensitive: true,  floor: 'confirm' },

  // ---- leaving the machine ---------------------------------------------
  { tool: 'message.send',       headline: { one: 'Send a message' },                                     owner: 'tuca', readsUntrusted: false, leavesMachine: true,  reversible: false, blastRadiusSensitive: true,  floor: 'confirm' },
  { tool: 'mail.send',          headline: { one: 'Send an email' },                                      owner: 'tuca', readsUntrusted: false, leavesMachine: true,  reversible: false, blastRadiusSensitive: true,  floor: 'confirm' },
  { tool: 'net.post',           headline: { one: 'Send something out' },                                 owner: 'tuca', readsUntrusted: false, leavesMachine: true,  reversible: false, blastRadiusSensitive: false, floor: 'confirm' },
  { tool: 'net.put',            headline: { one: 'Upload something' },                                   owner: 'tuca', readsUntrusted: false, leavesMachine: true,  reversible: false, blastRadiusSensitive: false, floor: 'confirm' },
  { tool: 'net.delete',         headline: { one: 'Delete something remotely' },                          owner: 'tuca', readsUntrusted: false, leavesMachine: true,  reversible: false, blastRadiusSensitive: false, floor: 'confirm' },

  // ---- money ------------------------------------------------------------
  { tool: 'spend',              headline: { one: 'Spend money' },                                        owner: 'zeca', readsUntrusted: false, leavesMachine: false, reversible: false, blastRadiusSensitive: true,  floor: 'confirm' },
  { tool: 'payment.send',       headline: { one: 'Send a payment' },                                     owner: 'zeca', readsUntrusted: false, leavesMachine: true,  reversible: false, blastRadiusSensitive: true,  floor: 'confirm' },

  // ---- credentials and the agent's own behaviour ------------------------
  { tool: 'credentials.read',   headline: { one: 'Read your saved passwords' },                          owner: 'zeca', readsUntrusted: false, leavesMachine: false, reversible: true,  blastRadiusSensitive: false, floor: 'confirm' },
  { tool: 'credentials.write',  headline: { one: 'Change your saved passwords' },                        owner: 'zeca', readsUntrusted: false, leavesMachine: false, reversible: false, blastRadiusSensitive: false, floor: 'confirm' },
  { tool: 'config.security',    headline: { one: 'Change your security settings' },                      owner: 'joca', readsUntrusted: false, leavesMachine: false, reversible: false, blastRadiusSensitive: false, floor: 'confirm' },
  { tool: 'agent.selfModify',   headline: { one: 'Change how it works' },                                owner: 'joca', readsUntrusted: false, leavesMachine: false, reversible: false, blastRadiusSensitive: false, floor: 'confirm' },
]);

const BY_TOOL = new Map(SKILLS.map((skill) => [skill.tool, skill]));

/** The declared skill, or `undefined` for a tool nobody has declared. */
export function skillFor(tool: string): Skill | undefined {
  return BY_TOOL.get(tool);
}

/** The owning capybara, or `undefined` if the skill is undeclared. */
export function ownerFor(tool: string): string | undefined {
  return BY_TOOL.get(tool)?.owner;
}

/**
 * The plain-language headline for an action. Falls back to naming the tool in
 * quotes rather than inventing a sentence, so an undeclared tool is visible in
 * the interface rather than disguised.
 */
export function headlineFor(tool: string, affectedCount = 1): string {
  const skill = BY_TOOL.get(tool);
  if (!skill) return `Do something with "${tool}"`;
  if (affectedCount > 1 && skill.headline.many) {
    return skill.headline.many.replace('{n}', String(affectedCount));
  }
  return skill.headline.one;
}

const TIER_ORDER: readonly RiskTier[] = ['silent', 'notify', 'confirm', 'hard_gate'];

/** The more cautious of two tiers. Escalation can only ever raise. */
export function moreCautious(a: RiskTier, b: RiskTier): RiskTier {
  return TIER_ORDER.indexOf(a) >= TIER_ORDER.indexOf(b) ? a : b;
}

/** The agent ids a skill may name. Duplicated deliberately so this module has
 *  no dependency on the sidecar, which imports *this*. */
const KNOWN_OWNERS = new Set(['tuca', 'bia', 'zeca', 'nina', 'joca', 'duda']);

/**
 * Check the registry itself. Returns a list of problems; empty means valid.
 *
 * This is the whole point of the manifest: a skill that has not declared how it
 * can hurt you must not be able to ship. Validation is a test AND a build step,
 * so silence cannot pass.
 */
export function validateRegistry(skills: readonly Skill[] = SKILLS): string[] {
  const problems: string[] = [];
  const seen = new Set<string>();

  for (const [index, skill] of skills.entries()) {
    if (skill === null || skill === undefined || typeof skill !== 'object') {
      problems.push(`skill at index ${index}: not an object`);
      continue;
    }

    // `tool` is used as a key and as a string below, so it is checked once and
    // then guarded. An earlier version pushed "missing tool" and then went on
    // to call .includes() on undefined -- it THREW instead of reporting. A
    // validator that crashes on malformed input is close to useless, because
    // malformed input is the only thing it exists to handle.
    const toolUsable = typeof skill.tool === 'string' && skill.tool.length > 0;
    const where = toolUsable ? `skill "${skill.tool}"` : `skill at index ${index}`;

    if (!toolUsable) {
      problems.push(`${where}: missing tool, or not a string`);
    } else if (seen.has(skill.tool)) {
      problems.push(`${where}: duplicate tool`);
    }
    if (toolUsable) seen.add(skill.tool);

    // --- headline: required, and must not leak a tool identifier ---
    if (!skill.headline || !skill.headline.one) {
      problems.push(`${where}: missing headline.one`);
    } else if (
      toolUsable &&
      skill.tool.includes('.') &&
      skill.headline.one.toLowerCase().includes(skill.tool.toLowerCase())
    ) {
      // Only DOTTED ids are identifiers a person must never see. A single-word
      // tool id like `spend` may legitimately be an English word inside its own
      // headline ("Spend money"), and flagging that is the check being wrong
      // rather than the skill being wrong.
      problems.push(`${where}: headline.one names the tool, which the card must never show`);
    }
    if (skill.headline?.many && !skill.headline.many.includes('{n}')) {
      problems.push(`${where}: headline.many is present but has no {n} placeholder`);
    }

    // --- owner: required, and must be a real capybara ---
    if (!skill.owner) problems.push(`${where}: missing owner`);
    else if (!KNOWN_OWNERS.has(skill.owner)) problems.push(`${where}: unknown owner "${skill.owner}"`);

    // --- the safety declarations: required, and required to be booleans ---
    for (const field of ['readsUntrusted', 'leavesMachine', 'reversible', 'blastRadiusSensitive'] as const) {
      if (typeof skill[field] !== 'boolean') problems.push(`${where}: ${field} must be declared as a boolean`);
    }

    // --- floor: required, and a real tier ---
    if (!skill.floor) problems.push(`${where}: missing floor`);
    else if (!TIER_ORDER.includes(skill.floor)) problems.push(`${where}: unknown floor "${skill.floor}"`);

    // --- combinations that are suspicious on their face ---
    if (skill.leavesMachine && skill.floor === 'silent') {
      problems.push(`${where}: leaves the machine but floors at silent`);
    }
    if (skill.leavesMachine && skill.readsUntrusted) {
      // Not an error in itself, but it is the exfiltration combination and
      // should be a deliberate decision rather than an oversight.
      problems.push(`${where}: reads untrusted input AND leaves the machine — confirm this is intended`);
    }
  }

  return problems;
}
