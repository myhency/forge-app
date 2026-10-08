import { mkdir, mkdtemp, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { isAbsolute, join } from "node:path";

export const SKILLS_PLUGIN_NAME = "forge-skills";

export interface SkillsPlugin {
  /** The generated plugin directory, to pass to the SDK as a local plugin. */
  path: string;
  /** The `forge-skills:<name>` entries for the SDK's `skills` option. */
  skills: string[];
  /** Removes the generated directory (the symlinks, never what they point to). */
  cleanup: () => Promise<void>;
}

// A name becomes a directory entry, so it must be exactly one safe path component.
export function isSafeSkillName(name: string): boolean {
  return name.length > 0 && name !== "." && name !== ".." && !/[\\/\0]/.test(name);
}

// The SDK does not discover user skills under settingSources ['project'], and ['user', 'project']
// leaks the user's enabled plugins (spec §17.7). A local plugin is loaded either way, so the enabled
// skills are gathered into one as symlinks (junctions on Windows). Returns null when there is nothing safe to gather.
export async function buildSkillsPlugin(skills: Record<string, string>): Promise<SkillsPlugin | null> {
  // Skip a relative target (the link would resolve against the plugin directory) and a name that
  // only differs in case (a case-insensitive filesystem would fail the whole start with EEXIST).
  const seen = new Set<string>();
  const names = Object.keys(skills).filter((name) => {
    if (!isSafeSkillName(name) || !isAbsolute(skills[name])) return false;
    const folded = name.toLowerCase();
    if (seen.has(folded)) return false;
    seen.add(folded);
    return true;
  });
  if (names.length === 0) return null;
  const root = await mkdtemp(join(tmpdir(), "forge-skills-"));
  try {
    await mkdir(join(root, ".claude-plugin"));
    await writeFile(join(root, ".claude-plugin", "plugin.json"), JSON.stringify({ name: SKILLS_PLUGIN_NAME }));
    await mkdir(join(root, "skills"));
    for (const name of names) {
      // A directory symlink on Windows needs administrator rights or Developer Mode; a junction needs neither
      // (and the target is absolute, as a junction requires).
      await symlink(skills[name], join(root, "skills", name), process.platform === "win32" ? "junction" : "dir");
    }
  } catch (err) {
    await rm(root, { recursive: true, force: true });
    throw err;
  }
  return {
    path: root,
    skills: names.map((name) => `${SKILLS_PLUGIN_NAME}:${name}`),
    cleanup: () => rm(root, { recursive: true, force: true }),
  };
}
