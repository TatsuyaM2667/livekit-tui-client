# LiveKit TUI Client

RustとRatatuiを使って構築された、ターミナル上で動作するSNS型ビデオ・音声通話クライアントです。
ログインしてオンラインのユーザー一覧を確認し、好きな相手を選んで通話できます。

## 必要な環境 (Prerequisites)

- **Rust** (最新の安定版)
- **Odin** & **Zig** (`terminal-pixel-animation` のビルドに必要)
- **Webカメラ** / **マイク** が接続されていること
- **LiveKit Server** (セルフホストのLiveKitサーバー)

## アーキテクチャ概要

```
[Client A (TUI)] <--WebSocket--> [Signaling Server] <--WebSocket--> [Client B (TUI)]
                                        |
                          (JWTトークンを生成してクライアントに配布)
                                        |
[Client A] <----WebRTC (via LiveKit)----> [Client B]
```

- **シグナリングサーバー** (`src/bin/server.rs`): オンラインユーザーの管理と着信通知を担当
- **クライアント** (`src/bin/client.rs`): TUI上でログイン・ユーザー選択・通話を行う

---

## セットアップ手順

### 1. `.env` ファイルの設定

プロジェクトルートの `.env` を以下のように設定します。

```env
# LiveKit ServerのURL (wss://プロトコル)
LIVEKIT_URL=wss://your-livekit-server.example.com

# LiveKit の API Key と Secret
# (LiveKit サーバーの設定に合わせてください)
LIVEKIT_API_KEY=devkey
LIVEKIT_API_SECRET=your_secret_here

# シグナリングサーバーのURL (クライアントが接続するアドレス)
# ローカルテスト: ws://127.0.0.1:3000/ws
# 本番環境:       ws://your-server-ip:3000/ws
SIGNALING_URL=ws://127.0.0.1:3000/ws
```

### 2. シグナリングサーバーの起動

シグナリングサーバーは、誰がオンラインかを管理し、通話の開始（発信・着信通知）を仲介します。
サーバーが動いているPCかVPS上で以下を実行します。

```bash
cargo run --bin server
```

> サーバーはポート `3000` でリッスンします。外部からアクセスさせる場合は、ファイアウォールでポート `3000` (TCP) を開放してください。

### 3. クライアントの起動

通話したい各デバイスで以下を実行します。

```bash
cargo run --bin client
```

起動後、ユーザー名を入力してログインするとオンラインユーザー一覧が表示されます。

---

## 使い方

| 画面 | 操作キー |
|------|---------|
| **ログイン画面** | 名前を入力して `[Enter]` |
| **ユーザー一覧** | `[↑↓]` で選択、`[Enter]` で発信、`[q]` で終了 |
| **着信画面** | `[y]` で受話、`[n]` で拒否 |
| **通話中** | `[m]` でマイクOn/Off、`[q]` で通話終了 |

---

## 別デバイスとの通話方法

1. シグナリングサーバーをインターネットに公開されているVPS（またはポート転送したPC）で起動します。
2. 各デバイスの `.env` の `SIGNALING_URL` をそのサーバーのアドレスに設定します。
3. 各デバイスで `cargo run --bin client` を実行し、お互いに別々のユーザー名でログインします。
4. ユーザー一覧から相手を選択して `[Enter]` を押すと着信通知が飛び、相手が `[y]` を押すと通話が始まります。

---

## トラブルシューティング

- **シグナリングサーバーに繋がらない場合**: `SIGNALING_URL` のIPアドレスやポートが正しいか確認してください。
- **LiveKitに繋がらない場合**: `LIVEKIT_URL` が正しいか (wss://) 、`LIVEKIT_API_KEY`/`LIVEKIT_API_SECRET` がサーバーの設定と一致しているか確認してください。
- **映像が映らない場合**: ターミナルが True Color (24-bit) に対応しているか確認してください (Alacritty, WezTerm, iTerm2 など)。
