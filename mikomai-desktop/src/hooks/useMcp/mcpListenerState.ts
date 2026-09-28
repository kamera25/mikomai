export interface TaskContentState {
  targetContent: string;
  displayedContent: string;
  isTyping: boolean;
  timerId: ReturnType<typeof setTimeout> | null;
  isFinished: boolean;
  summaryText?: string;
}

export function mergeTaskContent(currentAccumulated: string, newContent: string): string {
  if (!currentAccumulated) return newContent || "";
  if (!newContent || currentAccumulated === newContent) return currentAccumulated;
  if (newContent.startsWith(currentAccumulated) || currentAccumulated.includes(newContent)) {
    return newContent.startsWith(currentAccumulated) ? newContent : currentAccumulated;
  }
  return `${currentAccumulated}\n\n${newContent}`;
}

export function typingStep(remaining: number): number {
  if (remaining > 150) return Math.ceil(remaining / 30);
  if (remaining > 60) return 3;
  if (remaining > 20) return 2;
  return 1;
}
