import { useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ago } from "../lib/format";
import { adaptBaiduToken, adaptQuarkCookie } from "../lib/driveCredential";
import type { DriveMount } from "../lib/types";
import { StatusGlyph, type StatusGlyphKind } from "./StatusGlyph";

const STATE_LABEL: Record<string, string> = {
  up: "正常",
  down: "失效",
  missing: "未挂载",
  unknown: "未检查",
};

const BAIDU_GUIDE_URL = "https://alist.nn.ci/zh/guide/drivers/baidu.html";
const BAIDU_TOKEN_URL = "https://api.oplist.org/";

function stateClass(state: string): string {
  if (state === "up") return "tone-ok";
  if (state === "down" || state === "missing") return "tone-bad";
  return "tone-unknown";
}

const FALLBACK: DriveMount[] = [
  { id: "quark", label: "夸克", mountPath: "/夸克", state: "unknown", status: "", message: "尚未检查", checkedTs: 0, latencyMs: null },
  { id: "baidu", label: "百度", mountPath: "/百度", state: "unknown", status: "", message: "尚未检查", checkedTs: 0, latencyMs: null },
];

function orderedMounts(mounts: DriveMount[]): DriveMount[] {
  return FALLBACK.map((fallback) => mounts.find((item) => item.id === fallback.id) ?? fallback);
}

function TextLink({ href, children }: { href: string; children: ReactNode }) {
  return (
    <button type="button" className="text-link" onClick={() => void openUrl(href)}>
      {children}
    </button>
  );
}

export function DriveMounts({
  mounts,
  onChange,
  children,
  trailing,
}: {
  mounts: DriveMount[];
  onChange: (next: DriveMount[]) => void;
  children?: ReactNode;
  trailing?: ReactNode;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState<DriveMount | null>(null);
  const [credential, setCredential] = useState("");
  const [adaptNote, setAdaptNote] = useState<string | null>(null);
  const rows = orderedMounts(mounts);
  const quark = editing?.id === "quark";

  function applyCredential(next: string) {
    if (!editing) return;
    const adapted = quark ? adaptQuarkCookie(next) : adaptBaiduToken(next);
    setCredential(adapted.value);
    setAdaptNote(adapted.note);
  }

  async function remount() {
    if (!editing) return;
    const value = credential.trim();
    if (!value) {
      setError(quark ? "请粘贴夸克 Cookie" : "请粘贴百度 refresh_token");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      onChange(await invoke<DriveMount[]>("remount_drive", { kind: editing.id, credential: value }));
      setCredential("");
      setAdaptNote(null);
      setEditing(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <div className="dots-stack">
        {children}
        {rows.map((item) => (
          <span
            key={item.id}
            role="button"
            className={`dot-wrap is-action ${stateClass(item.state)}`}
            title={
              item.checkedTs
                ? `${item.message} · ${ago(item.checkedTs)}`
                : item.message
            }
            onClick={() => {
              if (busy) return;
              setError(null);
              setCredential("");
              setAdaptNote(null);
              setEditing(item);
            }}
          >
            <span className="dot" />
            <StatusGlyph kind={item.id as StatusGlyphKind} />
            <span>
              {item.label} · {STATE_LABEL[item.state] ?? item.state}
            </span>
          </span>
        ))}
        {trailing}
      </div>
      {editing
        ? createPortal(
            <div
              className="modal-backdrop"
              onMouseDown={() => {
                if (!busy) { setCredential(""); setAdaptNote(null); setEditing(null); }
              }}
            >
              <form
                className="card modal drive-remount"
                onMouseDown={(event) => event.stopPropagation()}
                onSubmit={(event) => {
                  event.preventDefault();
                  void remount();
                }}
              >
                <h2>更新{editing.label}凭据</h2>
                <p className="muted">目标存储：{editing.mountPath}。请先在后端创建对应网盘，停用的存储不会被自动启用。</p>
                {quark ? (
                  <div className="drive-remount-help">
                    <p>AList 需要 Cookie 头，一行即可，例如：</p>
                    <code>ctoken=值; __puus=值; __uid=值</code>
                    <p>
                      也可直接粘贴 Netscape cookies.txt（以
                      {" "}
                      <code className="inline"># Netscape HTTP Cookie File</code>
                      {" "}
                      开头、用 Tab 分隔的那种），或浏览器扩展导出的 JSON。粘贴后会自动转换成 Cookie 头。
                    </p>
                    <p>凭据会发送到你配置的 AList 并保存于该存储；本客户端不将凭据写入配置文件。</p>
                  </div>
                ) : (
                  <div className="drive-remount-help">
                    <p>需要百度网盘的 refresh_token，不是账号密码。按下面做：</p>
                    <ol>
                      <li>
                        打开获取地址{" "}
                        <TextLink href={BAIDU_TOKEN_URL}>api.oplist.org</TextLink>
                      </li>
                      <li>网盘名称选「百度网盘 OOB 验证」</li>
                      <li>点获取 token，完成百度登录授权</li>
                      <li>把得到的 refresh_token 粘贴到下方。整段 JSON 也可以，会自动提取。</li>
                    </ol>
                    <p>
                      教程：
                      <TextLink href={BAIDU_GUIDE_URL}>AList 百度网盘说明</TextLink>
                    </p>
                    <p>
                      获取地址：
                      <TextLink href={BAIDU_TOKEN_URL}>{BAIDU_TOKEN_URL}</TextLink>
                    </p>
                    <p>凭据会发送到你配置的 AList 并保存于该存储；本客户端不将凭据写入配置文件。</p>
                  </div>
                )}
                {adaptNote && <p className="adapt-note">{adaptNote}</p>}
                {error && <p className="banner bad">{error}</p>}
                <label>
                  {quark ? "Cookie" : "refresh_token"}
                  <textarea
                    value={credential}
                    rows={quark ? 8 : 4}
                    autoFocus
                    spellCheck={false}
                    placeholder={quark ? "在此粘贴 Cookie 头、cookies.txt 或 JSON" : "在此粘贴 refresh_token"}
                    onChange={(event) => applyCredential(event.target.value)}
                  />
                </label>
                <div className="nav-actions">
                  <button type="button" className="btn" disabled={busy} onClick={() => { setCredential(""); setAdaptNote(null); setEditing(null); }}>
                    取消
                  </button>
                  <button type="submit" className="btn" disabled={busy}>
                    {busy ? "更新中…" : "更新凭据"}
                  </button>
                </div>
              </form>
            </div>,
            document.body,
          )
        : null}
    </>
  );
}
