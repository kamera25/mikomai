# mikomai-core

Tauri と OS 固有の UI に依存しない Rust コアです。domain/application/port を提供し、GUI や CLI から共有します。

現在はタスク・操作モデル、アプリケーションサービス、ポートに加えて、ARP/経路/インターフェースの共通スキーマと検証、nwdiag DSL 検証、ツール種別、グラフ識別子を含みます。これらは Tauri や OS 固有機能に依存せず利用できます。

`mikomai-desktop/src-tauri/` には既存の Tauri アプリケーション統合と移行中のサービスが残っています。機能を移す際は OS/UI と独立したロジックをこの crate に置き、Tauri のコマンド層から呼び出してください。
