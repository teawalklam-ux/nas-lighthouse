import type { InternetState, LinkState } from "../lib/types";
import { StatusGlyph } from "./StatusGlyph";

type Tone = "ok" | "warn" | "bad" | "unknown";

const labels: Record<Tone, string> = {
  ok: "已连接",
  warn: "重连中",
  bad: "已断联",
  unknown: "未知",
};

function linkTone(state: LinkState): Tone {
  if (state === "connected") return "ok";
  if (state === "reconnecting") return "warn";
  return "bad";
}

function netTone(state: InternetState): Tone {
  if (state === "up") return "ok";
  if (state === "down") return "bad";
  return "unknown";
}

export function StatusDot({
  kind,
  state,
  title,
  compact = false,
}: {
  kind: "link" | "internet";
  state: LinkState | InternetState;
  title: string;
  compact?: boolean;
}) {
  const tone = kind === "link" ? linkTone(state as LinkState) : netTone(state as InternetState);
  return (
    <span className={`dot-wrap tone-${tone}${compact ? " compact" : ""}`} title={title}>
      <span className="dot" />
      {compact ? null : <StatusGlyph kind={kind === "link" ? "nas" : "internet"} />}
      {compact ? null : <span>{kind === "link" ? "本机↔NAS" : "NAS 出站"} · {labels[tone]}</span>}
    </span>
  );
}
