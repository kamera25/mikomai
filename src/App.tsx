import "katex/dist/katex.min.css";
import "./App.css";
import { UIProvider } from "./contexts/UIContext";
import { ModelProvider } from "./contexts/ModelContext";
import { ChatProvider } from "./contexts/ChatContext";
import { AppLayout } from "./components/AppLayout/AppLayout";

function App() {
  return (
    <UIProvider>
      <ModelProvider>
        <ChatProvider>
          <AppLayout />
        </ChatProvider>
      </ModelProvider>
    </UIProvider>
  );
}

export default App;
