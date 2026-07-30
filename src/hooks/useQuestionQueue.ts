import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

export interface ChoiceConfig {
  id: string;
  title: string;
  message: string;
  options: string[];
}

export interface InterfaceChoiceConfig {
  id: string;
  vendor: string;
  message?: string;
}

export interface IpAddressChoiceConfig {
  id: string;
  title: string;
  message: string;
  subnet: string;
  defaultIp?: string;
}

export type QuestionItem =
  | { type: "choice"; data: ChoiceConfig }
  | { type: "interface"; data: InterfaceChoiceConfig }
  | { type: "ipaddress"; data: IpAddressChoiceConfig };

export function useQuestionQueue() {
  const [questionQueue, setQuestionQueue] = useState<QuestionItem[]>([]);
  const [totalQuestionsCount, setTotalQuestionsCount] = useState(0);

  // Listen to user choice requests from Tauri backend
  useEffect(() => {
    const unlisten = listen<any>("request-user-choice", (event) => {
      const { id, title, message, options } = event.payload;
      const item: QuestionItem = {
        type: "choice",
        data: { id: id || "default", title, message, options },
      };
      setQuestionQueue((prev) => {
        const filtered = prev.filter((q) => q.data.id !== item.data.id);
        const next = [...filtered, item];
        setTotalQuestionsCount((prevTotal) => Math.max(prevTotal, next.length));
        return next;
      });
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Listen to interface choice requests from Tauri backend
  useEffect(() => {
    const unlisten = listen<any>("request-interface-choice", (event) => {
      const { id, vendor, message } = event.payload;
      const item: QuestionItem = {
        type: "interface",
        data: { id: id || "default", vendor, message },
      };
      setQuestionQueue((prev) => {
        const filtered = prev.filter((q) => q.data.id !== item.data.id);
        const next = [...filtered, item];
        setTotalQuestionsCount((prevTotal) => Math.max(prevTotal, next.length));
        return next;
      });
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Listen to IP address choice requests from Tauri backend
  useEffect(() => {
    const unlisten = listen<any>("request-ipaddress-choice", (event) => {
      const { id, title, message, subnet, defaultIp } = event.payload;
      const item: QuestionItem = {
        type: "ipaddress",
        data: { id: id || "default", title, message, subnet, defaultIp },
      };
      setQuestionQueue((prev) => {
        const filtered = prev.filter((q) => q.data.id !== item.data.id);
        const next = [...filtered, item];
        setTotalQuestionsCount((prevTotal) => Math.max(prevTotal, next.length));
        return next;
      });
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const handleSelectChoice = async (id: string, option: string) => {
    setQuestionQueue((prev) => {
      const next = prev.filter((q) => q.data.id !== id);
      if (next.length === 0) setTotalQuestionsCount(0);
      return next;
    });
    try {
      await invoke("submit_user_choice", { id, choice: option });
    } catch (err) {
      console.error("Failed to submit user choice:", err);
    }
  };

  const handleCancelChoice = async (id: string) => {
    setQuestionQueue((prev) => {
      const next = prev.filter((q) => q.data.id !== id);
      if (next.length === 0) setTotalQuestionsCount(0);
      return next;
    });
    try {
      await invoke("submit_user_choice", { id, choice: "cancelled" });
    } catch (err) {
      console.error("Failed to cancel user choice:", err);
    }
  };

  const handleSelectInterface = async (id: string, option: string) => {
    setQuestionQueue((prev) => {
      const next = prev.filter((q) => q.data.id !== id);
      if (next.length === 0) setTotalQuestionsCount(0);
      return next;
    });
    try {
      await invoke("submit_interface_choice", { id, choice: option });
    } catch (err) {
      console.error("Failed to submit interface choice:", err);
    }
  };

  const handleCancelInterface = async (id: string) => {
    setQuestionQueue((prev) => {
      const next = prev.filter((q) => q.data.id !== id);
      if (next.length === 0) setTotalQuestionsCount(0);
      return next;
    });
    try {
      await invoke("submit_interface_choice", { id, choice: "cancelled" });
    } catch (err) {
      console.error("Failed to cancel interface choice:", err);
    }
  };

  const handleSelectIpAddress = async (id: string, option: string) => {
    setQuestionQueue((prev) => {
      const next = prev.filter((q) => q.data.id !== id);
      if (next.length === 0) setTotalQuestionsCount(0);
      return next;
    });
    try {
      await invoke("submit_ipaddress_choice", { id, choice: option });
    } catch (err) {
      console.error("Failed to submit IP address choice:", err);
    }
  };

  const handleCancelIpAddress = async (id: string) => {
    setQuestionQueue((prev) => {
      const next = prev.filter((q) => q.data.id !== id);
      if (next.length === 0) setTotalQuestionsCount(0);
      return next;
    });
    try {
      await invoke("submit_ipaddress_choice", { id, choice: "cancelled" });
    } catch (err) {
      console.error("Failed to cancel IP address choice:", err);
    }
  };

  // Listen to keyboard Escape when choice panels are active
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && questionQueue.length > 0) {
        const current = questionQueue[0];
        if (current.type === "interface") {
          handleCancelInterface(current.data.id);
        } else if (current.type === "ipaddress") {
          handleCancelIpAddress(current.data.id);
        } else {
          handleCancelChoice(current.data.id);
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [questionQueue]);

  return {
    questionQueue,
    totalQuestionsCount,
    handleSelectChoice,
    handleCancelChoice,
    handleSelectInterface,
    handleCancelInterface,
    handleSelectIpAddress,
    handleCancelIpAddress,
  };
}
