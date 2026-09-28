/**
 * The herd.
 *
 * Six capybaras, one character design, duplicated (D18). They communicate the
 * multi-agent architecture: which specialist is doing what, and whether one of
 * them needs a human.
 *
 * Three rules, all from BRAND.md:
 *
 *   1. **"needs you" is the loudest thing in the interface.** One agent may be
 *      in that state. It is never a badge or a dot.
 *   2. **Never more than two agents look busy at once**, or the interface reads
 *      as chaos. A third contender pushes the least-recently-busy one back to
 *      listening.
 *   3. **The herd is always visible.** Stillness is information: nothing is
 *      happening, and that is a fact you can see.
 *
 * This module is pure state. It never touches the protocol or the filesystem.
 */

export type AgentState = 'dozing' | 'listening' | 'working' | 'needs-you';

export interface AgentDescriptor {
  /** Stable id, used in messages. */
  id: string;
  /** Display name. */
  label: string;
  /** What this one does, in plain words. */
  job: string;
  /** Token name for the accent colour. Coral is never assigned here. */
  accent: string;
}

export interface AgentSnapshot extends AgentDescriptor {
  state: AgentState;
  /** Milliseconds since this agent last changed state. */
  sinceMs: number;
}

/**
 * The roster, in fixed display order. Order matters: each has a permanent slot,
 * so the row is learnable spatially even though the six look identical.
 */
export const HERD: readonly AgentDescriptor[] = Object.freeze([
  { id: 'tuca', label: 'Tuca', job: 'the one you talk to', accent: 'atlantic' },
  { id: 'bia', label: 'Bia', job: 'looks things up', accent: 'sky' },
  { id: 'zeca', label: 'Zeca', job: 'checks the work', accent: 'ink' },
  { id: 'nina', label: 'Nina', job: 'writes and edits files', accent: 'leaf' },
  { id: 'joca', label: 'Joca', job: 'runs commands', accent: 'capy' },
  { id: 'duda', label: 'Duda', job: 'tidies and organises', accent: 'sand' },
]);

import { ownerFor } from '../../../../src/skills/registry';

/**
 * Who owns this tool.
 *
 * The mapping used to live here as a TOOL_OWNER record. It is now the skill
 * registry (src/skills/registry.ts), so ownership, the plain-language headline
 * and the tier floor all come from one list rather than three that could
 * silently disagree (D20).
 *
 * Unknown tools fall to Duda, who tidies up -- the same fail-safe direction as
 * the gate's "unknown means ask".
 */
export function agentForTool(tool: string): string {
  return ownerFor(tool) ?? 'duda';
}

/** How many agents may look busy at once. BRAND.md rule 2. */
const MAX_BUSY = 2;

export class Herd {
  private readonly states = new Map<string, AgentState>();
  private readonly changedAt = new Map<string, number>();

  constructor(private readonly clock: () => number = Date.now) {
    this.rest();
  }

  /** Everything awake and attentive, nothing busy. The idle state. */
  rest(): void {
    const now = this.clock();
    for (const agent of HERD) {
      this.states.set(agent.id, 'listening');
      this.changedAt.set(agent.id, now);
    }
  }

  private set(id: string, state: AgentState): void {
    if (this.states.get(id) === state) return;
    this.states.set(id, state);
    this.changedAt.set(id, this.clock());
  }

  /**
   * Mark the owner of `tool` as working, enforcing the at-most-two-busy rule.
   * Returns the agent id that took the work.
   */
  beginWork(tool: string): string {
    const owner = agentForTool(tool);

    // "needs you" is the loudest thing in the interface, and NOTHING may quietly
    // overwrite it -- including the same agent starting another task.
    //
    // The loop below only guarded OTHER agents, so an action owned by the agent
    // who was already waiting for a human lowered its OWN sign: the approval was
    // still pending, and the herd stopped saying so. Card and herd then
    // contradicted each other, which is worse than either being wrong alone.
    if (this.states.get(owner) === 'needs-you') return owner;

    // An agent in "needs you" keeps that state: it is the loudest thing and a
    // new task must not silently overwrite it.
    for (const agent of HERD) {
      if (agent.id !== owner && this.states.get(agent.id) === 'needs-you') {
        // Someone is still waiting on a human; do not start counting.
        this.set(owner, 'working');
        return owner;
      }
    }

    const busy = HERD.filter((a) => this.states.get(a.id) === 'working' && a.id !== owner);
    if (busy.length >= MAX_BUSY) {
      // Push the one that has been busy longest back to listening.
      const oldest = busy.reduce((a, b) =>
        (this.changedAt.get(a.id) ?? 0) <= (this.changedAt.get(b.id) ?? 0) ? a : b,
      );
      this.set(oldest.id, 'listening');
    }

    this.set(owner, 'working');
    return owner;
  }

  /** One agent raises the sign. Clears any other "needs you". */
  needsYou(agentId: string): void {
    for (const agent of HERD) {
      if (agent.id !== agentId && this.states.get(agent.id) === 'needs-you') {
        this.set(agent.id, 'listening');
      }
    }
    this.set(agentId, 'needs-you');
  }

  /**
   * Work finished: back to attentive.
   *
   * It deliberately does NOT clear "needs you". Finishing a task and answering
   * an approval are different events, and a raised sign means a human is still
   * being waited on -- so only `standDown` lowers it. (An earlier version had
   * `release` do both, which let a background task switch off the loudest thing
   * in the interface while its approval card was still on screen.)
   */
  release(agentId: string): void {
    if (this.states.get(agentId) === 'working') this.set(agentId, 'listening');
  }

  /** A human answered: the sign can come down. */
  standDown(agentId: string): void {
    if (this.states.get(agentId) === 'needs-you') this.set(agentId, 'listening');
  }

  /** The whole herd, in fixed display order, ready to send. */
  snapshot(): AgentSnapshot[] {
    const now = this.clock();
    return HERD.map((agent) => ({
      ...agent,
      state: this.states.get(agent.id) ?? 'listening',
      sinceMs: now - (this.changedAt.get(agent.id) ?? now),
    }));
  }
}
