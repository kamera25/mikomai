import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { GuiEventScope } from "../../gui/events";
import { QuestionPanel } from "./QuestionPanel";

describe("QuestionPanel", () => {
  it("bubbles a choice to the Root mediator", () => {
    const handle = vi.fn(() => true);
    render(
      <GuiEventScope handle={handle}>
        <QuestionPanel
          questionQueue={[
            {
              type: "choice",
              data: { id: "q1", title: "Choose", message: "Pick one", options: ["yes"] },
            },
          ]}
          totalQuestionsCount={1}
        />
      </GuiEventScope>
    );
    fireEvent.click(screen.getByText("yes"));
    expect(handle).toHaveBeenCalledWith({
      type: "question.answer",
      kind: "choice",
      id: "q1",
      value: "yes",
    });
  });
});
