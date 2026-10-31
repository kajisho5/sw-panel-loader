// 本体（Rust）の関数を呼ぶ窓口
import { invoke } from "@tauri-apps/api/core";

export type Fit = "contain" | "cover";
export type BootLogo = "keep" | "same" | "black";

export interface Job {
  sourcePath: string;
  rotation: number;
  fit: Fit;
  padColor: string;
  margin: [number, number, number, number];
}

export interface Prefs {
  rotation: number;
  fit: Fit;
  padColor: string;
  bootShowImage: boolean;
  noSleep: boolean;
  brightness: number;
  removeOthers: boolean;
  bootLogo: BootLogo;
  lastDir: string | null;
  margin: [number, number, number, number];
  version?: number;
}

export interface Storage {
  totalKb: number;
  usedKb: number;
  freeKb: number;
}

export interface Presence {
  connected: boolean;
  otherModel: string | null;
}

export interface DeviceStatus extends Presence {
  storage: Storage | null;
  error: string | null;
  busy?: boolean;
}

export interface SourceInfo {
  width: number;
  height: number;
  bytes: number;
  fileName: string;
}

export interface WriteOptions {
  bootShowImage: boolean;
  noSleep: boolean;
  brightness: number;
  removeOthers: boolean;
  bootLogo: BootLogo;
  restart: boolean;
}

export interface WriteResult {
  devicePath: string;
  playedNow: boolean;
  removed: string[];
  usedInternal: boolean;
}

export interface Progress {
  stage: "prepare" | "upload" | "save" | "restart" | "done";
  label: string;
  sent: number;
  total: number;
}

export interface CardDrive {
  letter: string;
  label: string;
  fs: string;
  sizeBytes: number;
  bus: string;
  diskNumber: number;
}

export const api = {
  presence: () => invoke<Presence>("device_presence"),
  status: () => invoke<DeviceStatus>("device_status"),
  inspect: (path: string) => invoke<SourceInfo>("inspect_image", { path }),
  preview: (job: Job) => invoke<string>("preview", { job }),
  write: (job: Job, opts: WriteOptions) => invoke<WriteResult>("write_panel", { job, opts }),
  restart: () => invoke<void>("restart_device"),
  reset: (blackLogo: boolean) => invoke<void>("reset_device", { blackLogo }),
  loadPrefs: () => invoke<Prefs>("load_prefs"),
  savePrefs: (prefs: Prefs) => invoke<void>("save_prefs", { prefs }),
  log: () => invoke<string[]>("get_log"),
  listCards: () => invoke<CardDrive[]>("list_cards"),
  formatCard: (letter: string, diskNumber: number) =>
    invoke<{ ok: boolean; message: string }>("format_card", { letter, diskNumber }),
};

export function formatBytes(b: number): string {
  if (b >= 1024 ** 3) return `${(b / 1024 ** 3).toFixed(1)}GB`;
  if (b >= 1024 ** 2) return `${(b / 1024 ** 2).toFixed(1)}MB`;
  if (b >= 1024) return `${Math.round(b / 1024)}KB`;
  return `${b}B`;
}

export const kbToBytes = (kb: number) => kb * 1024;

export function errText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}
