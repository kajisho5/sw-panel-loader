# SW PANEL LOADER

TURZX（Turing）8.8インチ画面に、PCから画像を入れて「電源を入れたらその画像を表示」までを1画面で設定するWindowsアプリです。

![アイコン](assets/icon.png)

## できること

- 画像を選ぶ（ドラッグ可）→ 画面ぴったり（縦480×横1920）に自動で整えて書き込み
- 回転（90°ずつ）、全体表示／いっぱいに切抜き、余白の色
- 電源オンでこの画像を表示（PCにつながなくても表示）、スリープしない、明るさ
- 起動ロゴ（電源直後に出る絵）を同じ画像・黒に替える
- 前に入れた画像を消す（残っていると順番に切り替わるため）
- TFカードをFAT32で作り直す（PCのカードリーダーに挿したもの）

## 使い方

1. [リリース](https://github.com/kajisho5/sw-panel-loader/releases) から `…_x64-setup.exe` をダウンロードして入れる
2. 公式のTURZXソフトを閉じる（同時には使えません）
3. 画面をUSBでつなぎ、アプリを開く → 右上が「接続中」になればOK
4. 画像を選んで「画面に書き込んで再起動」

## 対応機種

- TURZX / Turing 8.8インチ（USB接続の版。機器番号 1CBE:0088）
- 確認済みのファームウェア：turzx_0001_0015

## 作り

- Tauri 2（Rust）＋ React / TypeScript / Tailwind CSS
- 画面との通信は [turing-smart-screen-python](https://github.com/mathoudebine/turing-smart-screen-python) の解析結果と、公式ソフトの動きを照らし合わせて実装
- ビルドとリリースは GitHub Actions（Windows）で自動。`src-tauri/tauri.conf.json` の版番号を上げて main に入れると新しいリリースが出ます

## ライセンス

GPL-3.0-or-later（参考にした turing-smart-screen-python に合わせています）
