export type AdaptedCredential = {
  value: string;
  note: string | null;
};

function isBoolFlag(value: string): boolean {
  return value === "TRUE" || value === "FALSE" || value === "true" || value === "false";
}

function lastWins(pairs: { name: string; value: string }[]): string {
  const cookies: { name: string; value: string }[] = [];
  for (const pair of pairs) {
    const existing = cookies.find((item) => item.name === pair.name);
    if (existing) existing.value = pair.value;
    else cookies.push(pair);
  }
  return cookies.map((item) => `${item.name}=${item.value}`).join("; ");
}

function parseNetscapeLine(line: string): { name: string; value: string } | null {
  const trimmed = line.trim();
  if (!trimmed || trimmed.startsWith("#")) return null;
  if (trimmed.includes("\t")) {
    const cols = trimmed.split("\t");
    if (cols.length >= 7 && cols[5]) return { name: cols[5], value: cols[6] ?? "" };
    return null;
  }
  const parts = trimmed.split(/\s+/);
  if (parts.length >= 7 && isBoolFlag(parts[1]) && isBoolFlag(parts[3]) && parts[5]) {
    return { name: parts[5], value: parts.slice(6).join(" ") };
  }
  return null;
}

function fromNetscape(text: string): string | null {
  const pairs = text
    .split(/\r?\n/)
    .map(parseNetscapeLine)
    .filter((item): item is { name: string; value: string } => item != null);
  if (pairs.length === 0) return null;
  return lastWins(pairs);
}

function fromCookieJson(text: string): string | null {
  try {
    const parsed: unknown = JSON.parse(text);
    if (!Array.isArray(parsed)) return null;
    const pairs: { name: string; value: string }[] = [];
    for (const item of parsed) {
      if (!item || typeof item !== "object") continue;
      const record = item as { name?: unknown; value?: unknown };
      if (typeof record.name !== "string" || !record.name) continue;
      pairs.push({
        name: record.name,
        value: typeof record.value === "string" ? record.value : "",
      });
    }
    if (pairs.length === 0) return null;
    return lastWins(pairs);
  } catch {
    return null;
  }
}

function looksLikeNetscape(text: string): boolean {
  if (/Netscape HTTP Cookie File/i.test(text)) return true;
  if (text.includes("\t")) return true;
  return text.split(/\r?\n/).some((line) => parseNetscapeLine(line) != null);
}

function stripCookiePrefix(text: string): string {
  return text.replace(/^Cookie:\s*/i, "").replace(/[\r\n]+/g, "");
}

export function adaptQuarkCookie(raw: string): AdaptedCredential {
  const trimmed = raw.trim();
  if (!trimmed) return { value: raw, note: null };

  const fromJson = fromCookieJson(trimmed);
  if (fromJson) {
    return { value: fromJson, note: `已识别 JSON Cookie，已转换为 Cookie 头（${fromJson.split("; ").length} 条）` };
  }

  if (looksLikeNetscape(trimmed)) {
    const header = fromNetscape(trimmed);
    if (header) {
      return { value: header, note: `已识别 Netscape Cookie 文件，已转换为 Cookie 头（${header.split("; ").length} 条）` };
    }
  }

  const header = stripCookiePrefix(trimmed);
  if (header !== raw) return { value: header, note: "已去掉换行和 Cookie 前缀" };
  return { value: raw, note: null };
}

function fromBaiduJson(text: string): string | null {
  try {
    const parsed: unknown = JSON.parse(text);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return null;
    const token = (parsed as { refresh_token?: unknown }).refresh_token;
    return typeof token === "string" && token.trim() ? token.trim() : null;
  } catch {
    return null;
  }
}

function fromBaiduLines(text: string): string | null {
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim().replace(/^["']|["']$/g, "");
    const matched = line.match(/^refresh_token\s*[:=]\s*["']?(.+?)["']?\s*,?$/i);
    if (matched?.[1] && matched[1].trim().length >= 16) return matched[1].trim();
  }
  return null;
}

export function adaptBaiduToken(raw: string): AdaptedCredential {
  const trimmed = raw.trim();
  if (!trimmed) return { value: raw, note: null };

  const fromJson = fromBaiduJson(trimmed);
  if (fromJson) return { value: fromJson, note: "已从 JSON 中提取 refresh_token" };

  const fromLines = fromBaiduLines(trimmed);
  if (fromLines && fromLines !== trimmed) {
    return { value: fromLines, note: "已提取 refresh_token" };
  }

  if (trimmed.includes("\n")) {
    return { value: trimmed.replace(/[\r\n]+/g, ""), note: null };
  }
  return { value: raw, note: null };
}
