import { useCallback, useEffect, useRef, useState } from "react";
import { api, formatBytes, kbToBytes, type DeviceStatus, type Prefs } from "./lib/api";
import { ImageTab } from "./components/ImageTab";
import { CardTab } from "./components/CardTab";

const DEFAULT_PREFS: Prefs = {
  rotation: 0,
  fit: "contain",
  padColor: "#000000",
  bootShowImage: true,
  noSleep: true,
  brightness: 75,
  removeOthers: true,
  bootLogo: "same",
  lastDir: null,
  margin: [0, 0, 0, 0],
};

type Tab = "image" | "card";

export default function App() {
  const [tab, setTab] = useState<Tab>("image");
  const [prefs, setPrefsState] = useState<Prefs>(DEFAULT_PREFS);
  const [status, setStatus] = useState<DeviceStatus | null>(null);
  const loaded = useRef(false);
  const lastConnected = useRef(false);
  const hadError = useRef(false);

  useEffect(() => {
    api
      .loadPrefs()
      .then((p) => setPrefsState({ ...DEFAULT_PREFS, ...p }))
      .catch(() => undefined)
      .finally(() => (loaded.current = true));
  }, []);

  const setPrefs = useCallback((patch: Partial<Prefs>) => {
    setPrefsState((p) => {
      const next = { ...p, ...patch };
      if (loaded.current) void api.savePrefs(next).catch(() => undefined);
      return next;
    });
  }, []);

  const refreshStatus = useCallback(async () => {
    try {
      const s = await api.status();
      hadError.current = !!s.error;
      setStatus(s);
    } catch {
      /* 何もしない */
    }
  }, []);

  // 2秒ごとに挿さっているか確認。つながった瞬間だけカードの容量を読む
  useEffect(() => {
    let alive = true;
    const tick = async () => {
      try {
        const p = await api.presence();
        if (!alive) return;
        if (p.connected && (!lastConnected.current || hadError.current)) {
          // つながった瞬間と、応答がない間は読み直す
          lastConnected.current = true;
          await refreshStatus();
        } else if (!p.connected) {
          lastConnected.current = false;
          setStatus({ ...p, storage: null, error: null, busy: false });
        }
      } catch {
        /* 何もしない */
      }
    };
    void tick();
    const t = setInterval(tick, 2000);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, [refreshStatus]);

  const connected = !!status?.connected;
  const st = status?.storage;
  const freeText = !connected
    ? "画面がつながっていません"
    : st && st.totalKb > 0
      ? `画面に挿したTFカード（空き ${formatBytes(kbToBytes(st.freeKb))}）`
      : "画面の内蔵メモリ";

  return (
    <div className="flex h-full flex-col">
      <header className="flex h-[52px] shrink-0 items-end justify-between border-b border-line px-6">
        <nav className="flex gap-7" role="tablist" aria-label="画面の切り替え">
          {(
            [
              ["image", "画像を入れる"],
              ["card", "TFカード"],
            ] as const
          ).map(([k, label]) => (
            <button
              key={k}
              type="button"
              role="tab"
              aria-selected={tab === k}
              onClick={() => setTab(k)}
              className={
                "relative pb-[11px] text-[14px] " +
                (tab === k
                  ? "font-bold text-ink after:absolute after:inset-x-[-2px] after:bottom-0 after:h-[3px] after:rounded-full after:bg-amber"
                  : "text-dim hover:text-ink2")
              }
            >
              {label}
            </button>
          ))}
        </nav>
        <ConnectionChip status={status} />
      </header>

      {tab === "image" ? (
        <ImageTab prefs={prefs} setPrefs={setPrefs} connected={connected} freeText={freeText} />
      ) : (
        <CardTab status={status} onRefresh={refreshStatus} />
      )}
    </div>
  );
}

function ConnectionChip({ status }: { status: DeviceStatus | null }) {
  let tone: "ok" | "warn" | "off" = "off";
  let text = "画面が見つかりません：USBを挿し直してください";
  if (status?.connected && status.error) {
    tone = "warn";
    text = status.busy ? "公式のTURZXソフトを閉じてください" : "画面が応答しません：USBを挿し直してください";
  } else if (status?.connected) {
    tone = "ok";
    text = "接続中　8.8インチ";
  } else if (status?.otherModel) {
    tone = "warn";
    text = "8.8インチ以外の画面です（未対応）";
  }
  const cls = {
    ok: "border-ok-line bg-ok-bg text-ok-ink",
    warn: "border-bad-line bg-bad-bg text-bad-ink",
    off: "border-line3 bg-raise text-mute",
  }[tone];
  const dot = { ok: "bg-ok-dot", warn: "bg-bad-line", off: "bg-faint" }[tone];
  return (
    <div
      className={"mb-[10px] flex h-7 items-center gap-2 rounded-full border px-4 text-[12.5px] " + cls}
      title={status?.error ?? undefined}
      aria-live="polite"
    >
      <span className={"h-[10px] w-[10px] rounded-full " + dot} />
      {text}
    </div>
  );
}
