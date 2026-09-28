# mikomai-desktop

React / TypeScript のデスクトップ UI と Tauri アプリをまとめます。

- `src/`: Web UI と IPC クライアント
- `src-tauri/`: Tauri の起動、IPC、デスクトップ固有の統合

リポジトリ直下の npm workspace から `npm run dev`、`npm run build`、`npm run tauri dev` を実行します。Rust コアは `../mikomai-core/` にあります。
