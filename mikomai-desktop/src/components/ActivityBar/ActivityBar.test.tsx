import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { GuiEventScope } from "../../gui/events";
import { ActivityBar } from "./ActivityBar";

vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }));

describe("ActivityBar", () => {
  it("emits navigation intent without changing application state itself", () => {
    const handle = vi.fn(() => true);
    render(
      <GuiEventScope handle={handle}>
        <ActivityBar activePanel="chat" />
      </GuiEventScope>
    );
    fireEvent.click(screen.getByTitle("activity_bar.settings"));
    expect(handle).toHaveBeenCalledWith({ type: "navigate", panel: "settings" });
  });
});
