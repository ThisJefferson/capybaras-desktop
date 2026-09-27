export type { RiskTier, Severity, Taint } from './tiers';
export { TIER_LABEL, TIER_ORDER, severityOf, escalate, raise, atLeast } from './tiers';

export type { ActionDescriptor, Classification } from './classify';
export { classify, needsHuman } from './classify';
