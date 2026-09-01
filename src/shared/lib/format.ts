/// Утилиты форматирования чисел/строк для UI.

export function formatMb(mb: number): string {
  if (mb >= 1000) return `${(mb / 1000).toFixed(1)} ГБ`;
  return `${mb} МБ`;
}

export function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return `${n}`;
}

export function formatCost(c: number): string {
  if (!c || c <= 0) return "$0.00";
  return `$${c.toFixed(c < 0.01 ? 4 : 2)}`;
}

export function prettifyModel(id: string): string {
  if (!id) return "";
  return id.charAt(0).toUpperCase() + id.slice(1);
}

export function relTime(ms: number): string {
  if (!ms) return "";
  const diff = Date.now() - ms;
  if (diff < 60000) return "только что";
  const m = Math.floor(diff / 60000);
  if (m < 60) return `${m} мин назад`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h} ч назад`;
  return `${Math.floor(h / 24)} дн назад`;
}

/// Продолжительность в читаемом виде: `12с`, `1м 05с`.
export function formatDuration(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  if (s < 60) return `${s}с`;
  const m = Math.floor(s / 60);
  const rem = s % 60;
  return `${m}м ${String(rem).padStart(2, "0")}с`;
}
