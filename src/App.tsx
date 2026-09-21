import "katex/dist/katex.min.css";
import "./App.css";
import { UIProvider } from "./contexts/UIContext";
import { ModelProvider } from "./contexts/ModelContext";
import { ChatProvider } from "./contexts/ChatContext";
import { RootMediator } from "./components/AppLayout/AppLayout";
import { SettingsProvider } from "./contexts/SettingsContext";

export function Root() {
  return (
    <SettingsProvider>
      <UIProvider>
        <ModelProvider>
          <ChatProvider>
            <RootMediator />
          </ChatProvider>
        </ModelProvider>
      </UIProvider>
    </SettingsProvider>
  );
}

export default Root;
