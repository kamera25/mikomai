import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { CustomModal } from "../CustomModal";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

describe("CustomModal", () => {
  it("renders confirm modal with danger styling and handles confirm/cancel", () => {
    const onConfirm = vi.fn();
    const onCancel = vi.fn();

    const { container } = render(
      <CustomModal
        isOpen={true}
        type="confirm"
        title="セッションの削除"
        message="このセッションを削除してもよろしいですか？"
        confirmLabel="削除"
        onConfirm={onConfirm}
        onCancel={onCancel}
      />
    );

    expect(screen.getByText("セッションの削除")).toBeInTheDocument();
    expect(
      screen.getByText("このセッションを削除してもよろしいですか？")
    ).toBeInTheDocument();

    const deleteBtn = container.querySelector(".custom-modal-btn.confirm.danger")!;
    expect(deleteBtn).toBeInTheDocument();
    fireEvent.click(deleteBtn);
    expect(onConfirm).toHaveBeenCalledTimes(1);

    const cancelBtn = container.querySelector(".custom-modal-btn.cancel")!;
    fireEvent.click(cancelBtn);
    expect(onCancel).toHaveBeenCalledTimes(1);
  });
});
