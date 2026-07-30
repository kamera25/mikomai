import React from "react";
import { QuestionItem } from "../../hooks/useQuestionQueue";
import { ChoicePanel } from "./ChoicePanel";
import { InterfaceChoicePanel } from "./InterfaceChoicePanel";
import { IpAddressChoicePanel } from "./IpAddressChoicePanel";

interface QuestionPanelProps {
  questionQueue: QuestionItem[];
  totalQuestionsCount: number;
  handleSelectChoice: (id: string, option: string) => void;
  handleCancelChoice: (id: string) => void;
  handleSelectInterface: (id: string, option: string) => void;
  handleCancelInterface: (id: string) => void;
  handleSelectIpAddress: (id: string, option: string) => void;
  handleCancelIpAddress: (id: string) => void;
}

export function QuestionPanel({
  questionQueue,
  totalQuestionsCount,
  handleSelectChoice,
  handleCancelChoice,
  handleSelectInterface,
  handleCancelInterface,
  handleSelectIpAddress,
  handleCancelIpAddress,
}: QuestionPanelProps) {
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
        onSelect={handleSelectChoice}
        onCancel={handleCancelChoice}
      />
    );
  }

  if (currentQuestion.type === "ipaddress") {
    return (
      <IpAddressChoicePanel
        key={currentQuestion.data.id}
        choice={currentQuestion.data}
        progressPrefix={progressPrefix}
        onSelect={handleSelectIpAddress}
        onCancel={handleCancelIpAddress}
      />
    );
  }

  return (
    <InterfaceChoicePanel
      key={currentQuestion.data.id}
      choice={currentQuestion.data}
      progressPrefix={progressPrefix}
      onSelect={handleSelectInterface}
      onCancel={handleCancelInterface}
    />
  );
}
