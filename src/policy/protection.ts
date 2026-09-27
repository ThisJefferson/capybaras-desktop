/**
 * Protection policy: freezes and off-limits targets.
 *
 * THE HOLE THIS CLOSES. Until now the `protectedTarget` flag came from the
 * action descriptor — that is, from the caller. So the "declared code and action
 * freeze" that failed to stop the July 2025 database deletion was in exactly the
 * same position: a request the agent could simply not make. A protection that
 * the protected agent supplies is not a protection.
 *
 * So protection is now resolved from a POLICY, and the invariant is one-way:
 *
 *   **effective = (what the action claims) OR (what the policy says)**
 *
 * An action can ADD caution. It can never remove it. Passing
 * `protectedTarget: false` does nothing if the policy matches — the same
 * escalation-only rule the tiers already follow.
 *
 * READ THE LIMITS IN THE DOC COMMENT BELOW. This is a real improvement and not
 * yet a complete answer, and the difference matters.
 */

/** A freeze, as declared by an operator. */
export interface FreezeDeclaration {
  active: boolean;
  /** Shown to the person in the card. Say why, not just that. */
  reason?: string;
  declaredBy?: string;
  declaredAt?: string;
}

export interface ProtectionPolicy {
  freeze?: FreezeDeclaration;
  /**
   * Off-limits targets, as glob patterns. Matched against the target label and
   * against every string in the action's arguments, so `*production*` catches a
   * path, a table name, or a label.
   */
  protectedTargets?: string[];
  /** Tools that are always gated, whatever their arguments claim. */
  protectedTools?: string[];
}

export interface ProtectionResult {
  /** The action touches something off-limits. */
  protectedTarget: boolean;
  /** A freeze is in force. */
  freezeActive: boolean;
  /** Why, in words suitable for the card. Empty when nothing applies. */
  reasons: string[];
}

/** A policy that protects everything. Used when the policy cannot be trusted. */
const PARANOID: ProtectionPolicy = Object.freeze({
  freeze: {
    active: true,
    reason: 'the protection policy could not be read, so everything is being treated as protected',
  },
  protectedTargets: ['**'],
});

/**
 * Turn a glob into a regular expression.
 *
 * Deliberately minimal: `*` matches within a segment, `**` matches across them.
 * A richer glob dialect would be more expressive and more to get wrong, and this
 * is a safety control — predictable beats clever.
 */
function globToRegExp(pattern: string): RegExp {
  const escaped = pattern.replace(/[.+^${}()|[\]\\]/g, '\\$&');
  const body = escaped
    .replace(/\*\*/g, '\u0000')
    .replace(/\*/g, '[^/\\\\]*')
    .replace(/\u0000/g, '.*');
  return new RegExp(`^${body}$`, 'i');
}

/** Every string worth matching: the label, and the string values in the args. */
function candidates(action: unknown, targetLabel?: string): string[] {
  const found: string[] = [];
  if (typeof targetLabel === 'string') found.push(targetLabel);

  const args = (action as { args?: unknown } | undefined)?.args;
  if (args && typeof args === 'object') {
    for (const value of Object.values(args as Record<string, unknown>)) {
      if (typeof value === 'string') found.push(value);
    }
  }
  return found;
}

/**
 * Resolve the protection that actually applies to an action.
 *
 * `claimedProtected` is whatever the action asserted. It is OR'd with the
 * policy, never substituted for it.
 */
export function resolveProtection(
  action: unknown,
  policy: ProtectionPolicy | undefined,
  // `| undefined` is spelled out on purpose: the project runs with
  // exactOptionalPropertyTypes, under which `targetLabel?: string` means "may be
  // ABSENT", not "may be undefined" -- so a caller forwarding an optional value
  // straight through is a type error rather than a no-op.
  options: { targetLabel?: string | undefined; claimedProtected?: boolean | undefined } = {},
): ProtectionResult {
  const reasons: string[] = [];
  let protectedTarget = options.claimedProtected === true;

  if (protectedTarget) reasons.push('this target is marked off-limits');

  const tool = (action as { tool?: unknown } | undefined)?.tool;
  const toolName = typeof tool === 'string' ? tool : '';

  // ---- protected tools -------------------------------------------------
  if (policy?.protectedTools?.some((entry) => entry === toolName)) {
    protectedTarget = true;
    reasons.push('this action is on the protected list');
  }

  // ---- protected targets -----------------------------------------------
  const haystack = candidates(action, options.targetLabel);
  for (const pattern of policy?.protectedTargets ?? []) {
    const matches = haystack.some((value) => globToRegExp(pattern).test(value));
    if (matches) {
      protectedTarget = true;
      reasons.push('this target is off-limits');
      break; // one reason is enough; a wall of duplicates teaches nothing
    }
  }

  // ---- a declared freeze -----------------------------------------------
  const freezeActive = policy?.freeze?.active === true;
  if (freezeActive) {
    reasons.push(policy?.freeze?.reason ? `a freeze is in effect: ${policy.freeze.reason}` : 'a freeze is in effect');
  }

  return { protectedTarget, freezeActive, reasons: [...new Set(reasons)] };
}

export interface LoadedPolicy {
  policy: ProtectionPolicy;
  /** Set when the file existed but could not be trusted. */
  problem?: string;
}

/**
 * Read the policy file.
 *
 * FAILS CLOSED, and this is the point: a policy that exists but cannot be read
 * returns a policy that protects everything, so a corrupt or maliciously
 * truncated file can never silently DISABLE a freeze. An absent file is a
 * different case — that is a machine with no policy, which is legitimate, and is
 * reported so nobody mistakes it for a policy that is in force.
 *
 * LIMITS, STATED PLAINLY. This is a real improvement and not a complete answer:
 *
 *  - The file lives in the state directory, which is also where writable state
 *    lives. An agent with file access could tamper with it. Truly protecting it
 *    needs OS-level permissions, or a location outside the agent's reach — still
 *    to do.
 *  - Nothing here is signed, so the policy's provenance cannot be proven.
 *  - Freezes last until the file says otherwise; there is no expiry yet.
 */
export function loadProtectionPolicy(readFile: (path: string) => string, path: string): LoadedPolicy {
  let raw: string;
  try {
    raw = readFile(path);
  } catch (error) {
    const code = (error as NodeJS.ErrnoException).code;
    // No file is a legitimate machine with no policy, not a failure.
    if (code === 'ENOENT') return { policy: {} };
    return { policy: PARANOID, problem: `could not read the protection policy (${code ?? 'unknown error'})` };
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return { policy: PARANOID, problem: 'the protection policy is not valid JSON — treating everything as protected' };
  }

  if (parsed === null || typeof parsed !== 'object') {
    return { policy: PARANOID, problem: 'the protection policy is not an object — treating everything as protected' };
  }

  const candidate = parsed as ProtectionPolicy;
  const shapeProblems: string[] = [];
  if (candidate.freeze !== undefined && typeof candidate.freeze !== 'object') {
    shapeProblems.push('freeze is not an object');
  }
  for (const field of ['protectedTargets', 'protectedTools'] as const) {
    const value = candidate[field];
    if (value !== undefined && (!Array.isArray(value) || value.some((v) => typeof v !== 'string'))) {
      shapeProblems.push(`${field} must be an array of strings`);
    }
  }

  if (shapeProblems.length > 0) {
    return { policy: PARANOID, problem: `the protection policy is malformed (${shapeProblems.join('; ')})` };
  }

  return { policy: candidate };
}
