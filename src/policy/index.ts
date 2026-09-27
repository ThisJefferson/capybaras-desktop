export type { Grant, GrantScope, RecordResult } from './grants';
export { GrantStore, DEFAULT_TTL_MS, describeGrant } from './grants';
export type { LoadResult } from './grant-file';
export { loadGrants, saveGrants } from './grant-file';
export type { FreezeDeclaration, ProtectionPolicy, ProtectionResult, LoadedPolicy } from './protection';
export { resolveProtection, loadProtectionPolicy, selfProtection, withSelfProtection, policyIsUserWritable } from './protection';

export type { GateContext, ApprovalRequest, GateDecision } from './gate';
export { gate } from './gate';
