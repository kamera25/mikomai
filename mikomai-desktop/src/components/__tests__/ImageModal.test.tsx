import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ImageModal } from "../ImageModal/ImageModal";

describe("ImageModal Component", () => {
  it("renders modal with image and title", () => {
    const handleClose = vi.fn();
    render(
      <ImageModal
        src="data:image/png;base64,fake"
        alt="test_image.png"
        onClose={handleClose}
      />
    );

    expect(screen.getByText("test_image.png")).toBeInTheDocument();
    const img = screen.getByAltText("test_image.png");
    expect(img).toBeInTheDocument();
    expect(img).toHaveAttribute("src", "data:image/png;base64,fake");
  });

  it("calls onClose when close button is clicked", () => {
    const handleClose = vi.fn();
    render(
      <ImageModal
        src="data:image/png;base64,fake"
        alt="test_image.png"
        onClose={handleClose}
      />
    );

    const closeBtn = screen.getByTestId("image-modal-close-btn");
    fireEvent.click(closeBtn);
    expect(handleClose).toHaveBeenCalledTimes(1);
  });

  it("calls onClose when backdrop overlay is clicked", () => {
    const handleClose = vi.fn();
    render(
      <ImageModal
        src="data:image/png;base64,fake"
        alt="test_image.png"
        onClose={handleClose}
      />
    );

    const overlay = screen.getByTestId("image-modal-overlay");
    fireEvent.click(overlay);
    expect(handleClose).toHaveBeenCalledTimes(1);
  });

  it("calls onClose when Escape key is pressed", () => {
    const handleClose = vi.fn();
    render(
      <ImageModal
        src="data:image/png;base64,fake"
        alt="test_image.png"
        onClose={handleClose}
      />
    );

    fireEvent.keyDown(window, { key: "Escape" });
    expect(handleClose).toHaveBeenCalledTimes(1);
  });
});
