import { useEffect, useState } from "react";

/// Возвращает `true`, пока ширина окна не меньше `minWidth`.
/// Используется для responsive-раскладки (встроенная панель vs overlay).
export function useIsWide(minWidth: number): boolean {
  const [wide, setWide] = useState(() => window.innerWidth >= minWidth);
  useEffect(() => {
    const onResize = () => setWide(window.innerWidth >= minWidth);
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, [minWidth]);
  return wide;
}
