import {
  FileText,
  Globe,
  Pencil,
  Search,
  Terminal,
  Wrench,
  type LucideIcon,
} from "lucide-react";

/// Иконка по имени инструмента OpenCode.
export function toolIcon(name: string): LucideIcon {
  const t = name.toLowerCase();
  if (t.includes("edit") || t.includes("write") || t.includes("patch")) return Pencil;
  if (t.includes("grep") || t.includes("search") || t.includes("glob")) return Search;
  if (t.includes("bash") || t.includes("shell") || t.includes("exec")) return Terminal;
  if (t.includes("read")) return FileText;
  if (t.includes("web") || t.includes("fetch")) return Globe;
  return Wrench;
}
