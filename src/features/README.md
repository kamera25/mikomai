# Features

機能単位のUI（chat、operations、connections、settings）を配置する。既存componentsは移行中も動作させ、各機能から `src/platform` の型付きAPIだけを利用する。

外部からは `src/features/index.ts` を入口にする。
