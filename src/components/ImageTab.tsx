// メイン画面「画像を入れる」
import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import { api, errText, formatBytes, type Job, type Prefs, type Progress, type SourceInfo, type WriteResult } from "../lib/api";
import { PanelPreview } from "./PanelPreview";
import { GhostButton, Segmented, SettingRow, StepTitle, Toggle } from "./ui";

const PAD_COLORS = ["#000000", "#ffffff", "#0b1825"];
const IMAGE_EXT = ["png", "jpg", "jpeg", "bmp", "webp", "gif"];

type Phase =
  | { kind: "idle" }
  | { kind: "writing"; progress: Progress | null }
  | { kind: "done"; result: WriteResult }
  | { kind: "error"; message: string };

export function ImageTab({
  prefs,
  setPrefs,
  connected,
  freeText,
}: {
  prefs: Prefs;
  setPrefs: (p: Partial<Prefs>) => void;
  connected: boolean;
  freeText: string;
}) {
  const [path, setPath] = useState<string | null>(null);
  const [info, setInfo] = useState<SourceInfo | null>(null);
  const [previewSrc, setPreviewSrc] = useState<string | null>(null);
  const [previewBusy, setPreviewBusy] = useState(false);
  const [pickError, setPickError] = useState<string | null>(null);
  const [phase, setPhase] = useState<Phase>({ kind: "idle" });
  const [dragOver, setDragOver] = useState(false);
  const [showMore, setShowMore] = useState(false);
  const previewSeq = useRef(0);

  const writing = phase.kind === "writing";

  async function choose(p: string) {
    const ext = p.split(".").pop()?.toLowerCase() ?? "";
    if (!IMAGE_EXT.includes(ext)) {
      setPickError("画像ファイル（PNG・JPEG・BMP・WebP）を選んでください");
      return;
    }
    setPickError(null);
    try {
      const i = await api.inspect(p);
      setPath(p);
      setInfo(i);
      setPhase({ kind: "idle" });
      const dir = p.replace(/[\\/][^\\/]*$/, "");
      setPrefs({ lastDir: dir });
    } catch (e) {
      setPickError(errText(e));
    }
  }

  async function pick() {
    const p = await open({
      multiple: false,
      directory: false,
      defaultPath: prefs.lastDir ?? undefined,
      filters: [{ name: "画像", extensions: IMAGE_EXT }],
    });
    if (typeof p === "string") await choose(p);
  }

  // ドラッグで入れ替え
  useEffect(() => {
    let un: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((e) => {
        if (e.payload.type === "over" || e.payload.type === "enter") setDragOver(true);
        else if (e.payload.type === "leave") setDragOver(false);
        else if (e.payload.type === "drop") {
          setDragOver(false);
          const p = e.payload.paths[0];
          if (p) void choose(p);
        }
      })
      .then((f) => (un = f));
    return () => un?.();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 進み具合
  useEffect(() => {
    const p = listen<Progress>("progress", (e) => {
      setPhase((ph) => (ph.kind === "writing" ? { kind: "writing", progress: e.payload } : ph));
    });
    return () => {
      void p.then((f) => f());
    };
  }, []);

  // プレビュー（向き・合わせ方・余白色が変わるたび）
  const job: Job | null = path
    ? { sourcePath: path, rotation: prefs.rotation, fit: prefs.fit, padColor: prefs.padColor, margin: prefs.margin }
    : null;
  useEffect(() => {
    if (!job) return;
    const seq = ++previewSeq.current;
    setPreviewBusy(true);
    api
      .preview(job)
      .then((src) => {
        if (seq === previewSeq.current) setPreviewSrc(src);
      })
      .catch((e) => setPickError(errText(e)))
      .finally(() => {
        if (seq === previewSeq.current) setPreviewBusy(false);
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [path, prefs.rotation, prefs.fit, prefs.padColor, prefs.margin.join(",")]);

  async function write() {
    if (!job) return;
    setPhase({ kind: "writing", progress: null });
    try {
      const result = await api.write(job, {
        bootShowImage: prefs.bootShowImage,
        noSleep: prefs.noSleep,
        brightness: prefs.brightness,
        removeOthers: prefs.removeOthers,
        bootLogo: prefs.bootLogo,
        restart: false,
      });
      setPhase({ kind: "done", result });
    } catch (e) {
      setPhase({ kind: "error", message: errText(e) });
    }
  }

  const rotate = (d: number) => setPrefs({ rotation: (((prefs.rotation + d) % 360) + 360) % 360 });

  return (
    <div className="flex min-h-0 flex-1 gap-[26px] px-6 pb-6 pt-5">
      <PanelPreview src={path ? previewSrc : null} busy={previewBusy} onRotate={rotate} disabled={!path || writing} />

      <div className="flex min-w-0 flex-1 flex-col gap-[22px] overflow-y-auto pr-1">
        {/* 1 画像 */}
        <section className="flex flex-col gap-[10px]">
          <StepTitle n={1}>画像</StepTitle>
          <div
            className={
              "flex min-h-16 items-center gap-[14px] rounded-lg border border-dashed bg-raise px-4 py-3 " +
              (dragOver ? "border-amber" : "border-line3")
            }
          >
            {path && info ? (
              <>
                <div className="h-10 w-10 shrink-0 overflow-hidden rounded border border-line3 bg-black">
                  {previewSrc && <img src={previewSrc} alt="" className="h-full w-full object-cover" />}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="truncate text-[13px] text-ink" title={path}>
                    {info.fileName}
                  </div>
                  <div className="text-[11.5px] text-dim">
                    <span className="num">
                      {info.width}×{info.height}
                    </span>
                    ・{formatBytes(info.bytes)} ／ ここに画像をドラッグしても入れ替えできます
                  </div>
                </div>
                <GhostButton onClick={pick} disabled={writing} className="w-20">
                  選び直す
                </GhostButton>
              </>
            ) : (
              <>
                <div className="min-w-0 flex-1 text-[12.5px] text-mute">
                  画像をここにドラッグするか、ボタンから選んでください
                  <div className="text-[11.5px] text-dim">PNG・JPEG・BMP・WebP。大きさは自動で480×1920に合わせます</div>
                </div>
                <button
                  type="button"
                  onClick={pick}
                  className="h-8 rounded-md bg-amber px-4 text-[12.5px] font-bold text-amber-ink"
                >
                  画像を選ぶ
                </button>
              </>
            )}
          </div>
          {pickError && <p className="m-0 text-[12px] text-bad-ink">{pickError}</p>}
        </section>

        {/* 2 向き・合わせ方 */}
        <section className="flex flex-col gap-3">
          <StepTitle n={2}>向き・合わせ方</StepTitle>
          <div className="grid grid-cols-[80px_1fr] items-center justify-items-start gap-y-3">
            <span className="text-[12.5px] text-mute">回転</span>
            <Segmented
              label="回転"
              value={prefs.rotation}
              disabled={writing}
              onChange={(v) => setPrefs({ rotation: v })}
              options={[0, 90, 180, 270].map((d) => ({ value: d, label: `${d}°` }))}
            />
            <span className="text-[12.5px] text-mute">合わせ方</span>
            <div className="flex flex-wrap items-center gap-x-6 gap-y-2">
              <Segmented
                label="合わせ方"
                value={prefs.fit}
                disabled={writing}
                onChange={(v) => setPrefs({ fit: v })}
                options={[
                  { value: "contain", label: "全体を表示" },
                  { value: "cover", label: "いっぱいに切抜き" },
                ]}
              />
              <div className="flex items-center gap-2" role="radiogroup" aria-label="余白の色">
                <span className="mr-1 text-[12.5px] text-mute">余白の色</span>
                {PAD_COLORS.map((c) => (
                  <button
                    key={c}
                    type="button"
                    role="radio"
                    aria-checked={prefs.padColor === c}
                    aria-label={c}
                    disabled={writing || prefs.fit === "cover"}
                    onClick={() => setPrefs({ padColor: c })}
                    style={{ background: c }}
                    className={
                      "h-6 w-6 rounded-[5px] border disabled:opacity-40 " +
                      (prefs.padColor === c ? "border-2 border-amber" : "border-line3")
                    }
                  />
                ))}
              </div>
            </div>
            <span className="text-[12.5px] text-mute">余白</span>
            <MarginEditor
              value={prefs.margin}
              disabled={writing}
              onChange={(m) => setPrefs({ margin: m })}
            />
          </div>
        </section>

        {/* 3 表示の設定 */}
        <section className="flex flex-col gap-[10px]">
          <StepTitle n={3}>表示の設定</StepTitle>
          <div className="rounded-lg border border-line bg-raise px-[18px] py-1">
            <SettingRow title="電源を入れたらこの画像を表示" sub="PCにつながなくても表示されます">
              <Toggle
                label="電源を入れたらこの画像を表示"
                checked={prefs.bootShowImage}
                disabled={writing}
                onChange={(v) => setPrefs({ bootShowImage: v })}
              />
            </SettingRow>
            <div className="border-t border-line" />
            <SettingRow title="スリープしない（ずっと表示）">
              <Toggle
                label="スリープしない"
                checked={prefs.noSleep}
                disabled={writing}
                onChange={(v) => setPrefs({ noSleep: v })}
              />
            </SettingRow>
            <div className="flex items-center gap-4 pb-3 pt-1">
              <label htmlFor="brightness" className="w-16 text-[13px] text-ink">
                明るさ
              </label>
              <input
                id="brightness"
                type="range"
                min={10}
                max={100}
                step={5}
                value={prefs.brightness}
                disabled={writing}
                onChange={(e) => setPrefs({ brightness: Number(e.target.value) })}
                className="flex-1"
              />
              <span className="num w-10 text-right text-[12.5px] text-ink2">{prefs.brightness}%</span>
            </div>
            <button
              type="button"
              onClick={() => setShowMore((s) => !s)}
              aria-expanded={showMore}
              className="mb-2 text-[11.5px] text-dim hover:text-ink2"
            >
              {showMore ? "▾" : "▸"} くわしい設定
            </button>
            {showMore && (
              <div className="border-t border-line">
                <SettingRow title="起動ロゴ" sub="電源を入れた直後に出る絵。上に帯が残る場合はここで替えます">
                  <Segmented
                    label="起動ロゴ"
                    value={prefs.bootLogo}
                    disabled={writing}
                    onChange={(v) => setPrefs({ bootLogo: v })}
                    options={[
                      { value: "same", label: "同じ画像" },
                      { value: "black", label: "黒" },
                      { value: "keep", label: "そのまま" },
                    ]}
                  />
                </SettingRow>
                <div className="border-t border-line" />
                <SettingRow title="前に入れた画像を消す" sub="残っていると順番に切り替わって表示されます">
                  <Toggle
                    label="前に入れた画像を消す"
                    checked={prefs.removeOthers}
                    disabled={writing}
                    onChange={(v) => setPrefs({ removeOthers: v })}
                  />
                </SettingRow>
              </div>
            )}
          </div>
        </section>

        {/* 4 書き込み */}
        <section className="flex flex-col gap-[10px]">
          <StepTitle n={4} note={`保存先：${freeText}`}>
            書き込み
          </StepTitle>
          {phase.kind === "writing" ? (
            <WritingBox progress={phase.progress} />
          ) : (
            <>
              {phase.kind === "done" && (
                <div className="rounded-lg border border-ok-line bg-ok-bg px-[18px] py-3">
                  <div className="text-[13px] font-bold text-ok-ink">
                    {phase.result.playedNow ? "完了しました" : "書き込みました（すぐには表示されませんでした）"}
                  </div>
                  <div className="text-[11.5px] text-ok-sub">
                    画面に表示されました。USBを抜いて、電源だけでも表示されるか確認してください
                    {phase.result.removed.length > 0 && `（前の画像 ${phase.result.removed.length} 枚を消しました）`}
                    {phase.result.usedInternal && "。カードが見つからなかったので画面の内蔵メモリに入れました"}
                  </div>
                </div>
              )}
              {phase.kind === "error" && (
                <div className="selectable rounded-lg border border-bad-line bg-bad-bg px-[18px] py-3 text-[12.5px] text-bad-ink">
                  {phase.message}
                </div>
              )}
              <button
                type="button"
                onClick={write}
                disabled={!path || !connected}
                className="h-11 rounded-lg bg-amber text-[15px] font-bold text-amber-ink disabled:bg-line3 disabled:text-dim"
              >
                画面に書き込む
              </button>
              {!connected && (
                <p className="m-0 text-[11.5px] text-dim">画面をUSBでつなぐと押せるようになります</p>
              )}
            </>
          )}
        </section>
      </div>
    </div>
  );
}

const STAGES: { key: Progress["stage"]; label: string }[] = [
  { key: "upload", label: "送信" },
  { key: "save", label: "設定を保存" },
];

function WritingBox({ progress }: { progress: Progress | null }) {
  const pct = progress && progress.total > 0 ? Math.round((progress.sent / progress.total) * 100) : 0;
  const order = ["prepare", "upload", "save", "restart", "done"];
  const cur = order.indexOf(progress?.stage ?? "prepare");
  return (
    <div className="rounded-lg border border-line3 bg-raise px-[18px] py-3" aria-live="polite">
      <div className="text-[13px] text-ink">
        {progress?.label ?? "準備しています"}…
        {progress?.stage === "upload" && progress.total > 1 && (
          <span className="num ml-2 text-ink2">
            {formatBytes(progress.total)}中 {formatBytes(progress.sent)}
          </span>
        )}
      </div>
      <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-line2">
        <div
          className="h-full rounded-full bg-amber transition-[width]"
          style={{ width: `${progress?.stage === "upload" ? pct : cur >= 2 ? 100 : 6}%` }}
        />
      </div>
      <div className="mt-2 flex gap-3 text-[11px] text-dim">
        {STAGES.map((s) => (
          <span key={s.key} className={order.indexOf(s.key) <= cur ? "text-ink2" : ""}>
            {order.indexOf(s.key) <= cur ? "●" : "○"} {s.label}
          </span>
        ))}
      </div>
    </div>
  );
}

const SIDES = ["上", "右", "下", "左"] as const;

// 端が枠に隠れるときに、画像を内側へ寄せる（画面のピクセル単位）
function MarginEditor({
  value,
  onChange,
  disabled,
}: {
  value: [number, number, number, number];
  onChange: (v: [number, number, number, number]) => void;
  disabled: boolean;
}) {
  const set = (i: number, n: number) => {
    const v = [...value] as [number, number, number, number];
    v[i] = Math.max(0, Math.min(60, Math.round(n || 0)));
    onChange(v);
  };
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
      {SIDES.map((side, i) => (
        <div key={side} className="flex items-center gap-1">
          <label htmlFor={`margin-${i}`} className="text-[12px] text-dim">
            {side}
          </label>
          <button
            type="button"
            aria-label={`${side}の余白を減らす`}
            disabled={disabled || value[i] <= 0}
            onClick={() => set(i, value[i] - 1)}
            className="h-6 w-6 rounded border border-line3 bg-btn text-ink2 disabled:opacity-40"
          >
            −
          </button>
          <input
            id={`margin-${i}`}
            type="number"
            min={0}
            max={60}
            value={value[i]}
            disabled={disabled}
            onChange={(e) => set(i, Number(e.target.value))}
            className="num selectable h-6 w-10 rounded border border-line3 bg-deep text-center text-[12.5px] text-ink outline-none focus:border-amber [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none"
          />
          <button
            type="button"
            aria-label={`${side}の余白を増やす`}
            disabled={disabled || value[i] >= 60}
            onClick={() => set(i, value[i] + 1)}
            className="h-6 w-6 rounded border border-line3 bg-btn text-ink2 disabled:opacity-40"
          >
            ＋
          </button>
        </div>
      ))}
      <span className="text-[11px] text-dim">画面のピクセル。端が隠れる側を増やします</span>
    </div>
  );
}
