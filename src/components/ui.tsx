// 小さな共通部品（SVGデザインの見た目どおり）
import type { ReactNode } from "react";

export function StepTitle({ n, children, note }: { n: number; children: ReactNode; note?: ReactNode }) {
  return (
    <div className="flex items-baseline gap-2">
      <span className="num text-[12px] text-amber">{n}</span>
      <h2 className="m-0 text-[14px] font-bold text-ink">{children}</h2>
      {note && <span className="ml-2 text-[11.5px] text-dim">{note}</span>}
    </div>
  );
}

export function Segmented<T extends string | number>({
  options,
  value,
  onChange,
  disabled,
  label,
}: {
  options: { value: T; label: string }[];
  value: T;
  onChange: (v: T) => void;
  disabled?: boolean;
  label: string;
}) {
  return (
    <div
      role="radiogroup"
      aria-label={label}
      className="inline-flex rounded-md border border-line2 bg-raise p-[2px]"
    >
      {options.map((o) => {
        const on = o.value === value;
        return (
          <button
            key={String(o.value)}
            type="button"
            role="radio"
            aria-checked={on}
            disabled={disabled}
            onClick={() => onChange(o.value)}
            className={
              "h-6 min-w-[70px] rounded-[5px] px-3 text-[12.5px] transition-colors " +
              (on ? "bg-amber font-bold text-amber-ink" : "text-ink2 hover:text-ink")
            }
          >
            {o.label}
          </button>
        );
      })}
    </div>
  );
}

export function Toggle({
  checked,
  onChange,
  label,
  disabled,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={
        "relative h-6 w-11 shrink-0 rounded-full transition-colors " + (checked ? "bg-amber" : "bg-line3")
      }
    >
      <span
        className={
          "absolute top-[3px] h-[18px] w-[18px] rounded-full bg-win transition-[left] " +
          (checked ? "left-[23px]" : "left-[3px]")
        }
      />
    </button>
  );
}

export function SettingRow({ title, sub, children }: { title: string; sub?: string; children: ReactNode }) {
  return (
    <div className="flex items-center gap-4 py-[10px]">
      <div className="min-w-0 flex-1">
        <div className="text-[13px] text-ink">{title}</div>
        {sub && <div className="text-[11.5px] text-dim">{sub}</div>}
      </div>
      {children}
    </div>
  );
}

export function GhostButton(props: React.ButtonHTMLAttributes<HTMLButtonElement>) {
  const { className = "", ...rest } = props;
  return (
    <button
      type="button"
      {...rest}
      className={
        "h-7 rounded-md border border-line3 bg-btn px-3 text-[12.5px] text-ink hover:border-dim disabled:opacity-50 " +
        className
      }
    />
  );
}
