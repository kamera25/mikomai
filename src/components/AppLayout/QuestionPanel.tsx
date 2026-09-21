import { QuestionItem } from "../../hooks/useQuestionQueue";
import { ChoicePanel } from "./ChoicePanel";
import { InterfaceChoicePanel } from "./InterfaceChoicePanel";
import { IpAddressChoicePanel } from "./IpAddressChoicePanel";
import { useGuiEvent } from "../../gui/events";

interface QuestionPanelProps {
  questionQueue: QuestionItem[];
  totalQuestionsCount: number;
}

export function QuestionPanel({ questionQueue, totalQuestionsCount }: QuestionPanelProps) {
  const emit = useGuiEvent();
  if (questionQueue.length === 0) return null;

  const currentQuestion = questionQueue[0];
  const currentIndex = totalQuestionsCount - questionQueue.length + 1;
  const progressPrefix = `【質問 ${currentIndex}/${totalQuestionsCount}】`;

  if (currentQuestion.type === "choice") {
    return (
      <ChoicePanel
        key={currentQuestion.data.id}
        choice={currentQuestion.data}
        progressPrefix={progressPrefix}
        onSelect={(id, value) => emit({ type: "question.answer", kind: "choice", id, value })}
        onCancel={(id) => emit({ type: "question.cancel", kind: "choice", id })}
      />
    );
  }

  if (currentQuestion.type === "ipaddress") {
    return (
      <IpAddressChoicePanel
        key={currentQuestion.data.id}
        choice={currentQuestion.data}
        progressPrefix={progressPrefix}
        onSelect={(id, value) => emit({ type: "question.answer", kind: "ipaddress", id, value })}
        onCancel={(id) => emit({ type: "question.cancel", kind: "ipaddress", id })}
      />
    );
  }

  return (
    <InterfaceChoicePanel
      key={currentQuestion.data.id}
      choice={currentQuestion.data}
      progressPrefix={progressPrefix}
      onSelect={(id, value) => emit({ type: "question.answer", kind: "interface", id, value })}
      onCancel={(id) => emit({ type: "question.cancel", kind: "interface", id })}
    />
  );
}
