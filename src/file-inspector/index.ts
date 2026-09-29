/**
 * Structural inspection — the public surface.
 *
 * The sidecar imports from here (`src/file-inspector`), never from `inspect.ts`
 * directly, so the module can grow without the call sites moving. Same shape as
 * `src/risk-classifier`.
 */
export type {
  DeclaredKind,
  FileKind,
  Finding,
  Hazard,
  Inspection,
} from './inspect';

export {
  MAX_ARCHIVE_DEPTH,
  MAX_ARCHIVE_ENTRIES,
  MAX_FILE_BYTES,
  SCAN_WINDOW_BYTES,
  declaredKind,
  describeHazard,
  detectKind,
  inspect,
} from './inspect';

export type { FileActionDescription, FileActionInput, FileActionKind } from './actions';
export { describeFileAction } from './actions';
