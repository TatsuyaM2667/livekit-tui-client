# LiveKit TUI Client

RustとRatatuiを使って構築された、ターミナル上で動作するビデオ・音声通話クライアントです。
内部で `terminal-pixel-animation` を用いて、リモートからの映像をターミナル上に直接半角ブロック（Half-block）で描画します。

## 必要な環境 (Prerequisites)

- **Rust** (最新の安定版)
- **Odin** & **Zig** (`terminal-pixel-animation` のビルドに必要)
- **Webカメラ** / **マイク** が接続されていること
- **LiveKit Server** (LiveKit Cloud、またはセルフホストの AlmaServer / LiveKit サーバー)

## セットアップ手順 (Local Setup)

1. リポジトリをクローンまたはダウンロードします。
2. プロジェクトのルートディレクトリ（この `README.md` がある場所）に `.env` ファイルを作成し、以下の内容を記述します。

```env
LIVEKIT_URL=wss://<YOUR-LIVEKIT-SERVER-URL>
LIVEKIT_TOKEN=<YOUR-ACCESS-TOKEN>
```

3. ビルド＆実行します。
```bash
cargo run --release
```

※ 映像を描画するため、ターミナルは **True Color (24-bitカラー)** に対応している必要があります (Alacritty, WezTerm, iTerm2 など推奨)。

---

## 別デバイス（他のPCなど）でのセットアップ方法

このTUIアプリを使って2台以上のデバイスで通話するには、**共通のLiveKitサーバー（Room）**に接続する必要があります。

### 1. LiveKit サーバーの準備
一番簡単な方法は [LiveKit Cloud (無料枠あり)](https://cloud.livekit.io/) を使う方法です。
もしくは、AlmaServerやVPS上でTailscaleを使ってセルフホストしたLiveKitサーバーを用意します。

### 2. アクセストークン (Token) の発行
LiveKitに接続するためには、デバイスごとに**異なる参加者名 (Participant Identity)** を持ったトークンを発行する必要があります。
（同じIdentityのトークンで複数デバイスから接続すると、後から接続したデバイスにキックされてしまいます）

LiveKit Cloudのコンソール、または LiveKit CLI (`lk` コマンド) を使ってトークンを2つ発行します。

**デバイスA用（例：PC1）:**
- Room: `test-room`
- Identity: `user-1`
- 権限: `canPublish`, `canSubscribe` を付与

**デバイスB用（例：PC2）:**
- Room: `test-room`
- Identity: `user-2`
- 権限: `canPublish`, `canSubscribe` を付与

### 3. デバイスごとの `.env` 設定

**デバイスA (PC1) の `.env`**
```env
LIVEKIT_URL=wss://your-project.livekit.cloud
LIVEKIT_TOKEN=ey... (user-1用に発行したトークン)
```

**デバイスB (PC2) の `.env`**
```env
LIVEKIT_URL=wss://your-project.livekit.cloud
LIVEKIT_TOKEN=ey... (user-2用に発行したトークン)
```

### 4. アプリの起動
両方のデバイスでソースコードを取得し、それぞれ `cargo run --release` を実行します。
お互いの `.env` が同じ `LIVEKIT_URL` を指しており、かつトークンの `Room` 名が一致していれば、自動的にお互いの映像と音声がターミナル上に流れてきます。

---

## トラブルシューティング

- **`validate request timed out` エラーが出る場合**
  `.env` に記載された `LIVEKIT_URL` にネットワーク的に到達できていないか、URLが間違っている（`https://` ではなく `wss://` を指定しているか等）可能性があります。
- **Odin/Zigのコンパイルエラー**
  `odin` コマンドと `zig` コマンドにパスが通っていることを確認してください。
