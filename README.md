# LiveKit TUI Client

RustとRatatuiを使って構築された、ターミナル上で動作するSNS型ビデオ・音声通話プラットフォーム。
ログインしてオンラインのユーザー一覧を確認し、相手を選んで通話できる。

## 必要な環境 (Prerequisites)

- **Rust** (最新の安定版)
- **Odin** & **Zig** (`terminal-pixel-animation` のビルドに必要)
- **Webカメラ** / **マイク** が接続されていること
- **LiveKit Cloud** のアカウント (またはセルフホストのLiveKitサーバー)

## アーキテクチャ概要

```
[Client A (TUI)] ───────── Direct LiveKit Cloud ─────────> [Client B (TUI)]
       │                                                       │
       │  Room "lobby": 在籍管理 + シグナリング (Data Channel)    │
       │  Room "call_*": 音声/映像 (WebRTC)                    │
       │                                                       │
       └── JWTトークンはクライアントがAPI Key/Secretから生成 ──┘
```

- シグナリングサーバーは不要。**LiveKit Cloud** に直接接続
- 全クライアントが永続的に **lobby** ルームに参加し、在籍管理とシグナリング（発信・着信通知）を **LiveKit Data Channel** 上で行う
- 通話開始時に **call\_{caller}\_{callee}** ルームが動的生成され、音声・映像が流れる
- JWTトークンはクライアント側で生成（設定した API Key/Secret から生成）

## セットアップ手順

### 1. アプリの起動と設定

通話したい各デバイスで以下を実行します。

```bash
cargo run
# または
livekit-tui-client
```

起動後、ログイン画面にて以下の情報を入力します（`[Tab]` キーで項目を移動します）：
- **Username**: あなたの名前
- **LiveKit URL**: (例: `wss://your-livekit-server.example.com`)
- **API Key**: LiveKit APIキー
- **API Secret**: LiveKit シークレット

> **設定の保存について**
> 入力した情報はログイン成功時に `~/.config/livekit-tui-client/config.toml` へ自動で保存されます。次回起動時に入力を省略できます。
> また、ログイン後は `[s]` キーで **Settings** 画面を開き、APIキーなどを再編集することも可能です。ターミナルのコピー＆ペースト（Ctrl+Vなど）にも対応しています。

---

## 使い方

| 画面 | 操作キー |
|------|---------|
| **ログイン画面** | `[Tab]`で項目移動、名前等を入力して `[Enter]` で接続 |
| **ユーザー一覧** | `[↑↓]` で選択、`[Enter]` で発信、`[s]` で設定変更、`[q]` で終了 |
| **設定画面 (Settings)** | `[Tab]` で項目移動、`[Enter]` で保存して戻る |
| **着信画面** | `[y]` で受話、`[n]` で拒否 |
| **通話中** | `[m]` でマイクOn/Off、`[r]` で描画モード切替(Odin/Zig)、`[q]` で終了 |

> **動画描画モードについて**
> 通話中、`[r]` キーを押すことで動的に描画エンジンを切り替えられます。
> - **Odin (Braille)**: 高解像度なピクセルアニメーション描画
> - **Zig (HalfBlock)**: 従来のモザイク風ブロック描画

---

## Linux向けインストール

様々なLinuxディストリビューション（Arch, Ubuntu, Debian, Fedora, openSUSE, AlmaLinux 等）に対応した導入方法を用意しています。

### 【推奨】1クリック自動インストール (全Distro対応)
依存関係（Rust, Zig, Odin, ALSA等のシステムライブラリ）のインストールからビルドまでを全て自動で行うスクリプトです。ターミナルに以下の1行を貼り付けて実行するだけで構築が完了します。

```bash
curl -sSfL https://raw.githubusercontent.com/TatsuyaM2667/livekit-tui-client/master/install.sh | bash
```
> ※ スクリプト内でパッケージマネージャ(`apt`, `dnf`, `pacman`, `zypper`)を自動検知して処理します。インストール後は `livekit-tui-client` コマンドでどこからでも起動できます。

---

### パッケージマネージャを使う場合 (AUR / RPM)
手動でパッケージとしてシステムにインストールしたい場合は以下の手順をご利用ください。

#### Arch Linux系 (yay, paru)
Arch LinuxやManjaroなどの環境では、AURに登録されている `livekit-tui-client-git` をインストールできます。

```bash
paru -S livekit-tui-client-git
# または
yay -S livekit-tui-client-git
```

### Fedora, openSUSE, RHEL系 (RPM)
RPM系の環境では、同梱的 `livekit-tui-client.spec` を使用してRPMパッケージを作成し、インストールすることができます。

```bash
rpmbuild -ba livekit-tui-client.spec
```

## 別デバイスとの通話方法

1. 各デバイスを起動し、同じ LiveKit Cloud の認証情報を設定する。
2. 別々のユーザー名（Username）でログインする。
3. ユーザー一覧から相手を選択して `[Enter]` を押すと着信通知が飛び、相手が `[y]` を押すと通話が始まる。

## トラブルシューティング

- **LiveKit Cloud に繋がらない場合**: `LIVEKIT_URL` が正しいか (wss://)、`LIVEKIT_API_KEY`/`LIVEKIT_API_SECRET` が LiveKit Cloud のダッシュボードと一致しているか確認してください。
- **映像が映らない場合**: ターミナルが True Color (24-bit) に対応しているか確認してください (Alacritty, WezTerm, iTerm2 など)。
