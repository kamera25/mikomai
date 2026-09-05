import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { StatusBar } from "../StatusBar";

describe("StatusBar", () => {
  it("renders default display name when no modelPath or loadedModelPath is provided", () => {
    render(<StatusBar modelStatus="NotLoaded" />);
    expect(screen.getByText("Gemma 4-E4B-it (ローカル)")).toBeInTheDocument();
    expect(screen.getByText("NotLoaded")).toBeInTheDocument();
  });

  it("renders model filename from modelPath when loadedModelPath is not provided", () => {
    render(
      <StatusBar
        modelStatus="NotLoaded"
        modelPath="/path/to/models/gemma-4-E2B-it-UD-Q4_K_XL.gguf"
      />
    );
    expect(screen.getByText("gemma-4-E2B-it-UD-Q4_K_XL.gguf")).toBeInTheDocument();
  });

  it("prioritizes loadedModelPath over modelPath when provided", () => {
    render(
      <StatusBar
        modelStatus="Loaded"
        modelPath="/path/to/models/gemma-4-E2B-it-UD-Q4_K_XL.gguf"
        loadedModelPath="/path/to/models/gemma-4-E4B-it-UD-Q4_K_XL.gguf"
      />
    );
    expect(screen.getByText("gemma-4-E4B-it-UD-Q4_K_XL.gguf")).toBeInTheDocument();
    expect(screen.queryByText("gemma-4-E2B-it-UD-Q4_K_XL.gguf")).not.toBeInTheDocument();
    expect(screen.getByText("Ready")).toBeInTheDocument();
  });
});
