// 「TFカード」画面：画面内カードの容量と、PCのカードリーダーでの作り直し
import { useEffect, useState } from "react";
import { api, errText, formatBytes, kbToBytes, type CardDrive, type DeviceStatus } from "../lib/api";
import { GhostButton } from "./ui";

export function CardTab({ status, onRefresh }: { status: DeviceStatus | null; onRefresh: () => void }) {
  const [drives, setDrives] = useState<CardDrive[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);
  const [typed, setTyped] = useState("");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [log, setLog] = useState<string[] | null>(null);

  async function loadDrives() {
    try {
      const d = await api.listCards();
      setDrives(d);
      if (!d.some((x) => x.letter === selected)) setSelected(d[0]?.letter ?? null);
    } catch {
      setDrives([]);
    }
  }

  useEffect(() => {
    void loadDrives();
    const t = setInterval(loadDrives, 3000);
    return () => clearInterval(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const target = drives.find((d) => d.letter === selected) ?? null;
  const st = status?.storage;
  const usedPct = st && st.totalKb > 0 ? Math.max(2, Math.round((st.usedKb / st.totalKb) * 100)) : 0;

  async function runFormat() {
    if (!target) return;
    setBusy(true);
    setResult(null);
    try {
      setResult(await api.formatCard(target.letter, target.diskNumber));
    } catch (e) {
      setResult({ ok: false, message: errText(e) });
    } finally {
      setBusy(false);
      setConfirm(false);
      setTyped("");
      void loadDrives();
    }
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto px-6 pb-6 pt-5">
      <div className="grid grid-cols-[400px_1fr] gap-8">
        {/* 画面に挿してあるカード */}
        <section className="flex flex-col gap-3">
          <h2 className="m-0 text-[14px] font-bold">画面に挿してあるカード</h2>
          <div className="rounded-lg border border-line bg-raise px-[18px] py-4">
            {!status?.connected ? (
              <p className="m-0 text-[12.5px] text-mute">画面がつながっていません</p>
            ) : st && st.totalKb > 0 ? (
              <>
                <div className="flex items-baseline justify-between">
                  <span className="text-[12px] text-mute">容量</span>
                  <span className="num text-[13px]">{formatBytes(kbToBytes(st.totalKb))}</span>
                </div>
                <div className="mt-2 h-2 overflow-hidden rounded-full bg-line2">
                  <div className="h-full rounded-full bg-amber" style={{ width: `${usedPct}%` }} />
                </div>
                <div className="mt-3 flex items-center justify-between">
                  <span className="text-[11.5px] text-dim">
                    使用 {formatBytes(kbToBytes(st.usedKb))} ／ 空き {formatBytes(kbToBytes(st.freeKb))}
                  </span>
                  <GhostButton onClick={onRefresh} className="h-6 text-[11.5px]">
                    再読み込み
                  </GhostButton>
                </div>
              </>
            ) : (
              <div className="flex items-center justify-between gap-3">
                <p className="m-0 text-[12.5px] text-mute">
                  {status.error ?? "カードが見つかりません（画像は画面の内蔵メモリに入ります）"}
                </p>
                <GhostButton onClick={onRefresh} className="h-6 shrink-0 text-[11.5px]">
                  再読み込み
                </GhostButton>
              </div>
            )}
          </div>

          {/* 確認窓 */}
          {confirm && target && (
            <div
              role="dialog"
              aria-modal="true"
              aria-labelledby="confirm-title"
              className="mt-2 rounded-[10px] border border-bad-line bg-[#202726] px-5 py-4"
            >
              <div id="confirm-title" className="text-[14px] font-bold text-bad-ink">
                本当に消しますか？
              </div>
              <p className="mb-1 mt-2 text-[12.5px]">
                {target.letter}:（{formatBytes(target.sizeBytes)}）の中身はすべて消えます。
              </p>
              <label htmlFor="confirm-letter" className="text-[12px] text-mute">
                確認のため「{target.letter}」と入力してください
              </label>
              <input
                id="confirm-letter"
                autoFocus
                value={typed}
                maxLength={2}
                onChange={(e) => setTyped(e.target.value.toUpperCase())}
                className="num selectable mt-2 block h-8 w-[120px] rounded-md border border-line3 bg-deep px-3 text-[13px] text-ink outline-none focus:border-amber"
              />
              <div className="mt-4 flex justify-end gap-3">
                <GhostButton onClick={() => setConfirm(false)} disabled={busy} className="h-[34px] w-24">
                  やめる
                </GhostButton>
                <button
                  type="button"
                  disabled={typed.replace(":", "") !== target.letter || busy}
                  onClick={runFormat}
                  className="h-[34px] w-[110px] rounded-md bg-bad-line text-[12.5px] font-bold text-white disabled:opacity-40"
                >
                  {busy ? "作り直し中…" : "消して作り直す"}
                </button>
              </div>
              <p className="mb-0 mt-2 text-[10.5px] text-dim">実行時だけWindowsの許可（管理者）を求めます</p>
            </div>
          )}
          {result && (
            <div
              className={
                "selectable rounded-lg border px-[18px] py-3 text-[12.5px] " +
                (result.ok ? "border-ok-line bg-ok-bg text-ok-ink" : "border-bad-line bg-bad-bg text-bad-ink")
              }
            >
              {result.ok ? `${result.message}。カードを画面に戻して電源を入れ直してください` : result.message}
            </div>
          )}
        </section>

        {/* 作り直し */}
        <section className="flex min-w-0 flex-col gap-3">
          <div>
            <h2 className="m-0 text-[14px] font-bold">カードを作り直す（FAT32）</h2>
            <p className="m-0 mt-1 text-[11.5px] text-dim">カードを画面から抜いて、PCのカードリーダーに挿してください</p>
          </div>
          <div role="radiogroup" aria-label="作り直すドライブ" className="flex flex-col gap-[6px]">
            {drives.length === 0 && (
              <div className="rounded-lg border border-line bg-[#191e1d] px-4 py-3 text-[12.5px] text-mute">
                カードリーダーやUSBメモリが見つかりません
              </div>
            )}
            {drives.map((d) => {
              const on = d.letter === selected;
              return (
                <button
                  key={d.letter}
                  type="button"
                  role="radio"
                  aria-checked={on}
                  disabled={busy}
                  onClick={() => {
                    setSelected(d.letter);
                    setConfirm(false);
                  }}
                  className={
                    "flex items-center gap-3 rounded-lg border px-4 py-[7px] text-left " +
                    (on ? "border-amber bg-raise" : "border-line bg-[#191e1d]")
                  }
                >
                  <span
                    className={
                      "flex h-3 w-3 items-center justify-center rounded-full border-2 " +
                      (on ? "border-amber" : "border-faint")
                    }
                  >
                    {on && <span className="h-[5px] w-[5px] rounded-full bg-amber" />}
                  </span>
                  <span className="min-w-0">
                    <span className={"block text-[13px] " + (on ? "text-ink" : "text-mute")}>
                      {d.letter}:　{d.label || (d.bus === "SD" ? "SDカードリーダー" : "USBメモリ")}
                    </span>
                    <span className={"block text-[11px] " + (on ? "text-dim" : "text-faint")}>
                      {formatBytes(d.sizeBytes)}・現在 {d.fs || "未フォーマット"}・{d.bus}
                    </span>
                  </span>
                </button>
              );
            })}
          </div>
          <p className="m-0 text-[11px] text-dim">内蔵ディスク（C:など）は一覧に出しません</p>
          <button
            type="button"
            disabled={!target || busy}
            onClick={() => {
              setConfirm(true);
              setTyped("");
              setResult(null);
            }}
            className="h-10 rounded-lg border border-bad-line bg-bad-bg text-[13.5px] font-bold text-bad-ink disabled:opacity-40"
          >
            {target ? `${target.letter}: を消してFAT32で作り直す…` : "ドライブを選んでください"}
          </button>
        </section>
      </div>

      {/* 記録 */}
      <section className="mt-auto">
        <button
          type="button"
          className="text-[11.5px] text-dim hover:text-ink2"
          aria-expanded={log !== null}
          onClick={async () => setLog(log ? null : await api.log())}
        >
          {log ? "▾" : "▸"} 詳しい記録（うまく動かないときに送ってください）
        </button>
        {log && (
          <div className="mt-2">
            <div className="mb-2 flex gap-2">
              <GhostButton className="h-6 text-[11.5px]" onClick={async () => setLog(await api.log())}>
                更新
              </GhostButton>
              <GhostButton
                className="h-6 text-[11.5px]"
                onClick={() => navigator.clipboard?.writeText(log.join("\n")).catch(() => undefined)}
              >
                コピー
              </GhostButton>
            </div>
            <pre className="selectable num m-0 max-h-48 overflow-auto rounded-md border border-line bg-deep p-3 text-[11px] leading-relaxed text-ink2">
              {log.length ? log.join("\n") : "まだ記録はありません"}
            </pre>
          </div>
        )}
      </section>
    </div>
  );
}
