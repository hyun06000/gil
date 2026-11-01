// Where the product keeps a Project's record, derived from the product contract rather than
// guessed: src/project_state.rs::state_for_root and the Windows fixture helper
// tests/mcp.rs::product_state_path. macOS/Linux: <root>/.gil/state.yaml. Windows: the private
// app-data store LOCALAPPDATA\GIL\project-state-v1\<sha256 of the UTF-16LE canonical root>\.gil\state.yaml,
// where "canonical" is what Rust's canonicalize returns (verbatim \\?\ prefix included); an
// existing external record wins, otherwise an existing inline record, otherwise the external path.
import { createHash } from 'node:crypto';
import { existsSync, realpathSync, statSync } from 'node:fs';
import { join, dirname } from 'node:path';

export const STATE_RELATIVE = ['.gil', 'state.yaml'];
export const EXTERNAL_LAYOUT = 'project-state-v1';

/** Rust `Path::canonicalize` spelling on Windows: `\\?\C:\...` or `\\?\UNC\server\share\...`. */
export function windowsCanonical(root) {
  const real = realpathSync.native(root);
  if (real.startsWith('\\\\?\\')) return real;
  if (real.startsWith('\\\\')) return `\\\\?\\UNC\\${real.slice(2)}`;
  return `\\\\?\\${real}`;
}

export function externalStorageRoot(root, localAppData) {
  const identity = Buffer.from(windowsCanonical(root), 'utf16le');
  const key = createHash('sha256').update(identity).digest('hex');
  return join(localAppData, 'GIL', EXTERNAL_LAYOUT, key);
}

/**
 * Resolve the record path for `root`. `env` is the Core child's environment (not the parent's),
 * `platform` defaults to process.platform. Mirrors state_for_root's preference order.
 */
export function projectStatePath(root, { env = process.env, platform = process.platform } = {}) {
  const inline = join(root, ...STATE_RELATIVE);
  if (platform !== 'win32') return { path: inline, layout: 'inline' };
  if (!env.LOCALAPPDATA) throw new Error('LOCALAPPDATA is not set for the Core child; the Windows record location cannot be resolved');
  const external = join(externalStorageRoot(root, env.LOCALAPPDATA), ...STATE_RELATIVE);
  const isFile = (p) => { try { return statSync(p).isFile(); } catch { return false; } };
  if (isFile(external) || existsSync(dirname(external))) return { path: external, layout: 'private_app_data' };
  if (isFile(inline)) return { path: inline, layout: 'inline' };
  return { path: external, layout: 'private_app_data' };
}
