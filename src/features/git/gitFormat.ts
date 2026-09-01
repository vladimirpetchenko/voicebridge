import {
  FileMinus,
  FilePlus,
  FileX,
  FolderGit,
  type LucideIcon,
} from "lucide-react";
import type { GitFileChange } from "../../shared/types";

/// Вспомогательные функции для Git-панели изменений.

export function basename(p: string): string {
  const parts = p.split(/[\\/]/);
  return parts[parts.length - 1] || p;
}

export function dirname(p: string): string {
  const i = Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\"));
  return i >= 0 ? p.slice(0, i) : "";
}

export interface GitGroup {
  dir: string;
  name: string;
  changes: GitFileChange[];
  additions: number;
  deletions: number;
}

/// Группирует изменённые файлы по папкам. Файлы в корне репозитория идут
/// в группу с пустым `dir` (без заголовка папки).
export function groupChangesByDir(changes: GitFileChange[]): GitGroup[] {
  const map = new Map<string, GitFileChange[]>();
  for (const c of changes) {
    const dir = dirname(c.path);
    const list = map.get(dir) ?? [];
    list.push(c);
    map.set(dir, list);
  }
  const dirs = [...map.keys()].sort((a, b) => {
    if (a === "") return -1;
    if (b === "") return 1;
    return a.localeCompare(b);
  });
  return dirs.map((dir) => {
    const items = (map.get(dir) ?? []).sort((a, b) => a.path.localeCompare(b.path));
    const additions = items.reduce((s, c) => s + c.additions, 0);
    const deletions = items.reduce((s, c) => s + c.deletions, 0);
    return {
      dir,
      name: dir === "" ? "" : basename(dir),
      changes: items,
      additions,
      deletions,
    };
  });
}

export function gitStatusMeta(status: string): { label: string; Icon: LucideIcon; cls: string } {
  switch (status) {
    case "added":
      return { label: "добавлен", Icon: FilePlus, cls: "added" };
    case "deleted":
      return { label: "удалён", Icon: FileX, cls: "deleted" };
    case "untracked":
      return { label: "новый", Icon: FilePlus, cls: "untracked" };
    case "renamed":
      return { label: "переименован", Icon: FolderGit, cls: "renamed" };
    default:
      return { label: "изменён", Icon: FileMinus, cls: "modified" };
  }
}

export interface DiffRow {
  old: string;
  neu: string;
  cls: "add" | "del" | "hunk" | "meta" | "ctx";
  text: string;
}

/// Разбирает unified diff на строки с номерами (до/после) и типом для подсветки.
export function parseDiff(diff: string): DiffRow[] {
  const rows: DiffRow[] = [];
  let oldLine = 0;
  let newLine = 0;
  for (const line of diff.split("\n")) {
    if (line.startsWith("@@")) {
      const m = line.match(/@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
      if (m) {
        oldLine = parseInt(m[1], 10);
        newLine = parseInt(m[2], 10);
      }
      rows.push({ old: "", neu: "", cls: "hunk", text: line });
    } else if (
      line.startsWith("diff") ||
      line.startsWith("index") ||
      line.startsWith("new file") ||
      line.startsWith("deleted file") ||
      line.startsWith("similarity") ||
      line.startsWith("rename") ||
      line.startsWith("---") ||
      line.startsWith("+++") ||
      line.startsWith("\\ No newline")
    ) {
      rows.push({ old: "", neu: "", cls: "meta", text: line });
    } else if (line.startsWith("+")) {
      rows.push({ old: "", neu: String(newLine++), cls: "add", text: line });
    } else if (line.startsWith("-")) {
      rows.push({ old: String(oldLine++), neu: "", cls: "del", text: line });
    } else {
      rows.push({ old: String(oldLine++), neu: String(newLine++), cls: "ctx", text: line });
    }
  }
  return rows;
}
