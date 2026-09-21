import { SettingsProvider } from "../contexts/SettingsContext";
import { UIProvider } from "../contexts/UIContext";
import { ModelProvider } from "../contexts/ModelContext";
import { ChatProvider } from "../contexts/ChatContext";
import { Mediator } from "./Mediator";

export function Root() {
  return (
    <SettingsProvider>
      <UIProvider>
        <ModelProvider>
          <ChatProvider>
            <Mediator />
          </ChatProvider>
        </ModelProvider>
      </UIProvider>
    </SettingsProvider>
  );
}
