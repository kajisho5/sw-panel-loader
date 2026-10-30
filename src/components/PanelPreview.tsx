// 左側：実物と同じ縦横比（480×1920）のプレビュー
export function PanelPreview({
  src,
  busy,
  onRotate,
  disabled,
}: {
  src: string | null;
  busy: boolean;
  onRotate: (delta: number) => void;
  disabled: boolean;
}) {
  return (
    <section className="flex w-[250px] shrink-0 flex-col rounded-[10px] border border-line bg-sunk px-5 pb-3 pt-4">
      <div className="flex items-baseline justify-between">
        <span className="text-[12px] text-mute">画面の見た目</span>
        <span className="num text-[11px] text-faint">480×1920</span>
      </div>
      <div className="flex flex-1 items-center justify-center py-3">
        <div className="relative rounded-[7px] border border-line3 bg-[#2a2f2e] p-[7px]">
          <div className="relative aspect-[1/4] h-[min(380px,calc(100vh-290px))] min-h-[260px] overflow-hidden rounded-[2px] bg-black">
            {src ? (
              <img src={src} alt="画面に表示される画像" className="h-full w-full object-fill" draggable={false} />
            ) : (
              <div className="flex h-full flex-col items-center justify-center gap-2 px-2 text-center text-[11px] leading-relaxed text-faint">
                画像を選ぶと
                <br />
                ここに表示
              </div>
            )}
            {busy && <div className="absolute inset-0 animate-pulse bg-white/5" />}
          </div>
        </div>
      </div>
      <div className="flex justify-center gap-3">
        <button
          type="button"
          disabled={disabled}
          onClick={() => onRotate(-90)}
          className="h-[22px] w-[86px] rounded-full border border-line2 bg-[#202726] text-[11.5px] text-ink2 hover:text-ink disabled:opacity-40"
        >
          ↺ 左に回す
        </button>
        <button
          type="button"
          disabled={disabled}
          onClick={() => onRotate(90)}
          className="h-[22px] w-[86px] rounded-full border border-line2 bg-[#202726] text-[11.5px] text-ink2 hover:text-ink disabled:opacity-40"
        >
          右に回す ↻
        </button>
      </div>
    </section>
  );
}
