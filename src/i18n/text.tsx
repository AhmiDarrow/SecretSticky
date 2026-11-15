import type { ReactNode } from "react";

/** Honor a single `<strong>` pair. User text never goes through this. */
export function RichText({ text }: { text: string }) {
  const parts = text.split(/(<\/?strong>)/g);
  const nodes: ReactNode[] = [];
  let strong = false;
  parts.forEach((part, index) => {
    if (part === "<strong>") {
      strong = true;
      return;
    }
    if (part === "</strong>") {
      strong = false;
      return;
    }
    if (!part) return;
    nodes.push(strong ? <strong key={index}>{part}</strong> : part);
  });
  return <>{nodes}</>;
}

/** Split a catalog string on `{name}` and inject React nodes (not HTML). */
export function Template({
  text,
  slots,
}: {
  text: string;
  slots: Record<string, ReactNode>;
}) {
  const pattern = /\{([A-Za-z0-9_]+)\}/g;
  const nodes: ReactNode[] = [];
  let last = 0;
  let match: RegExpExecArray | null;
  let index = 0;
  while ((match = pattern.exec(text))) {
    if (match.index > last) nodes.push(text.slice(last, match.index));
    const name = match[1];
    nodes.push(<span key={index}>{slots[name] ?? match[0]}</span>);
    index += 1;
    last = match.index + match[0].length;
  }
  if (last < text.length) nodes.push(text.slice(last));
  return <>{nodes}</>;
}
