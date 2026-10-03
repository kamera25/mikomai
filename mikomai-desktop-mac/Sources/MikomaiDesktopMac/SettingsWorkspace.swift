import SwiftUI
import AppKit
import Foundation
import UniformTypeIdentifiers
import MikomaiDesktopCore

private struct RightAlignedSwitchStyle: ToggleStyle {
    func makeBody(configuration: Configuration) -> some View {
        HStack(spacing: 16) {
            configuration.label
                .frame(maxWidth: .infinity, alignment: .leading)
            Toggle(isOn: configuration.$isOn) {
                configuration.label
            }
            .labelsHidden()
            .toggleStyle(.switch)
            .fixedSize()
        }
        .frame(maxWidth: .infinity)
    }
}

struct SettingsWorkspace: View {
    @ObservedObject var model: DesktopModel
    @State private var selectedCategory = 0
    @State private var availablePorts: [String] = []

    private let categories: [(title: String, icon: String, color: Color)] = [
        ("チャット・通信", "bubble.left.and.bubble.right.fill", .blue),
        ("LLM モデル", "cpu", .purple),
        ("Vision (画像)", "photo.fill", .pink),
        ("ナレッジ RAG", "books.vertical.fill", .orange),
        ("設定", "arrow.triangle.2.circlepath", .green)
    ]

    var body: some View {
        HStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 6) {
                Text("設定")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 12).padding(.bottom, 8)
                ForEach(categories.indices, id: \.self) { index in
                    Button { selectedCategory = index } label: {
                        HStack(spacing: 10) {
                            Image(systemName: categories[index].icon)
                                .font(.system(size: 15, weight: .medium))
                                .foregroundStyle(.white)
                                .frame(width: 26, height: 26)
                                .background(categories[index].color.gradient, in: RoundedRectangle(cornerRadius: 6))
                            Text(categories[index].title)
                                .font(.system(size: 15, weight: selectedCategory == index ? .semibold : .regular))
                            Spacer(minLength: 0)
                        }
                        .foregroundStyle(selectedCategory == index ? Color.white : Color.primary)
                        .padding(.horizontal, 10).padding(.vertical, 8)
                        .background(selectedCategory == index ? Color.accentColor : .clear, in: RoundedRectangle(cornerRadius: 8))
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
                Spacer()
            }
            .padding(12)
            .frame(width: 210)
            .frame(maxHeight: .infinity)
            .background(.regularMaterial)

            Divider()

            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    Text(categories[selectedCategory].title)
                        .font(.system(size: 24, weight: .bold))
                        .padding(.bottom, 4)
                    VStack(alignment: .leading, spacing: 20) {
                        switch selectedCategory {
                        case 0: chatAndNetworkSection
                        case 1: llmModelSection
                        case 2: visionSection
                        case 3: knowledgeSection
                        case 4: nativeSettingsSection
                        default: EmptyView()
                        }
                    }
                    .padding(20)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 12))
                    .overlay {
                        RoundedRectangle(cornerRadius: 12)
                            .strokeBorder(Color(nsColor: .separatorColor).opacity(0.5), lineWidth: 1)
                    }
                }
                .padding(28)
                .frame(maxWidth: 880, alignment: .leading)
                .frame(maxWidth: .infinity, alignment: .topLeading)
            }
            .background(Color(nsColor: .underPageBackgroundColor))
        }
        .toggleStyle(RightAlignedSwitchStyle())
        .onAppear {
            availablePorts = SerialPortDetector.listPorts()
        }
    }

    // MARK: Category 0: Chat & Network Settings

    private var chatAndNetworkSection: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("チャット・通信設定").font(.system(size: 17, weight: .semibold))

            // History Limit
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Text("会話履歴の上限 (ターン数)")
                    Spacer()
                    Text("\(model.settings.historyLimit)").font(.system(size: 14, design: .monospaced)).bold()
                }
                Slider(value: Binding(
                    get: { Double(model.settings.historyLimit) },
                    set: { model.settings.historyLimit = Int($0); model.saveSettings() }
                ), in: 0...20, step: 1)
                Text("モデルに送信する直近の会話履歴の最大往復数です (0〜20)。").font(.system(size: 13)).foregroundStyle(.secondary)
            }

            Divider()

            // Temperature
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Text("サンプリング温度 (Temperature)")
                    Spacer()
                    Text(String(format: "%.1f", model.settings.temperature)).font(.system(size: 14, design: .monospaced)).bold()
                }
                Slider(value: Binding(
                    get: { model.settings.temperature },
                    set: { model.settings.temperature = $0; model.saveSettings() }
                ), in: 0.0...2.0, step: 0.1)
                Text("生成される回答のランダム性を調整します。ネットワーク設定には 0.0〜0.2 の決定的な値が推奨されます。").font(.system(size: 13)).foregroundStyle(.secondary)
            }

            Divider()

            // Repetition Penalty
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Text("繰り返しペナルティ (Repetition Penalty)")
                    Spacer()
                    Text(String(format: "%.2f", model.settings.repetitionPenalty)).font(.system(size: 14, design: .monospaced)).bold()
                }
                Slider(value: Binding(
                    get: { model.settings.repetitionPenalty },
                    set: { model.settings.repetitionPenalty = $0; model.saveSettings() }
                ), in: 1.0...2.0, step: 0.05)
                Text("同じ単語や句の重複を抑えるペナルティ係数です (1.0〜2.0、デフォルト: 1.10)。").font(.system(size: 13)).foregroundStyle(.secondary)
            }

            Divider()

            // MCP Timeout
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Text("MCP / ツール実行タイムアウト (秒)")
                    Spacer()
                    Text("\(model.settings.mcpTimeout ?? 30) 秒").font(.system(size: 14, design: .monospaced)).bold()
                }
                Slider(value: Binding(
                    get: { Double(model.settings.mcpTimeout ?? 30) },
                    set: { model.settings.mcpTimeout = Int($0); model.saveSettings() }
                ), in: 5...120, step: 5)
                Text("ネットワークツールやコマンド実行の待機タイムアウト時間です。").font(.system(size: 13)).foregroundStyle(.secondary)
            }

            Divider()

            // Cache Expiry
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Text("事実グラフ・キャッシュ有効期限 (分)")
                    Spacer()
                    Text("\(model.settings.cacheExpiryMinutes ?? 10) 分").font(.system(size: 14, design: .monospaced)).bold()
                }
                Slider(value: Binding(
                    get: { Double(model.settings.cacheExpiryMinutes ?? 10) },
                    set: { model.settings.cacheExpiryMinutes = Int($0); model.saveSettings() }
                ), in: 0...60, step: 1)
                Text("ネットワークトポロジ事実キャッシュの保持時間です (0 = キャッシュ無効)。").font(.system(size: 13)).foregroundStyle(.secondary)
            }

            Divider()

            // IP Version
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text("優先 IP バージョン").font(.system(size: 15, weight: .medium))
                    Text("Ping や接続テスト時に優先する IP プロトコルを指定します。").font(.system(size: 13)).foregroundStyle(.secondary)
                }
                Spacer()
                Picker("", selection: Binding(
                    get: { model.settings.ipVersion ?? "auto" },
                    set: { model.settings.ipVersion = $0; model.saveSettings() }
                )) {
                    Text("自動判定 (Auto)").tag("auto")
                    Text("IPv4").tag("ipv4")
                    Text("IPv6").tag("ipv6")
                }
                .frame(width: 140)
            }

            Divider()

            // Auto Dry-Run
            Toggle(isOn: Binding(
                get: { model.settings.autoDryRun },
                set: { model.settings.autoDryRun = $0; model.saveSettings() }
            )) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("自動 Dry-Run 検証").font(.system(size: 15, weight: .medium))
                    Text("設定投入前に自動的にドライラン構文チェックを行います。").font(.system(size: 13)).foregroundStyle(.secondary)
                }
            }

            Divider()

            // Console Port & Baud Rate
            VStack(alignment: .leading, spacing: 10) {
                Text("シリアルコンソール設定").font(.system(size: 15, weight: .medium))
                HStack(spacing: 12) {
                    Picker("ポート", selection: Binding(
                        get: { model.settings.consolePort ?? "" },
                        set: { model.settings.consolePort = $0.isEmpty ? nil : $0; model.saveSettings() }
                    )) {
                        Text("未設定 (None)").tag("")
                        ForEach(availablePorts, id: \.self) { port in
                            Text(port).tag(port)
                        }
                    }
                    .frame(maxWidth: .infinity)

                    Picker("ボーレート", selection: Binding(
                        get: { model.settings.consoleBaudRate ?? 9600 },
                        set: { model.settings.consoleBaudRate = $0; model.saveSettings() }
                    )) {
                        Text("9600 bps").tag(9600)
                        Text("19200 bps").tag(19200)
                        Text("38400 bps").tag(38400)
                        Text("57600 bps").tag(57600)
                        Text("115200 bps").tag(115200)
                    }
                    .frame(width: 140)
                }
            }
        }
    }

    // MARK: Category 1: LLM Model Settings

    private var llmModelSection: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("ローカル LLM モデル設定").font(.system(size: 17, weight: .semibold))

            // Presets
            VStack(alignment: .leading, spacing: 6) {
                Text("モデルプリセット").font(.system(size: 14, weight: .medium))
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(PRESET_MODELS) { preset in
                        let exists = HuggingFaceHub.modelExists(repo: preset.repo, filename: preset.filename)
                        modelSelectionRow(preset.id, title: "\(preset.name) \(exists ? "(✓ DL済)" : "(未DL)")")
                    }
                    if model.supportsAppleModelOS {
                        modelSelectionRow(AppleModelPolicy.presetID, title: "AFM 3 Core (macOS 標準)")
                    }
                    modelSelectionRow("custom", title: "カスタムモデル (任意の GGUF)")
                }
                .disabled(model.isWorking || model.isLoadingModel)
                if !model.supportsAppleModelOS {
                    Text(AppleModelPolicy.unsupportedOSMessage)
                        .font(.system(size: 11))
                        .foregroundStyle(.secondary)
                }
            }

            if model.isAppleModelSelected {
                VStack(alignment: .leading, spacing: 10) {
                    Text(model.modelStatus)
                        .font(.system(size: 13))
                        .foregroundStyle(model.modelStatus.hasPrefix("エラー") ? .red : .secondary)
                    Text("Apple Intelligence のオンデバイスモデルを使用します。GGUF のダウンロードは不要です。")
                        .font(.system(size: 13)).foregroundStyle(.secondary)
                    Text("長い会話履歴や参考資料は入力上限に合わせて短縮します。大きな添付資料や画像入力には、Gemma のモデルを選択してください。")
                        .font(.system(size: 12)).foregroundStyle(.secondary)
                    Button(model.isLoadingModel ? "確認中…" : "利用可否を再確認") { model.loadModel() }
                        .disabled(model.isWorking || model.isLoadingModel)
                }
            } else {
            // Hugging Face Repo & Filename
            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 8) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Hugging Face リポジトリ").font(.system(size: 13, weight: .medium))
                        TextField("unsloth/gemma-4-E4B-it-GGUF", text: $model.repoPath)
                            .textFieldStyle(.roundedBorder)
                            .disabled(model.selectedPresetId != "custom")
                    }

                    VStack(alignment: .leading, spacing: 2) {
                        Text("GGUF ファイル名").font(.system(size: 13, weight: .medium))
                        TextField("gemma-4-E4B-it-UD-Q4_K_XL.gguf", text: $model.modelFilename)
                            .textFieldStyle(.roundedBorder)
                            .disabled(model.selectedPresetId != "custom")
                    }
                }

                // Presence check
                let exists = HuggingFaceHub.modelExists(repo: model.repoPath, filename: model.modelFilename)
                HStack(spacing: 6) {
                    Circle().fill(exists ? Color.green : Color.secondary).frame(width: 8, height: 8)
                    Text(exists ? "HuggingFace キャッシュに配置済みです" : "HuggingFace キャッシュに未ダウンロードです")
                        .font(.system(size: 13))
                        .foregroundStyle(exists ? Color.green : Color.secondary)
                    Spacer()
                    if exists {
                        Button("このモデルを適用") {
                            let url = HuggingFaceHub.modelURL(repo: model.repoPath, filename: model.modelFilename)
                            model.modelPath = url.path
                            model.loadModel()
                            model.saveSettings()
                        }
                        .buttonStyle(.borderedProminent)
                        .controlSize(.small)
                    }
                    Button("キャッシュフォルダを開く") {
                        model.openModelDirectory()
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                }
            }

            Divider()

            // Direct local GGUF file path
            VStack(alignment: .leading, spacing: 6) {
                Text("現在ロード対象の GGUF ファイルパス").font(.system(size: 14, weight: .medium))
                HStack(spacing: 8) {
                    TextField("ローカル GGUF パス", text: $model.modelPath)
                        .textFieldStyle(.roundedBorder)
                    Button("選択…") { model.selectModel() }
                    Button(model.isLoadingModel ? "読み込み中…" : "読み込む") { model.loadModel() }
                        .disabled(model.modelPath.isEmpty || model.isLoadingModel)
                }
                Text(model.modelStatus)
                    .font(.system(size: 13))
                    .foregroundStyle(model.modelStatus.hasPrefix("エラー") ? .red : .secondary)
            }

            Divider()

            // Advanced context & generation parameters
            VStack(alignment: .leading, spacing: 12) {
                Text("詳細コンテキスト & 生成パラメータ").font(.system(size: 15, weight: .medium))

                HStack(spacing: 14) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("コンテキスト長 (n_ctx)").font(.system(size: 13))
                        TextField("8192", value: Binding(
                            get: { model.settings.nCtx },
                            set: { model.settings.nCtx = $0; model.saveSettings() }
                        ), formatter: NumberFormatter())
                        .textFieldStyle(.roundedBorder)
                        .frame(width: 100)
                    }

                    VStack(alignment: .leading, spacing: 4) {
                        Text("最大生成トークン (max_gen)").font(.system(size: 13))
                        TextField("2048", value: Binding(
                            get: { model.settings.maxGen },
                            set: { model.settings.maxGen = $0; model.saveSettings() }
                        ), formatter: NumberFormatter())
                        .textFieldStyle(.roundedBorder)
                        .frame(width: 100)
                    }

                    VStack(alignment: .leading, spacing: 4) {
                        Text("保持トークン数 (prompt_keep)").font(.system(size: 13))
                        TextField("500", value: Binding(
                            get: { model.settings.promptKeepTokens },
                            set: { model.settings.promptKeepTokens = $0; model.saveSettings() }
                        ), formatter: NumberFormatter())
                        .textFieldStyle(.roundedBorder)
                        .frame(width: 100)
                    }
                }
            }

            Divider()

            // KV Cache Preload options
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 8) {
                    Text("ワーカー別 KV キャッシュ・プリロード").font(.system(size: 15, weight: .medium))
                    Text("準備中")
                        .font(.system(size: 11, weight: .medium))
                        .padding(.horizontal, 6).padding(.vertical, 2)
                        .background(Color.secondary.opacity(0.2), in: Capsule())
                        .foregroundStyle(.secondary)
                }
                Text("モデルロード時に各専門ワーカーのシステムプロンプトを KV キャッシュに事前展開する機能です（今後の推論エンジン更新で有効化予定）。").font(.system(size: 13)).foregroundStyle(.secondary)

                VStack(alignment: .leading, spacing: 12) {
                    Toggle("ナレッジワーカー", isOn: Binding(
                        get: { model.settings.preloadKnowledge },
                        set: { model.settings.preloadKnowledge = $0; model.saveSettings() }
                    ))
                    Divider()
                    Toggle("アナリストワーカー", isOn: Binding(
                        get: { model.settings.preloadAnalysis },
                        set: { model.settings.preloadAnalysis = $0; model.saveSettings() }
                    ))
                    Divider()
                    Toggle("RAG ワーカー", isOn: Binding(
                        get: { model.settings.preloadRag },
                        set: { model.settings.preloadRag = $0; model.saveSettings() }
                    ))
                    Divider()
                    Toggle("ビルダーワーカー", isOn: Binding(
                        get: { model.settings.preloadBuilder },
                        set: { model.settings.preloadBuilder = $0; model.saveSettings() }
                    ))
                    Divider()
                    Toggle("プロッターワーカー", isOn: Binding(
                        get: { model.settings.preloadPlotter },
                        set: { model.settings.preloadPlotter = $0; model.saveSettings() }
                    ))
                    Divider()
                    Toggle("要約ワーカー", isOn: Binding(
                        get: { model.settings.preloadSummarization },
                        set: { model.settings.preloadSummarization = $0; model.saveSettings() }
                    ))
                }
                .toggleStyle(RightAlignedSwitchStyle())
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(16)
                .background(Color(nsColor: .windowBackgroundColor).opacity(0.55), in: RoundedRectangle(cornerRadius: 10))
                .overlay {
                    RoundedRectangle(cornerRadius: 10)
                        .strokeBorder(Color(nsColor: .separatorColor).opacity(0.45), lineWidth: 1)
                }

            }
            }
        }
    }

    private func modelSelectionRow(_ id: String, title: String) -> some View {
        HistorySelectionRow(isSelected: model.selectedPresetId == id, action: { model.selectPreset(id) }) {
            HStack(spacing: 8) {
                Image(systemName: model.selectedPresetId == id ? "largecircle.fill.circle" : "circle")
                    .foregroundStyle(model.selectedPresetId == id ? Color.accentColor : Color.secondary)
                Text(title).font(.system(size: 13))
            }
        }
        .accessibilityLabel(title)
        .accessibilityValue(model.selectedPresetId == id ? "選択中" : "未選択")
    }

    // MARK: Category 2: Vision Settings

    private var visionSection: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack(spacing: 8) {
                Text("Vision (画像・マルチモーダル) 設定").font(.system(size: 17, weight: .semibold))

            }
            Text("画像入力に対応したGemma 4 GGUFモデルと、そのモデルに対応するmmprojを設定してください。PNG/JPEG画像をチャットに添付して解析できます。").font(.system(size: 13)).foregroundStyle(.secondary)
            if model.isAppleModelSelected {
                Text("AFM 3 Core は現在、画像入力に対応していません。Vision を使う場合は Gemma を選択してください。")
                    .font(.system(size: 12)).foregroundStyle(.secondary)
            }

            Toggle(isOn: Binding(
                get: { model.settings.visionEnabled },
                set: { model.settings.visionEnabled = $0; model.saveSettings() }
            )) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Vision 機能を有効化").font(.system(size: 15, weight: .medium))
                    Text("トポロジ図や機器外観の画像を読み取って分析するマルチモーダル機能を有効化します。").font(.system(size: 13)).foregroundStyle(.secondary)
                }
            }

            Divider()

            VStack(alignment: .leading, spacing: 6) {
                Text("マルチモーダルプロジェクター (mmproj)").font(.system(size: 14, weight: .medium))
                Text("GGUF マルチモーダルプロジェクターファイル (例: mmproj-F16.gguf) のパスを指定します。").font(.system(size: 13)).foregroundStyle(.secondary)
                HStack(spacing: 8) {
                    TextField("mmproj ファイルパス", text: Binding(
                        get: { model.settings.mmprojPath ?? "" },
                        set: { model.settings.mmprojPath = $0.isEmpty ? nil : $0; model.saveSettings() }
                    ))
                    .textFieldStyle(.roundedBorder)

                    Button("選択…") {
                        let panel = NSOpenPanel()
                        if let gguf = UTType(filenameExtension: "gguf") { panel.allowedContentTypes = [gguf] }
                        panel.canChooseFiles = true
                        panel.canChooseDirectories = false
                        panel.allowsMultipleSelection = false
                        FilePanelPresenter.present(panel) { response in
                            guard response == .OK, let url = panel.url else { return }
                            model.settings.mmprojPath = url.path
                            model.saveSettings()
                        }
                    }
                }
            }
        }
        .disabled(model.isAppleModelSelected)
    }


    // MARK: Category 3: Knowledge Base RAG

    private var knowledgeSection: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("ナレッジベース (RAG) 設定").font(.system(size: 17, weight: .semibold))

            VStack(alignment: .leading, spacing: 6) {
                Text("埋め込みモデル").font(.system(size: 14, weight: .medium))
                HStack {
                    Text("MultilingualE5Large (多言語対応ベクトル埋め込み)")
                        .font(.system(size: 14, design: .monospaced))
                    Spacer()
                    Text("固定").font(.system(size: 13)).foregroundStyle(.secondary)
                }
                .padding(10)
                .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 6))
            }

            Divider()

            FolderPickerRow(title: "技術資料フォルダ (Markdown コーパス)", path: $model.documentsDirectory)
            Text("Cisco、Yamaha、Fitelnet などの Markdown ドキュメントが配置されたディレクトリです。").font(.system(size: 13)).foregroundStyle(.secondary)

            Divider()

            FolderPickerRow(title: "検索インデックス保存先", path: $model.knowledgeDirectory)
            Text("ベクトルインデックスやメタデータキャッシュが保存されるローカルディレクトリです。").font(.system(size: 13)).foregroundStyle(.secondary)
        }
    }

    // MARK: Category 4: Native Settings

    private var nativeSettingsSection: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("Swift版設定").font(.system(size: 17, weight: .semibold))

            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 8) {
                    Circle().fill(model.isSettingsLoaded ? Color.green : Color.orange).frame(width: 10, height: 10)
                    Text(model.isSettingsLoaded ? "設定を読み込み済み" : "デフォルト設定を使用中")
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundStyle(model.isSettingsLoaded ? Color.green : Color.orange)
                    Spacer()
                }

                Text(model.settingsStatusMessage)
                    .font(.system(size: 14, design: .monospaced))
                    .foregroundStyle(.secondary)
                    .textSelection(.enabled)
                    .padding(10)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 6))

                HStack(spacing: 10) {
                    Button("設定を再読込") {
                        model.loadSettings()
                    }
                    .buttonStyle(.bordered)

                    Button("設定を保存") {
                        model.saveSettings()
                    }
                    .buttonStyle(.borderedProminent)

                    Button("設定フォルダを Finder で開く") {
                        model.openSettingsDirectory()
                    }
                    .buttonStyle(.bordered)

                    Spacer()

                    Button(role: .destructive, action: model.resetSettingsToDefault) {
                        Text("デフォルトにリセット")
                    }
                    .buttonStyle(.borderless)
                    .foregroundStyle(.red)
                }
            }

            Divider()

            VStack(alignment: .leading, spacing: 8) {
                Text("同期される項目一覧").font(.system(size: 15, weight: .medium))
                Text("設定は Swift版のApplication Support配下へ保存され、起動時に読み込まれます。")
                    .font(.system(size: 13)).foregroundStyle(.secondary)

                VStack(alignment: .leading, spacing: 4) {
                    Text("• 会話履歴数 (`historyLimit`), サンプリング温度 (`temperature`), 繰り返しペナルティ (`repetitionPenalty`)")
                    Text("• モデルパス (`modelPath`), リポジトリ名, GGUFファイル名")
                    Text("• コンテキスト長 (`nCtx`), 最大生成数 (`maxGen`), プロンプト保持数 (`promptKeepTokens`)")
                    Text("• MCP タイムアウト (`mcpTimeout`), キャッシュ保持時間 (`cacheExpiryMinutes`), IP設定 (`ipVersion`)")
                    Text("• 自動 Dry-Run (`autoDryRun`), シリアルポート (`consolePort`), ボーレート (`consoleBaudRate`)")
                    Text("• 6種のワーカー別 KV プリロード (`preloadKnowledge`, `preloadAnalysis` など)")
                    Text("• Vision 有効化 (`visionEnabled`) およびプロジェクターパス (`mmprojPath`)")
                }
                .font(.system(size: 13, design: .monospaced))
                .foregroundStyle(.secondary)
                .padding(12)
                .background(Color(nsColor: .controlBackgroundColor).opacity(0.6), in: RoundedRectangle(cornerRadius: 6))
            }
        }
    }
}

private struct FolderPickerRow: View {
    let title: String
    @Binding var path: String

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text(title).font(.system(size: 14, weight: .medium))
            HStack(spacing: 8) {
                TextField("フォルダのパス", text: $path).textFieldStyle(.roundedBorder)
                Button("選択…") {
                    let panel = NSOpenPanel()
                    panel.canChooseDirectories = true
                    panel.canChooseFiles = false
                    panel.allowsMultipleSelection = false
                    FilePanelPresenter.present(panel) { response in
                        guard response == .OK, let url = panel.url else { return }
                        path = url.path
                    }
                }
            }
        }
    }
}
