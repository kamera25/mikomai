export interface ModelPreset {
  id: string;
  labelKey: string;
  repo: string;
  filename: string;
  mmprojFilename?: string;
}

export const PRESET_MODELS: ModelPreset[] = [
  { id: "gemma-4-e4b-ud", labelKey: "settings.opt_preset_gemma_4_e4b", repo: "unsloth/gemma-4-E4B-it-GGUF", filename: "gemma-4-E4B-it-UD-Q4_K_XL.gguf", mmprojFilename: "mmproj-F16.gguf" },
  { id: "gemma-4-12b-ud", labelKey: "settings.opt_preset_gemma_4_12b", repo: "unsloth/gemma-4-12b-it-GGUF", filename: "gemma-4-12b-it-UD-Q4_K_XL.gguf", mmprojFilename: "mmproj-F16.gguf" },
  { id: "gemma-4-e2b-ud", labelKey: "settings.opt_preset_gemma_4_e2b", repo: "unsloth/gemma-4-E2B-it-GGUF", filename: "gemma-4-E2B-it-UD-Q4_K_XL.gguf", mmprojFilename: "mmproj-F16.gguf" },
];

export function findPreset(repo: string, filename: string): ModelPreset | undefined {
  return PRESET_MODELS.find((preset) => preset.repo === repo && preset.filename === filename);
}
