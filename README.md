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
- JWTトークンはクライアント側で生成（API Key/Secret はクライアントの `.env` に保持）

## セットアップ手順

### 1. `.env` ファイルの設定

プロジェクトルートの `.env` を以下のように設定。

```env
# LiveKit Cloud の URL (wss://プロトコル)
LIVEKIT_URL=wss://your-project.livekit.cloud

# LiveKit Cloud の API Key と Secret
LIVEKIT_API_KEY=your_api_key
LIVEKIT_API_SECRET=your_api_secret
```

### 2. クライアントの起動

起動する各端末で以下を実行。

```bash
cargo run --bin client
```

起動後、ユーザー名を入力してログインするとオンラインユーザー一覧が表示。

シグナリングサーバーは不要。各クライアントが LiveKit Cloud に直接接続

## 使い方

| 画面             | 操作キー                                      |
| ---------------- | --------------------------------------------- |
| **ログイン画面** | 名前を入力して `[Enter]`                      |
| **ユーザー一覧** | `[↑↓]` で選択、`[Enter]` で発信、`[q]` で終了 |
| **着信画面**     | `[y]` で受話、`[n]` で拒否                    |
| **通話中**       | `[m]` でマイクOn/Off、`[q]` で通話終了        |

## 別デバイスとの通話方法

1. 各デバイスの `.env` に同じ LiveKit Cloud の認証情報を設定。
2. 各デバイスで `cargo run --bin client` を実行し、別々のユーザー名でログイン。
3. ユーザー一覧から相手を選択して `[Enter]` を押すと着信通知が飛び、相手が `[y]` を押すと通話が始まる。

## トラブルシューティング

- **LiveKit Cloud に繋がらない場合**: `LIVEKIT_URL` が正しいか (wss://)、`LIVEKIT_API_KEY`/`LIVEKIT_API_SECRET` が LiveKit Cloud のダッシュボードと一致しているか確認してください。
- **映像が映らない場合**: ターミナルが True Color (24-bit) に対応しているか確認してください (Alacritty, WezTerm, iTerm2 など)。
