# mikomai-core

OS固有のUIに依存しないRustコアです。domain/application/portを提供し、macOSアプリやCLIから共有します。

現在はタスク・操作モデル、アプリケーションサービス、ポートに加えて、ARP/経路/インターフェースの共通スキーマと検証、nwdiag DSL検証、ツール種別、グラフ識別子を含みます。これらはOS固有機能に依存せず利用できます。

macOSアプリは `mikomai-desktop-mac/`、CLIとFFIは `crates/` にあります。OS/UIと独立したロジックはこのcrateに置いてください。
