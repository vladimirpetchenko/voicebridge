import { useEffect, useRef, useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import Markdown from "../../../shared/ui/Markdown";

/// Сворачиваемый блок размышлений (reasoning) модели.
export function ReasoningBlock({ text, streaming }: { text: string; streaming: boolean }) {
  const [open, setOpen] = useState(streaming);
  const prevStreaming = useRef(streaming);
  useEffect(() => {
    if (prevStreaming.current && !streaming) setOpen(false);
    prevStreaming.current = streaming;
  }, [streaming]);

  return (
    <div className="reasoning-block">
      <button
        type="button"
        className="reasoning-toggle"
        onClick={() => setOpen((o) => !o)}
      >
        {open ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        <span>{streaming ? "Размышляет…" : "Размышление"}</span>
      </button>
      {open && (
        <div className="reasoning-text">
          <Markdown>{text}</Markdown>
        </div>
      )}
    </div>
  );
}
